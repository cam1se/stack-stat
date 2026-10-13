//! Fatal checks required before opening the dashboard.

use std::{
    fmt, io,
    process::{Command, Stdio},
};

/// Minimal command result needed for prerequisite checks.
#[derive(Clone, Debug)]
pub struct CommandOutput {
    /// Whether the process succeeded.
    pub success: bool,
}

/// Command boundary allowing startup behavior to be exercised without binaries.
pub trait CommandRunner {
    /// Run one program with no interactive standard input or output.
    fn run(&self, program: &str, args: &[&str]) -> io::Result<CommandOutput>;
}

/// Production process runner for startup checks.
#[derive(Clone, Copy, Debug)]
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> io::Result<CommandOutput> {
        let status = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        Ok(CommandOutput {
            success: status.success(),
        })
    }
}

/// Specific prerequisite failure with actionable help.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreflightError {
    /// GitHub CLI is not on PATH.
    MissingGh,
    /// GitHub CLI is not signed in.
    Unauthenticated,
    /// Current directory is not within a Git repository.
    NotGitRepository,
}

impl fmt::Display for PreflightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingGh => write!(
                f,
                "error: gh CLI not found. Install: https://cli.github.com/"
            ),
            Self::Unauthenticated => write!(f, "error: gh not authenticated. Run: gh auth login"),
            Self::NotGitRepository => write!(f, "error: not in a git repository"),
        }
    }
}
impl std::error::Error for PreflightError {}

/// Check `gh`, authentication, then Git in that order.
pub fn run(runner: &dyn CommandRunner) -> Result<(), PreflightError> {
    let version = runner
        .run("gh", &["--version"])
        .map_err(|_| PreflightError::MissingGh)?;
    if !version.success {
        return Err(PreflightError::MissingGh);
    }
    let auth = runner
        .run("gh", &["auth", "status"])
        .map_err(|_| PreflightError::Unauthenticated)?;
    if !auth.success {
        return Err(PreflightError::Unauthenticated);
    }
    let repo = runner
        .run("git", &["rev-parse", "--show-toplevel"])
        .map_err(|_| PreflightError::NotGitRepository)?;
    if !repo.success {
        return Err(PreflightError::NotGitRepository);
    }
    Ok(())
}
