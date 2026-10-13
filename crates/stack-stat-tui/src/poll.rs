//! Independent timers for Git, GitHub, and local status-store snapshots.

use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use stack_stat_core::{
    compose::{GitHubSnapshot, GitSnapshot, StackOutcome, StoreSnapshot},
    config::Config,
    providers::{GitHubError, GitHubProvider, GitProvider, StatusStoreProvider},
};
use tokio::{
    sync::{Notify, mpsc, watch},
    task::JoinHandle,
    time::{Instant, sleep, sleep_until},
};

use crate::message::{AppMessage, Source};

/// Load local Git data through one provider, without touching UI state.
pub async fn fetch_git<G: GitProvider>(git: &G, now: DateTime<Utc>) -> anyhow::Result<GitSnapshot> {
    let current_branch = git.current_branch().await?;
    let branch_shas = git.branch_shas().await?;
    Ok(GitSnapshot {
        current_branch,
        branch_shas,
        fetched_at: now,
    })
}

/// Load stack and exact PR details, treating off-stack as a successful outcome.
pub async fn fetch_github<H: GitHubProvider>(
    github: &H,
    now: DateTime<Utc>,
) -> Result<GitHubSnapshot, GitHubError> {
    match github.stack_view().await {
        Ok(stack) => {
            let numbers: Vec<_> = stack
                .branches
                .iter()
                .filter_map(|branch| branch.pr.as_ref().map(|pr| pr.number))
                .collect();
            let prs = github.pr_details(&numbers).await?;
            Ok(GitHubSnapshot {
                stack: StackOutcome::InStack(stack),
                prs,
                fetched_at: now,
            })
        }
        Err(GitHubError::NotInStack) => Ok(GitHubSnapshot {
            stack: StackOutcome::NotInStack,
            prs: vec![],
            fetched_at: now,
        }),
        Err(error) => Err(error),
    }
}

/// Read local check results with warnings after bounded retention.
pub fn fetch_store<S: StatusStoreProvider>(
    store: &S,
    live: Option<&HashMap<String, String>>,
    now: DateTime<Utc>,
) -> anyhow::Result<StoreSnapshot> {
    if let Some(live) = live {
        store.gc(live)?;
    }
    let (entries, warnings) = store.read_with_warnings()?;
    Ok(StoreSnapshot {
        entries,
        warnings,
        fetched_at: now,
    })
}

/// Wake-up and shutdown handles for all background pollers.
pub struct PollHandles {
    git: Arc<Notify>,
    github: Arc<Notify>,
    store: Arc<Notify>,
    active: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

impl PollHandles {
    /// Wake every poller without waiting for its next timer.
    pub fn refresh_all(&self) {
        self.git.notify_one();
        self.github.notify_one();
        self.store.notify_one();
    }

    /// Shorten remote polling while checks are queued or running.
    pub fn set_ci_active(&self, active: bool) {
        if *self.active.borrow() != active {
            self.active.send_replace(active);
        }
    }
}

impl Drop for PollHandles {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// Spawn pollers, each sending results on the same app message channel.
pub fn spawn<G, H, S>(
    git: Arc<G>,
    github: Arc<H>,
    store: Arc<S>,
    config: Config,
    tx: mpsc::UnboundedSender<AppMessage>,
) -> PollHandles
where
    G: GitProvider + 'static,
    H: GitHubProvider + 'static,
    S: StatusStoreProvider + 'static,
{
    let git_wake = Arc::new(Notify::new());
    let github_wake = Arc::new(Notify::new());
    let store_wake = Arc::new(Notify::new());
    let (active_tx, active_rx) = watch::channel(false);
    let (branches_tx, branches_rx) = watch::channel::<Option<HashMap<String, String>>>(None);
    let git_interval = config.poll.git;
    let store_interval = config.poll.store;
    let git_task = {
        let tx = tx.clone();
        let wake = Arc::clone(&git_wake);
        tokio::spawn(async move {
            loop {
                if tx.send(AppMessage::FetchStarted(Source::Git)).is_err() {
                    break;
                }
                let result = fetch_git(git.as_ref(), Utc::now()).await;
                if let Ok(snapshot) = &result {
                    branches_tx.send_replace(Some(snapshot.branch_shas.clone()));
                }
                if tx
                    .send(AppMessage::Git(result.map_err(|err| err.to_string())))
                    .is_err()
                {
                    break;
                }
                tokio::select! { () = sleep(Duration::from_secs(git_interval)) => {}, () = wake.notified() => {}, () = tx.closed() => break }
            }
        })
    };
    let github_task = {
        let tx = tx.clone();
        let wake = Arc::clone(&github_wake);
        let mut active_rx = active_rx;
        let github_poll = config.poll.clone();
        tokio::spawn(async move {
            loop {
                if tx.send(AppMessage::FetchStarted(Source::GitHub)).is_err() {
                    break;
                }
                let result = fetch_github(github.as_ref(), Utc::now()).await;
                let delay = match &result {
                    Err(GitHubError::RateLimited { reset_hint }) => {
                        rate_limit_wait(reset_hint.as_deref(), Utc::now())
                    }
                    _ => github_interval(&github_poll, *active_rx.borrow()),
                };
                let limited = matches!(result, Err(GitHubError::RateLimited { .. }));
                if tx.send(AppMessage::GitHub(result)).is_err() {
                    break;
                }
                if limited {
                    tokio::select! { () = sleep_until(Instant::now() + delay) => {}, () = tx.closed() => break }
                } else {
                    tokio::select! {
                        () = sleep(delay) => {},
                        () = wake.notified() => {},
                        changed = active_rx.changed() => { if changed.is_err() { break; } },
                        () = tx.closed() => break,
                    }
                }
            }
        })
    };
    let store_task = {
        let tx = tx.clone();
        let wake = Arc::clone(&store_wake);
        tokio::spawn(async move {
            loop {
                if tx.send(AppMessage::FetchStarted(Source::Store)).is_err() {
                    break;
                }
                let live = branches_rx.borrow().clone();
                let result = fetch_store(store.as_ref(), live.as_ref(), Utc::now());
                if tx
                    .send(AppMessage::Store(result.map_err(|err| err.to_string())))
                    .is_err()
                {
                    break;
                }
                tokio::select! { () = sleep(Duration::from_secs(store_interval)) => {}, () = wake.notified() => {}, () = tx.closed() => break }
            }
        })
    };
    PollHandles {
        git: git_wake,
        github: github_wake,
        store: store_wake,
        active: active_tx,
        tasks: vec![git_task, github_task, store_task],
    }
}

/// Select GitHub polling cadence according to visible CI activity.
#[must_use]
pub fn github_interval(poll: &stack_stat_core::config::PollConfig, ci_active: bool) -> Duration {
    Duration::from_secs(if ci_active {
        poll.github_active
    } else {
        poll.github
    })
}

/// Derive a bounded API retry delay from a reset hint, or use a five-minute fallback.
#[must_use]
pub fn rate_limit_wait(hint: Option<&str>, now: DateTime<Utc>) -> Duration {
    if let Some(hint) = hint {
        for word in hint.split_whitespace() {
            if let Ok(reset) = DateTime::parse_from_rfc3339(word.trim_end_matches(['.', ',', ';']))
                && let Ok(duration) = (reset.with_timezone(&Utc) - now).to_std()
            {
                return duration;
            }
        }
    }
    Duration::from_secs(300)
}
