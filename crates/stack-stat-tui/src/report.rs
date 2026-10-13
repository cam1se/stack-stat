//! Local check reporting and listing without launching a terminal UI.

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use clap::{Args, ValueEnum};
use serde::Serialize;
use stack_stat_core::{
    config::Config,
    providers::{
        CheckResult, CliGitProvider, DETACHED_HEAD, GitProvider, ReportedState, StatusStoreProvider,
    },
};

/// CLI arguments for reporting, listing, or clearing local check results.
#[derive(Clone, Debug, Args)]
pub struct ReportArgs {
    /// Name of a check to report.
    #[arg(value_name = "CHECK", required_unless_present_any = ["clear", "list"], conflicts_with_all = ["clear", "list"])]
    pub check: Option<String>,
    /// Result for the named check.
    #[arg(value_enum, required_unless_present_any = ["clear", "list"], conflicts_with_all = ["clear", "list"], requires = "check")]
    pub state: Option<Outcome>,
    /// Explanation for this check result.
    #[arg(long, requires = "check")]
    pub message: Option<String>,
    /// Branch to report or clear; defaults to current worktree branch.
    #[arg(long)]
    pub branch: Option<String>,
    /// Git revision to associate with the result; defaults to branch tip.
    #[arg(long, requires = "check")]
    pub sha: Option<String>,
    /// Remove all reports for the chosen branch.
    #[arg(long, conflicts_with = "list")]
    pub clear: bool,
    /// List all reports.
    #[arg(long)]
    pub list: bool,
    /// Render the list as machine-readable JSON.
    #[arg(long, requires = "list")]
    pub json: bool,
}

/// Valid report outcomes accepted by clap.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Outcome {
    /// Check succeeded.
    Pass,
    /// Check failed.
    Fail,
    /// Check was not run.
    Skip,
}

impl From<Outcome> for ReportedState {
    fn from(value: Outcome) -> Self {
        match value {
            Outcome::Pass => Self::Pass,
            Outcome::Fail => Self::Fail,
            Outcome::Skip => Self::Skip,
        }
    }
}

#[derive(Serialize)]
struct ListedEntry<'a> {
    branch: &'a str,
    sha: &'a str,
    checks: &'a std::collections::BTreeMap<String, CheckResult>,
    stale: bool,
}

/// Apply one `report` invocation and return text for stdout.
pub async fn run(
    args: &ReportArgs,
    git: &CliGitProvider,
    store: &dyn StatusStoreProvider,
    config: &Config,
    now: DateTime<Utc>,
) -> Result<String> {
    if args.list {
        let tips = git.branch_shas().await?;
        let entries = store.read_all()?;
        let rows: Vec<_> = entries
            .iter()
            .map(|entry| ListedEntry {
                branch: &entry.branch,
                sha: &entry.sha,
                checks: &entry.checks,
                stale: tips.get(&entry.branch) != Some(&entry.sha),
            })
            .collect();
        if args.json {
            return serde_json::to_string_pretty(&rows).context("serializing reports");
        }
        let mut output = String::new();
        for row in rows {
            let age = if row.stale { "stale" } else { "fresh" };
            output.push_str(&format!("{} {} ({age})\n", row.branch, row.sha));
            for (name, check) in row.checks {
                output.push_str(&format!("  {name}: {:?}\n", check.state));
            }
        }
        return Ok(output);
    }

    let branch = match args.branch.as_deref() {
        Some(branch) => branch.to_owned(),
        None => {
            let current = git.current_branch().await?;
            if current == DETACHED_HEAD {
                bail!("detached HEAD: pass --branch to report or clear a check");
            }
            current
        }
    };
    if args.clear {
        let count = store
            .read_all()?
            .iter()
            .filter(|entry| entry.branch == branch)
            .count();
        store.clear(&branch)?;
        return Ok(format!("Cleared {count} report(s) for {branch}"));
    }

    let check = args.check.as_deref().context("a check name is required")?;
    let state = args.state.context("a check state is required")?;
    let sha = if let Some(rev) = &args.sha {
        git.resolve_rev(rev).await?
    } else {
        git.branch_shas()
            .await?
            .get(&branch)
            .cloned()
            .with_context(|| format!("branch {branch} has no local commit; supply --sha"))?
    };
    if !config.checks.local.is_empty() && !config.checks.local.contains_key(check) {
        eprintln!("note: {check} is not declared in repository [checks.local]");
    }
    store.write(
        &branch,
        &sha,
        check,
        &CheckResult {
            state: state.into(),
            message: args.message.clone(),
            recorded_at: now,
        },
    )?;
    Ok(format!("Reported {check} for {branch}@{}", &sha[..7]))
}
