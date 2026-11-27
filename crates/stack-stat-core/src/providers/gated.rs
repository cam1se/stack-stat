//! A GitHub provider that skips the GraphQL queries while nothing has changed.

use std::{
    collections::BTreeSet,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use async_trait::async_trait;

use super::{ChangeProbe, GitHubError, GitHubProvider, StackDetailsResponse, StackViewResponse};

/// Counters describing how often the gate avoided a GraphQL refresh.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GateStats {
    /// Polls answered from the cache because every conditional request returned 304.
    pub served_from_cache: u64,
    /// Polls that ran the GraphQL stack discovery and detail queries.
    pub full_refreshes: u64,
    /// Conditional requests answered `304 Not Modified`.
    pub not_modified: u64,
    /// Conditional requests answered with a fresh body.
    pub modified: u64,
}

#[derive(Default)]
struct GateState {
    branch: String,
    stack: Option<StackViewResponse>,
    details: Option<StackDetailsResponse>,
    refreshed_at: Option<Instant>,
    /// Pull requests the probe already holds ETags for.
    probed: BTreeSet<u32>,
    /// The latest `stack_view` answered from the cache, so `pr_details` may too.
    serving_cache: bool,
    /// The next poll must run the full queries.
    forced: bool,
}

/// Wraps a provider so a poll runs GraphQL only after the [`ChangeProbe`] reports a change.
///
/// A poll still runs the full queries when there is no cached result, the branch differs,
/// [`GitHubProvider::invalidate`] was called, an action changed GitHub, or `full_refresh`
/// has elapsed. The last case covers what REST cannot see, such as review-thread resolution.
pub struct ChangeGatedProvider<P, D> {
    inner: P,
    probe: D,
    full_refresh: Duration,
    state: Mutex<GateState>,
    served_from_cache: AtomicU64,
    full_refreshes: AtomicU64,
    not_modified: AtomicU64,
    modified: AtomicU64,
}

impl<P: GitHubProvider, D: ChangeProbe> ChangeGatedProvider<P, D> {
    /// Gate `inner` behind `probe`, forcing a full refresh at least every `full_refresh`.
    #[must_use]
    pub fn new(inner: P, probe: D, full_refresh: Duration) -> Self {
        Self {
            inner,
            probe,
            full_refresh,
            state: Mutex::default(),
            served_from_cache: AtomicU64::new(0),
            full_refreshes: AtomicU64::new(0),
            not_modified: AtomicU64::new(0),
            modified: AtomicU64::new(0),
        }
    }

    /// Snapshot of the savings counters since construction.
    #[must_use]
    pub fn stats(&self) -> GateStats {
        GateStats {
            served_from_cache: self.served_from_cache.load(Ordering::Relaxed),
            full_refreshes: self.full_refreshes.load(Ordering::Relaxed),
            not_modified: self.not_modified.load(Ordering::Relaxed),
            modified: self.modified.load(Ordering::Relaxed),
        }
    }

    fn state(&self) -> MutexGuard<'_, GateState> {
        self.state.lock().expect("gate state mutex poisoned")
    }

    /// The cached stack when it may be reused for `branch` without a full refresh.
    fn reusable_stack(&self, branch: &str) -> Option<StackViewResponse> {
        let state = self.state();
        let fresh = state
            .refreshed_at
            .is_some_and(|at| at.elapsed() < self.full_refresh);
        (!state.forced && fresh && state.details.is_some() && state.branch == branch)
            .then(|| state.stack.clone())
            .flatten()
    }

    /// Hold ETags for `numbers` so the next poll compares against current state.
    async fn seed(&self, numbers: &[u32]) {
        let set: BTreeSet<u32> = numbers.iter().copied().collect();
        if self.state().probed == set {
            return;
        }
        // Failures are harmless: the next poll sees unseeded resources as changed.
        let _ = self.probe.probe(numbers).await;
        self.state().probed = set;
    }

    fn after_action<T>(&self, result: Result<T, GitHubError>) -> Result<T, GitHubError> {
        self.invalidate();
        result
    }
}

fn pr_numbers(stack: &StackViewResponse) -> Vec<u32> {
    let mut numbers: Vec<u32> = stack
        .branches
        .iter()
        .filter_map(|branch| branch.pr.as_ref().map(|pr| pr.number))
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    numbers
}

#[async_trait]
impl<P: GitHubProvider, D: ChangeProbe> GitHubProvider for ChangeGatedProvider<P, D> {
    async fn stack_view(&self, current_branch: &str) -> Result<StackViewResponse, GitHubError> {
        if let Some(stack) = self.reusable_stack(current_branch) {
            let numbers = pr_numbers(&stack);
            match self.probe.probe(&numbers).await {
                Ok(outcome) => {
                    self.not_modified
                        .fetch_add(u64::from(outcome.not_modified), Ordering::Relaxed);
                    self.modified
                        .fetch_add(u64::from(outcome.modified), Ordering::Relaxed);
                    if !outcome.changed {
                        self.served_from_cache.fetch_add(1, Ordering::Relaxed);
                        self.state().serving_cache = true;
                        tracing::debug!(?outcome, "GitHub unchanged; skipped GraphQL");
                        return Ok(stack);
                    }
                    self.state().probed = numbers.into_iter().collect();
                }
                // Querying GraphQL while rate limited would only deepen the limit.
                Err(error @ GitHubError::RateLimited { .. }) => return Err(error),
                // Any other probe failure falls back to the full queries, which report
                // their own failure if GitHub is truly unreachable.
                Err(error) => tracing::debug!(%error, "change probe failed; refreshing fully"),
            }
        }
        {
            let mut state = self.state();
            state.serving_cache = false;
            // Cleared before the query so an action finishing mid-refresh stays pending.
            state.forced = false;
        }
        self.full_refreshes.fetch_add(1, Ordering::Relaxed);
        match self.inner.stack_view(current_branch).await {
            Ok(stack) => {
                let mut state = self.state();
                state.branch = current_branch.to_owned();
                state.stack = Some(stack.clone());
                state.details = None;
                state.refreshed_at = None;
                Ok(stack)
            }
            Err(error) => {
                *self.state() = GateState::default();
                Err(error)
            }
        }
    }

    fn repo(&self) -> String {
        self.inner.repo()
    }

    async fn pr_details(
        &self,
        stack: &StackViewResponse,
    ) -> Result<StackDetailsResponse, GitHubError> {
        {
            let state = self.state();
            if state.serving_cache
                && state.stack.as_ref() == Some(stack)
                && let Some(details) = &state.details
            {
                return Ok(details.clone());
            }
        }
        let details = self.inner.pr_details(stack).await?;
        {
            let mut state = self.state();
            if state.stack.as_ref() == Some(stack) {
                state.details = Some(details.clone());
                state.refreshed_at = Some(Instant::now());
            }
        }
        self.seed(&pr_numbers(stack)).await;
        Ok(details)
    }

    fn invalidate(&self) {
        self.state().forced = true;
    }

    async fn open_pr_in_browser(&self, number: u32) -> Result<(), GitHubError> {
        self.inner.open_pr_in_browser(number).await
    }

    async fn checkout_branch(&self, branch: &str) -> Result<(), GitHubError> {
        let result = self.inner.checkout_branch(branch).await;
        self.after_action(result)
    }

    async fn push_stack(&self) -> Result<(), GitHubError> {
        let result = self.inner.push_stack().await;
        self.after_action(result)
    }

    async fn push_branch(&self) -> Result<(), GitHubError> {
        let result = self.inner.push_branch().await;
        self.after_action(result)
    }

    async fn sync_stack(&self) -> Result<String, GitHubError> {
        let result = self.inner.sync_stack().await;
        self.after_action(result)
    }

    async fn rebase_stack(&self) -> Result<String, GitHubError> {
        let result = self.inner.rebase_stack().await;
        self.after_action(result)
    }

    async fn toggle_draft(&self, number: u32, make_ready: bool) -> Result<(), GitHubError> {
        let result = self.inner.toggle_draft(number, make_ready).await;
        self.after_action(result)
    }
}
