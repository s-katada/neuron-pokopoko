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
    Sync {
        vault: PathBuf,
        /// Worker のベース URL
        #[arg(long, default_value = "http://localhost:8787")]
        endpoint: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Lint { dir } => lint_command(&dir),
        Command::Sync { vault, endpoint } => sync_command(&vault, &endpoint),
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

fn sync_command(vault: &Path, endpoint: &str) -> ExitCode {
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

    let local = match collect(vault) {
        Ok(notes) => notes,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .into();
    let remote = match get_json::<Vec<ManifestEntry>>(&agent, &url(endpoint, "/api/sync/manifest"))
    {
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
        if let Err(code) = post_json(&agent, &url(endpoint, "/api/sync/notes"), &body) {
            return code;
        }
    }
    for ids in chunk_deletes(&sync_plan.deletes) {
        let body = DeleteBody { ids: &ids };
        if let Err(code) = post_json(&agent, &url(endpoint, "/api/sync/delete"), &body) {
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

fn get_json<T: serde::de::DeserializeOwned>(agent: &ureq::Agent, url: &str) -> Result<T, ExitCode> {
    let mut response = http(agent.get(url).call())?;
    response.body_mut().read_json().map_err(|err| {
        eprintln!("{err}");
        ExitCode::from(1)
    })
}

fn post_json(agent: &ureq::Agent, url: &str, body: &impl Serialize) -> Result<(), ExitCode> {
    http(agent.post(url).send_json(body))?;
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
