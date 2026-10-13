use std::process::ExitCode;

use anyhow::Result;
use chrono::Utc;
use clap::{Parser, Subcommand};
use stack_stat_core::{
    config::Config,
    providers::{CliGitProvider, FileStatusStore, GitProvider},
};
use stack_stat_tui::{
    preflight::{self, SystemCommandRunner},
    report::{self, ReportArgs},
};

#[derive(Parser)]
#[command(
    name = "stack-stat",
    version,
    about = "Status for stacked pull requests"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Record, inspect, or clear local check results.
    Report(ReportArgs),
    /// Update the installed binary from GitHub Releases.
    Update,
    /// Print the binary version.
    Version,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<()> {
    match Cli::parse().command {
        Some(Command::Version) => println!("stack-stat {}", env!("CARGO_PKG_VERSION")),
        Some(Command::Report(args)) => {
            let git = CliGitProvider::discover(&std::env::current_dir()?)?;
            let config = Config::load(git.worktree_root(), None)?;
            let store = FileStatusStore::new(git.common_dir());
            let output = report::run(&args, &git, &store, &config, Utc::now()).await?;
            println!("{output}");
        }
        Some(Command::Update) => anyhow::bail!("stack-stat update is not yet available"),
        None => {
            preflight::run(&SystemCommandRunner)?;
            println!("TUI not yet wired");
        }
    }
    Ok(())
}
