use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use poko_core::lint_vault;

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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Lint { dir } => match lint_vault(&dir) {
            Ok(report) if report.findings.is_empty() => {
                println!("OK ({} notes, {} cards)", report.notes, report.cards);
                ExitCode::SUCCESS
            }
            Ok(report) => {
                for finding in report.findings {
                    println!("{}: {}: {}", finding.path, finding.kind, finding.detail);
                }
                ExitCode::from(1)
            }
            Err(err) => {
                eprintln!("{err}");
                ExitCode::from(1)
            }
        },
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn core_ping_returns_pong() {
        assert_eq!(poko_core::ping(), "pong");
    }
}
