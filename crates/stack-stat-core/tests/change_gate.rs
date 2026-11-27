//! The change gate must skip GraphQL while the probe reports no change, and only then.

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use stack_stat_core::providers::{
    ChangeGatedProvider, ChangeProbe, FakeGitHubProvider, GitHubError, GitHubProvider,
    ProbeOutcome, StackDetailsResponse, StackViewResponse, classify_rest_status,
    graphql::StackDiscovery,
};

fn stack() -> StackViewResponse {
    StackDiscovery::new("api")
        .unwrap()
        .accept_response(0, include_str!("fixtures/stack_view.json"), "")
        .unwrap()
        .unwrap()
}

/// Counts the GraphQL-backed calls that reach the wrapped provider.
#[derive(Clone)]
struct Counting {
    inner: FakeGitHubProvider,
    stack_views: Arc<AtomicUsize>,
    detail_queries: Arc<AtomicUsize>,
}

impl Counting {
    fn new() -> Self {
        Self {
            inner: FakeGitHubProvider::new(stack(), Vec::new()),
            stack_views: Arc::default(),
            detail_queries: Arc::default(),
        }
    }

    fn queries(&self) -> (usize, usize) {
        (
            self.stack_views.load(Ordering::SeqCst),
            self.detail_queries.load(Ordering::SeqCst),
        )
    }
}

#[async_trait]
impl GitHubProvider for Counting {
    async fn stack_view(&self, branch: &str) -> Result<StackViewResponse, GitHubError> {
        self.stack_views.fetch_add(1, Ordering::SeqCst);
        self.inner.stack_view(branch).await
    }
    fn repo(&self) -> String {
        self.inner.repo()
    }
    async fn pr_details(
        &self,
        stack: &StackViewResponse,
    ) -> Result<StackDetailsResponse, GitHubError> {
        self.detail_queries.fetch_add(1, Ordering::SeqCst);
        self.inner.pr_details(stack).await
    }
    async fn open_pr_in_browser(&self, number: u32) -> Result<(), GitHubError> {
        self.inner.open_pr_in_browser(number).await
    }
    async fn checkout_branch(&self, branch: &str) -> Result<(), GitHubError> {
        self.inner.checkout_branch(branch).await
    }
    async fn push_stack(&self) -> Result<(), GitHubError> {
        self.inner.push_stack().await
    }
    async fn push_branch(&self) -> Result<(), GitHubError> {
        self.inner.push_branch().await
    }
    async fn sync_stack(&self) -> Result<String, GitHubError> {
        self.inner.sync_stack().await
    }
    async fn rebase_stack(&self) -> Result<String, GitHubError> {
        self.inner.rebase_stack().await
    }
    async fn toggle_draft(&self, number: u32, make_ready: bool) -> Result<(), GitHubError> {
        self.inner.toggle_draft(number, make_ready).await
    }
}

/// Plays back scripted probe results and records the PR sets it was asked about.
#[derive(Clone, Default)]
struct ScriptedProbe {
    script: Arc<Mutex<VecDeque<Result<ProbeOutcome, GitHubError>>>>,
    calls: Arc<Mutex<Vec<Vec<u32>>>>,
}

impl ScriptedProbe {
    fn push(&self, result: Result<ProbeOutcome, GitHubError>) {
        self.script.lock().unwrap().push_back(result);
    }
    fn calls(&self) -> Vec<Vec<u32>> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl ChangeProbe for ScriptedProbe {
    async fn probe(&self, prs: &[u32]) -> Result<ProbeOutcome, GitHubError> {
        self.calls.lock().unwrap().push(prs.to_vec());
        self.script
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Ok(unchanged()))
    }
}

fn unchanged() -> ProbeOutcome {
    ProbeOutcome {
        changed: false,
        not_modified: 4,
        modified: 0,
    }
}

fn changed() -> ProbeOutcome {
    ProbeOutcome {
        changed: true,
        not_modified: 3,
        modified: 1,
    }
}

const LONG: Duration = Duration::from_secs(3600);

async fn poll<P: GitHubProvider>(
    gate: &P,
    branch: &str,
) -> Result<(StackViewResponse, StackDetailsResponse), GitHubError> {
    let stack = gate.stack_view(branch).await?;
    let details = gate.pr_details(&stack).await?;
    Ok((stack, details))
}

fn gate(
    full_refresh: Duration,
) -> (
    ChangeGatedProvider<Counting, ScriptedProbe>,
    Counting,
    ScriptedProbe,
) {
    let inner = Counting::new();
    let probe = ScriptedProbe::default();
    (
        ChangeGatedProvider::new(inner.clone(), probe.clone(), full_refresh),
        inner,
        probe,
    )
}

#[tokio::test]
async fn unchanged_polls_issue_no_graphql_queries() {
    let (gate, inner, probe) = gate(LONG);
    let first = poll(&gate, "api").await.unwrap();
    assert_eq!(inner.queries(), (1, 1));
    for _ in 0..3 {
        assert_eq!(poll(&gate, "api").await.unwrap(), first);
    }
    assert_eq!(inner.queries(), (1, 1), "unchanged polls must not query");
    let stats = gate.stats();
    assert_eq!((stats.full_refreshes, stats.served_from_cache), (1, 3));
    assert_eq!(stats.not_modified, 12);
    // The first poll seeds the probe with the stack's pull requests, sorted and unique.
    assert_eq!(probe.calls()[0], vec![42, 43, 44]);
}

#[tokio::test]
async fn a_reported_change_refreshes_with_new_data() {
    let (gate, inner, probe) = gate(LONG);
    poll(&gate, "api").await.unwrap();
    probe.push(Ok(changed()));
    let prs = vec![stack_stat_core::providers::PrResponse {
        number: 42,
        url: String::new(),
        title: "new title".into(),
        is_draft: true,
        review_decision: None,
        reviews: Vec::new(),
        review_requests: Vec::new(),
        unresolved_threads: 0,
        checks: Vec::new(),
    }];
    inner.inner.set_prs(prs);
    let (_, details) = poll(&gate, "api").await.unwrap();
    assert_eq!(inner.queries(), (2, 2));
    assert_eq!(details.prs[0].title, "new title");
    assert_eq!(gate.stats().modified, 1);
    // Once refreshed, the next unchanged poll is served from the cache again.
    poll(&gate, "api").await.unwrap();
    assert_eq!(inner.queries(), (2, 2));
}

#[tokio::test]
async fn invalidate_and_actions_force_a_full_refresh() {
    let (gate, inner, _) = gate(LONG);
    poll(&gate, "api").await.unwrap();
    gate.invalidate();
    poll(&gate, "api").await.unwrap();
    assert_eq!(inner.queries(), (2, 2));
    gate.toggle_draft(42, true).await.unwrap();
    poll(&gate, "api").await.unwrap();
    assert_eq!(inner.queries(), (3, 3));
    poll(&gate, "api").await.unwrap();
    assert_eq!(
        inner.queries(),
        (3, 3),
        "the force flag clears after a refresh"
    );
}

#[tokio::test]
async fn an_expired_full_refresh_interval_bypasses_the_probe() {
    let (gate, inner, probe) = gate(Duration::ZERO);
    poll(&gate, "api").await.unwrap();
    poll(&gate, "api").await.unwrap();
    assert_eq!(inner.queries(), (2, 2));
    assert_eq!(
        probe.calls().len(),
        1,
        "an expired cache is not probed, only the first poll seeds it"
    );
}

#[tokio::test]
async fn switching_branch_refreshes() {
    let (gate, inner, _) = gate(LONG);
    poll(&gate, "api").await.unwrap();
    poll(&gate, "ui").await.unwrap();
    assert_eq!(inner.queries(), (2, 2));
}

#[tokio::test]
async fn probe_rate_limit_surfaces_without_querying_graphql() {
    let (gate, inner, probe) = gate(LONG);
    poll(&gate, "api").await.unwrap();
    let limited = GitHubError::RateLimited {
        reset_hint: Some("resets at 2026-11-27T20:00:00+00:00".into()),
    };
    probe.push(Err(limited.clone()));
    assert_eq!(poll(&gate, "api").await.unwrap_err(), limited);
    assert_eq!(inner.queries(), (1, 1));
}

#[tokio::test]
async fn other_probe_failures_fall_back_to_the_full_queries() {
    let (gate, inner, probe) = gate(LONG);
    poll(&gate, "api").await.unwrap();
    probe.push(Err(GitHubError::Network("offline".into())));
    poll(&gate, "api").await.unwrap();
    assert_eq!(inner.queries(), (2, 2));
}

#[tokio::test]
async fn a_failed_refresh_is_not_cached() {
    let (gate, inner, _) = gate(LONG);
    inner.inner.set_stack_error(GitHubError::Auth);
    assert_eq!(poll(&gate, "api").await.unwrap_err(), GitHubError::Auth);
    inner.inner.set_stack(stack());
    poll(&gate, "api").await.unwrap();
    poll(&gate, "api").await.unwrap();
    assert_eq!(
        inner.queries(),
        (2, 1),
        "recovery refreshes once, then caches"
    );
}

#[test]
fn rest_status_classification_separates_rate_limits_from_permission_errors() {
    assert!(matches!(
        classify_rest_status(403, Some("0"), Some("1795805252"), None),
        GitHubError::RateLimited { reset_hint: Some(hint) } if hint.starts_with("resets at 20")
    ));
    assert!(matches!(
        classify_rest_status(429, None, None, Some("60")),
        GitHubError::RateLimited { reset_hint: None }
    ));
    assert_eq!(
        classify_rest_status(401, None, None, None),
        GitHubError::Auth
    );
    assert!(matches!(
        classify_rest_status(403, Some("4000"), None, None),
        GitHubError::Network(_)
    ));
}
