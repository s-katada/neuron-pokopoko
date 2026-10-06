use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use poko_core::{
    EXPORT_PAGE_DEFAULT, EXPORT_PAGE_MAX, IMPORT_BATCH_MAX, ImportResult, ManifestEntry, Report,
    ReviewLog, ReviewLogPage, SyncNote, chunk, chunk_deletes, collect, lint_vault,
    parse_review_log_line, plan,
};
use serde::Serialize;

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// vault のノートを検査する
    Lint { dir: PathBuf },
    /// vault を Worker に同期する
    ///
    /// 送り先は --endpoint、無ければ POKO_ENDPOINT、それも無ければ http://localhost:8787。
    /// POKO_ACCESS_CLIENT_ID と POKO_ACCESS_CLIENT_SECRET が両方あるとき、全リクエストに Cloudflare Access のサービストークンを付ける。片方だけはエラー。
    Sync {
        vault: PathBuf,
        /// Worker のベース URL。未指定なら POKO_ENDPOINT、それも無ければ http://localhost:8787
        #[arg(long)]
        endpoint: Option<String>,
    },
    /// 復習ログを JSONL に書き出す
    ///
    /// 送り先は --endpoint、無ければ POKO_ENDPOINT、それも無ければ http://localhost:8787。
    /// POKO_ACCESS_CLIENT_ID と POKO_ACCESS_CLIENT_SECRET が両方あるとき、全リクエストに Cloudflare Access のサービストークンを付ける。片方だけはエラー。
    Export {
        file: PathBuf,
        /// Worker のベース URL。未指定なら POKO_ENDPOINT、それも無ければ http://localhost:8787
        #[arg(long)]
        endpoint: Option<String>,
        /// 1 ページの件数。1 から 500。既定 200
        #[arg(long, hide = true)]
        page_size: Option<usize>,
    },
    /// JSONL から復習ログを戻す
    ///
    /// 送り先は --endpoint、無ければ POKO_ENDPOINT、それも無ければ http://localhost:8787。
    /// POKO_ACCESS_CLIENT_ID と POKO_ACCESS_CLIENT_SECRET が両方あるとき、全リクエストに Cloudflare Access のサービストークンを付ける。片方だけはエラー。
    Import {
        file: PathBuf,
        /// Worker のベース URL。未指定なら POKO_ENDPOINT、それも無ければ http://localhost:8787
        #[arg(long)]
        endpoint: Option<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Lint { dir } => lint_command(&dir),
        Command::Sync { vault, endpoint } => sync_command(&vault, endpoint.as_deref()),
        Command::Export {
            file,
            endpoint,
            page_size,
        } => export_command(&file, endpoint.as_deref(), page_size),
        Command::Import { file, endpoint } => import_command(&file, endpoint.as_deref()),
    }
}

fn lint_command(dir: &Path) -> ExitCode {
    match lint_vault(dir) {
        Ok(report) if report.findings.is_empty() => {
            println!("OK ({} notes, {} cards)", report.notes, report.cards);
            ExitCode::SUCCESS
        }
        Ok(report) => {
            print_findings(&report);
            ExitCode::from(1)
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn sync_command(vault: &Path, endpoint: Option<&str>) -> ExitCode {
    match lint_vault(vault) {
        Ok(report) if report.findings.is_empty() => {}
        Ok(report) => {
            print_findings(&report);
            return ExitCode::from(1);
        }
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    }

    let endpoint = resolve_endpoint(endpoint);
    let access = match access_from_env() {
        Ok(access) => access,
        Err(code) => return code,
    };
    let local = match collect(vault) {
        Ok(notes) => notes,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };
    let agent = http_agent();
    let remote = match get_json::<Vec<ManifestEntry>>(
        &agent,
        &url(&endpoint, "/api/sync/manifest"),
        access.as_ref(),
    ) {
        Ok(remote) => remote,
        Err(code) => return code,
    };
    let sync_plan = plan(&local, &remote);
    let chunks = match chunk(&sync_plan.upserts) {
        Ok(chunks) => chunks,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };
    for notes in &chunks {
        let body = NotesBody { notes };
        if let Err(code) = post_json(
            &agent,
            &url(&endpoint, "/api/sync/notes"),
            &body,
            access.as_ref(),
        ) {
            return code;
        }
    }
    for ids in chunk_deletes(&sync_plan.deletes) {
        let body = DeleteBody { ids: &ids };
        if let Err(code) = post_json(
            &agent,
            &url(&endpoint, "/api/sync/delete"),
            &body,
            access.as_ref(),
        ) {
            return code;
        }
    }
    let cards: usize = sync_plan.upserts.iter().map(|note| note.cards.len()).sum();
    println!(
        "synced: upserted {} notes ({cards} cards), deleted {}, unchanged {}",
        sync_plan.upserts.len(),
        sync_plan.deletes.len(),
        sync_plan.unchanged
    );
    ExitCode::SUCCESS
}

fn print_findings(report: &Report) {
    for finding in &report.findings {
        println!("{}: {}: {}", finding.path, finding.kind, finding.detail);
    }
}

fn export_command(file: &Path, endpoint: Option<&str>, page_size: Option<usize>) -> ExitCode {
    let page_size = match page_size {
        None => EXPORT_PAGE_DEFAULT,
        Some(size) if (1..=EXPORT_PAGE_MAX).contains(&size) => size,
        Some(_) => {
            eprintln!("page-size は 1 から {EXPORT_PAGE_MAX}");
            return ExitCode::from(1);
        }
    };
    let endpoint = resolve_endpoint(endpoint);
    let access = match access_from_env() {
        Ok(access) => access,
        Err(code) => return code,
    };
    let parent = file
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "export.jsonl".to_owned());
    let tmp = parent.join(format!(".{}.{name}.tmp", std::process::id()));
    let count = match write_export(&http_agent(), &endpoint, access.as_ref(), &tmp, page_size) {
        Ok(count) => count,
        Err(code) => {
            let _ = fs::remove_file(&tmp);
            return code;
        }
    };
    if let Err(err) = fs::rename(&tmp, file) {
        let _ = fs::remove_file(&tmp);
        eprintln!("{err}");
        return ExitCode::from(1);
    }
    println!("exported {count}");
    ExitCode::SUCCESS
}

fn write_export(
    agent: &ureq::Agent,
    endpoint: &str,
    access: Option<&Access>,
    tmp: &Path,
    page_size: usize,
) -> Result<usize, ExitCode> {
    let mut writer = BufWriter::new(File::create(tmp).map_err(|err| {
        eprintln!("{err}");
        ExitCode::from(1)
    })?);
    let mut after = 0_i64;
    let mut count = 0_usize;
    loop {
        let path = format!("/api/export/reviews?after={after}&limit={page_size}");
        let rows = get_json::<Vec<ReviewLogPage>>(agent, &url(endpoint, &path), access)?;
        let last = rows.last().map(|row| row.id);
        let short = rows.len() < page_size;
        for row in &rows {
            let line = row.log.to_json_line().map_err(|err| {
                eprintln!("{err}");
                ExitCode::from(1)
            })?;
            writeln!(writer, "{line}").map_err(|err| {
                eprintln!("{err}");
                ExitCode::from(1)
            })?;
            count += 1;
        }
        if short {
            break;
        }
        let Some(last) = last else {
            break;
        };
        if last <= after {
            eprintln!("export の id が進まなかった");
            return Err(ExitCode::from(1));
        }
        after = last;
    }
    writer.flush().map_err(|err| {
        eprintln!("{err}");
        ExitCode::from(1)
    })?;
    writer.get_ref().sync_all().map_err(|err| {
        eprintln!("{err}");
        ExitCode::from(1)
    })?;
    Ok(count)
}

const IMPORT_BODY_MAX: usize = 256 * 1024;

fn import_command(file: &Path, endpoint: Option<&str>) -> ExitCode {
    let text = match fs::read_to_string(file) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };
    let mut errors = Vec::new();
    let mut logs = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match parse_review_log_line(line) {
            Ok(log) => logs.push(log),
            Err(err) => errors.push(format!("{}: {err}", index + 1)),
        }
    }
    if !errors.is_empty() {
        for error in errors {
            eprintln!("{error}");
        }
        return ExitCode::from(1);
    }
    let rows = logs.len();
    let mut seen = HashSet::new();
    logs.retain(|log| seen.insert((log.card_key.clone(), log.reviewed_at)));
    let mut duplicates = rows - logs.len();
    let chunks = match chunk_import(&logs) {
        Ok(chunks) => chunks,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };
    let endpoint = resolve_endpoint(endpoint);
    let access = match access_from_env() {
        Ok(access) => access,
        Err(code) => return code,
    };
    let agent = http_agent();
    let mut inserted = 0;
    let mut missing = Vec::new();
    let mut missing_seen = HashSet::new();
    for chunk in &chunks {
        let body = ImportBody { reviews: chunk };
        let result = match post_json_read::<ImportResult>(
            &agent,
            &url(&endpoint, "/api/import/reviews"),
            &body,
            access.as_ref(),
        ) {
            Ok(result) => result,
            Err(code) => return code,
        };
        inserted += result.inserted;
        duplicates += result.duplicates;
        for key in result.missing {
            if missing_seen.insert(key.clone()) {
                missing.push(key);
            }
        }
    }
    println!(
        "imported {inserted}, duplicates {duplicates}, missing {}",
        missing.len()
    );
    if missing.is_empty() {
        ExitCode::SUCCESS
    } else {
        for key in missing {
            eprintln!("{key}");
        }
        ExitCode::from(1)
    }
}

fn chunk_import(logs: &[ReviewLog]) -> Result<Vec<Vec<ReviewLog>>, String> {
    // {"reviews":} と配列の []。2 行目以降は区切りの , を足す。
    const BODY_BASE: usize = "{\"reviews\":}".len() + 2;
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut current_len = 0usize;
    for log in logs {
        if current.len() == IMPORT_BATCH_MAX {
            chunks.push(std::mem::take(&mut current));
            current_len = 0;
        }
        let encoded = log.to_json_line().map_err(|err| err.to_string())?;
        let next_len = if current.is_empty() {
            BODY_BASE + encoded.len()
        } else {
            current_len + 1 + encoded.len()
        };
        current.push(log.clone());
        current_len = next_len;
        if current_len > IMPORT_BODY_MAX {
            current.pop();
            if current.is_empty() {
                return Err("1 行が 256KB を超えている".to_owned());
            }
            chunks.push(std::mem::take(&mut current));
            current.push(log.clone());
            current_len = BODY_BASE + encoded.len();
            if current_len > IMPORT_BODY_MAX {
                return Err("1 行が 256KB を超えている".to_owned());
            }
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    Ok(chunks)
}

fn http_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        // 0 だとリダイレクトを追わず、3xx をそのまま返す。
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .into()
}

fn resolve_endpoint(flag: Option<&str>) -> String {
    if let Some(flag) = flag.filter(|value| !value.is_empty()) {
        return flag.to_owned();
    }
    if let Some(env) = std::env::var("POKO_ENDPOINT")
        .ok()
        .filter(|value| !value.is_empty())
    {
        return env;
    }
    "http://localhost:8787".to_owned()
}

fn url(endpoint: &str, path: &str) -> String {
    format!("{}{path}", endpoint.trim_end_matches('/'))
}

#[derive(Serialize)]
struct NotesBody<'a> {
    notes: &'a [&'a SyncNote],
}

#[derive(Serialize)]
struct DeleteBody<'a> {
    ids: &'a [String],
}

struct Access {
    id: String,
    secret: String,
}

fn access_from_env() -> Result<Option<Access>, ExitCode> {
    let id = std::env::var("POKO_ACCESS_CLIENT_ID")
        .ok()
        .filter(|value| !value.is_empty());
    let secret = std::env::var("POKO_ACCESS_CLIENT_SECRET")
        .ok()
        .filter(|value| !value.is_empty());
    match (id, secret) {
        (Some(id), Some(secret)) => Ok(Some(Access { id, secret })),
        (None, None) => Ok(None),
        (None, Some(_)) => {
            eprintln!("POKO_ACCESS_CLIENT_ID が無い");
            Err(ExitCode::from(1))
        }
        (Some(_), None) => {
            eprintln!("POKO_ACCESS_CLIENT_SECRET が無い");
            Err(ExitCode::from(1))
        }
    }
}

fn with_access<B>(
    request: ureq::RequestBuilder<B>,
    access: Option<&Access>,
) -> ureq::RequestBuilder<B> {
    match access {
        Some(access) => request
            .header("CF-Access-Client-Id", access.id.as_str())
            .header("CF-Access-Client-Secret", access.secret.as_str()),
        None => request,
    }
}

fn get_json<T: serde::de::DeserializeOwned>(
    agent: &ureq::Agent,
    url: &str,
    access: Option<&Access>,
) -> Result<T, ExitCode> {
    let mut response = http(with_access(agent.get(url), access).call())?;
    response.body_mut().read_json().map_err(|err| {
        eprintln!("{err}");
        ExitCode::from(1)
    })
}

#[derive(Serialize)]
struct ImportBody<'a> {
    reviews: &'a [ReviewLog],
}

fn post_json_read<T: serde::de::DeserializeOwned>(
    agent: &ureq::Agent,
    url: &str,
    body: &impl Serialize,
    access: Option<&Access>,
) -> Result<T, ExitCode> {
    let mut response = http(with_access(agent.post(url), access).send_json(body))?;
    response.body_mut().read_json().map_err(|err| {
        eprintln!("{err}");
        ExitCode::from(1)
    })
}

fn post_json(
    agent: &ureq::Agent,
    url: &str,
    body: &impl Serialize,
    access: Option<&Access>,
) -> Result<(), ExitCode> {
    http(with_access(agent.post(url), access).send_json(body))?;
    Ok(())
}

fn http(
    result: Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<ureq::http::Response<ureq::Body>, ExitCode> {
    match result {
        Ok(mut response) => {
            let status = response.status();
            if status.is_success() {
                Ok(response)
            } else if status.is_redirection() || status.as_u16() == 401 || status.as_u16() == 403 {
                eprintln!(
                    "Access に拒否されました(POKO_ACCESS_CLIENT_ID / POKO_ACCESS_CLIENT_SECRET とサービス認証ポリシーを確認)"
                );
                Err(ExitCode::from(1))
            } else {
                let body = response
                    .body_mut()
                    .read_to_string()
                    .unwrap_or_else(|err| err.to_string());
                eprintln!("{} {body}", status.as_u16());
                Err(ExitCode::from(1))
            }
        }
        Err(err) => {
            eprintln!("接続失敗: {err}");
            Err(ExitCode::from(1))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn core_ping_returns_pong() {
        assert_eq!(poko_core::ping(), "pong");
    }
}
