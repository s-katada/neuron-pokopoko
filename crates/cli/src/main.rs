use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use poko_core::{ManifestEntry, Report, SyncNote, chunk, chunk_deletes, collect, lint_vault, plan};
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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Lint { dir } => lint_command(&dir),
        Command::Sync { vault, endpoint } => sync_command(&vault, endpoint.as_deref()),
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
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        // 0 だとリダイレクトを追わず、3xx をそのまま返す。
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .into();
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
