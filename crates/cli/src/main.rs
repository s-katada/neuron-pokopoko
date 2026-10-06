use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use poko_core::{
    EXPORT_PAGE_DEFAULT, EXPORT_PAGE_MAX, ManifestEntry, Report, ReviewLogPage, SyncNote, chunk,
    chunk_deletes, collect, lint_vault, plan,
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
