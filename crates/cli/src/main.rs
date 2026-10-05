use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Lint,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Lint => println!("lint: 未実装"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn core_ping_returns_pong() {
        assert_eq!(poko_core::ping(), "pong");
    }
}
