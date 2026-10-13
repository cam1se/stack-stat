//! Interactive dashboard demo backed by in-memory fake providers.
//!
//! Run with `make demo`. The stack is fictional and covers every layer verdict and issue
//! category; one layer's running checks finish after a few seconds so the spinner and change
//! flash are visible. Fakes replace Git, GitHub, and the status store; only the runtime's
//! background release check still calls `gh`. `docs/user/assets/demo.tape` records the
//! published gif from this example.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration as Age, Utc};
use stack_stat_core::{
    config::{Config, LocalCheckConfig},
    model::LocalSync,
    providers::{
        CheckResult, FakeGitHubProvider, FakeGitProvider, GitHubError, GitHubProvider,
        MemoryStatusStore, PrCheckResponse, PrResponse, ReportedState, ReviewRequest,
        ReviewResponse, StackBranch, StackDetailsResponse, StackPr, StackViewResponse,
        StatusStoreProvider,
    },
};

const REPO: &str = "acme/rocket-app";
const WORKTREE_BRANCH: &str = "alex/offline-cache";
const BOT: &str = "review-bot[bot]";
/// Delay before the worktree layer's running checks complete.
const CHECKS_FINISH_AFTER: Duration = Duration::from_secs(6);
/// Simulated GitHub stack-read latency for the first fetch, so the startup intro is visible.
const FIRST_FETCH_LATENCY: Duration = Duration::from_millis(2500);

/// Lifecycle of a fictional layer as reported by `gh stack`.
#[derive(Clone, Copy, PartialEq)]
enum Lifecycle {
    Open,
    Merged,
    Queued,
}

struct Layer {
    branch: &'static str,
    lifecycle: Lifecycle,
    remote_rebase: bool,
    pr: PrResponse,
}

#[tokio::main]
async fn main() -> Result<()> {
    let now = Utc::now();
    let initial = layers(now, false);
    let shas: HashMap<String, String> = initial
        .iter()
        .enumerate()
        .map(|(index, layer)| (layer.branch.to_owned(), format!("{:040x}", index + 1)))
        .collect();
    let github = FakeGitHubProvider::new(stack(&initial), prs(&initial));
    github.set_remote_rebase_branches(
        initial
            .iter()
            .filter(|layer| layer.remote_rebase)
            .map(|layer| layer.branch.to_owned()),
    );
    github.set_repo(REPO);
    let git = FakeGitProvider::new(
        WORKTREE_BRANCH,
        shas.clone(),
        PathBuf::from("/tmp/stack-stat-demo"),
    );
    // The worktree has a commit it has not pushed, so the gutter shows `*↑`.
    git.set_current_sync(LocalSync::Ahead);
    let store = MemoryStatusStore::default();
    report_local_checks(&store, &shas, now)?;

    let finishing = github.clone();
    tokio::spawn(async move {
        tokio::time::sleep(FIRST_FETCH_LATENCY + CHECKS_FINISH_AFTER).await;
        finishing.set_prs(prs(&layers(Utc::now(), true)));
    });

    let github = SlowFirstFetch {
        inner: github,
        fetched: AtomicBool::new(false),
    };
    stack_stat_tui::runtime::run(Arc::new(git), Arc::new(github), Arc::new(store), config()).await
}

/// Fake GitHub whose first stack fetch takes as long as a real `gh` call.
struct SlowFirstFetch {
    inner: FakeGitHubProvider,
    fetched: AtomicBool,
}

#[async_trait]
impl GitHubProvider for SlowFirstFetch {
    async fn stack_view(&self, current_branch: &str) -> Result<StackViewResponse, GitHubError> {
        if !self.fetched.swap(true, Ordering::SeqCst) {
            tokio::time::sleep(FIRST_FETCH_LATENCY).await;
        }
        self.inner.stack_view(current_branch).await
    }
    fn repo(&self) -> String {
        self.inner.repo()
    }
    async fn pr_details(
        &self,
        stack: &StackViewResponse,
    ) -> Result<StackDetailsResponse, GitHubError> {
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
    async fn sync_stack(&self) -> Result<String, GitHubError> {
        self.inner.sync_stack().await
    }
    async fn toggle_draft(&self, number: u32, make_ready: bool) -> Result<(), GitHubError> {
        self.inner.toggle_draft(number, make_ready).await
    }
}

/// Repository configuration: two expected local checks, one config-skipped CI check, and a
/// known review bot. Short poll intervals let the demo pick up the check transition quickly.
fn config() -> Config {
    let mut config = Config::default();
    for (name, description) in [("lint", "cargo clippy"), ("unit", "cargo test")] {
        config.checks.local.insert(
            name.into(),
            LocalCheckConfig {
                description: description.into(),
            },
        );
    }
    config.checks.skip.patterns.push("deploy-preview*".into());
    config.reviews.bots.push(BOT.into());
    config.poll.github = 2;
    config.poll.github_active = 1;
    config
}

/// Bottom-to-top stack exercising each verdict: merged, queued, ready, waiting, and blocked
/// by every blocker category.
fn layers(now: DateTime<Utc>, finished: bool) -> Vec<Layer> {
    let layer = |branch, lifecycle, remote_rebase, pr| Layer {
        branch,
        lifecycle,
        remote_rebase,
        pr,
    };
    let cache_checks = if finished {
        passing(now)
    } else {
        let mut checks = passing(now);
        checks[1] = running(now, "unit-tests", 3);
        checks[3] = running(now, "e2e (android)", 5);
        checks
    };
    vec![
        layer(
            "alex/telemetry-client",
            Lifecycle::Merged,
            false,
            pr(101, "Add a typed telemetry client", passing(now)).approved_by("sam-lee"),
        ),
        layer(
            "alex/event-schema",
            Lifecycle::Merged,
            false,
            pr(102, "Version the analytics event schema", passing(now)).approved_by("priya-n"),
        ),
        layer(
            "alex/session-store",
            Lifecycle::Queued,
            true,
            pr(103, "Persist sessions in the encrypted store", passing(now)).approved_by("sam-lee"),
        ),
        layer(
            "alex/auth-refresh",
            Lifecycle::Open,
            false,
            pr(104, "Refresh auth tokens before expiry", passing(now))
                .approved_by("priya-n")
                .bot_comment(),
        ),
        layer(
            "alex/retry-policy",
            Lifecycle::Open,
            false,
            pr(105, "Back off failed uploads exponentially", passing(now))
                .approved_by("jordan-kim"),
        ),
        layer(
            WORKTREE_BRANCH,
            Lifecycle::Open,
            false,
            pr(106, "Cache feeds for offline reading", cache_checks)
                .required()
                .requested("sam-lee"),
        ),
        layer(
            "alex/sync-indicator",
            Lifecycle::Open,
            false,
            pr(107, "Show sync progress in the toolbar", passing(now))
                .required()
                .threads(2),
        ),
        layer(
            "alex/upload-queue",
            Lifecycle::Open,
            false,
            pr(
                108,
                "Queue uploads while offline",
                with_failure(now, "e2e (android)"),
            )
            .changes_requested_by("jordan-kim")
            .approved_by("priya-n"),
        ),
        layer(
            "alex/conflict-banner",
            Lifecycle::Open,
            true,
            pr(109, "Explain sync conflicts with a banner", passing(now)).approved_by("sam-lee"),
        ),
        layer(
            "alex/settings-screen",
            Lifecycle::Open,
            false,
            pr(
                110,
                "Redesign the settings screen",
                vec![
                    done(now, "build", "SUCCESS", 4),
                    running(now, "unit-tests", 2),
                ],
            )
            .draft(),
        ),
        layer(
            "alex/crash-reporting",
            Lifecycle::Open,
            false,
            pr(111, "Attach breadcrumbs to crash reports", passing(now)).approved_by("priya-n"),
        ),
        layer(
            "alex/release-notes",
            Lifecycle::Open,
            false,
            pr(112, "Generate release notes from commits", passing(now)).approved_by("jordan-kim"),
        ),
        layer(
            "alex/haptic-feedback",
            Lifecycle::Open,
            false,
            pr(113, "Add haptic feedback to capture", passing(now)).approved_by("sam-lee"),
        ),
    ]
}

/// Local results: current passes for most layers and a skip on one green layer, a current
/// failure on the haptic-feedback layer, results from an older commit (stale) on the
/// crash-reporting layer, and a missing `unit` result on the sync-indicator layer.
fn report_local_checks(
    store: &MemoryStatusStore,
    shas: &HashMap<String, String>,
    now: DateTime<Utc>,
) -> Result<()> {
    let result = |state, minutes| CheckResult {
        state,
        message: None,
        recorded_at: now - Age::minutes(minutes),
    };
    let current = |branch: &str| shas[branch].clone();
    for branch in [
        "alex/telemetry-client",
        "alex/event-schema",
        "alex/session-store",
        "alex/auth-refresh",
        "alex/retry-policy",
        WORKTREE_BRANCH,
        "alex/upload-queue",
        "alex/conflict-banner",
        "alex/settings-screen",
        "alex/release-notes",
    ] {
        store.write(
            branch,
            &current(branch),
            "lint",
            &result(ReportedState::Pass, 6),
        )?;
        store.write(
            branch,
            &current(branch),
            "unit",
            &result(ReportedState::Pass, 5),
        )?;
    }
    store.write(
        "alex/retry-policy",
        &current("alex/retry-policy"),
        "unit",
        &result(ReportedState::Skip, 5),
    )?;
    store.write(
        "alex/sync-indicator",
        &current("alex/sync-indicator"),
        "lint",
        &result(ReportedState::Pass, 4),
    )?;
    store.write(
        "alex/haptic-feedback",
        &current("alex/haptic-feedback"),
        "lint",
        &result(ReportedState::Fail, 3),
    )?;
    store.write(
        "alex/haptic-feedback",
        &current("alex/haptic-feedback"),
        "unit",
        &result(ReportedState::Pass, 3),
    )?;
    let older_commit = "f".repeat(40);
    for check in ["lint", "unit"] {
        store.write(
            "alex/crash-reporting",
            &older_commit,
            check,
            &result(ReportedState::Pass, 90),
        )?;
    }
    Ok(())
}

fn stack(layers: &[Layer]) -> StackViewResponse {
    StackViewResponse {
        trunk: "main".into(),
        current_branch: WORKTREE_BRANCH.into(),
        branches: layers
            .iter()
            .map(|layer| StackBranch {
                name: layer.branch.into(),
                is_merged: layer.lifecycle == Lifecycle::Merged,
                is_queued: layer.lifecycle == Lifecycle::Queued,
                pr: Some(StackPr {
                    number: layer.pr.number,
                    url: layer.pr.url.clone(),
                    state: match layer.lifecycle {
                        Lifecycle::Merged => "MERGED",
                        Lifecycle::Open | Lifecycle::Queued => "OPEN",
                    }
                    .into(),
                }),
            })
            .collect(),
    }
}

fn prs(layers: &[Layer]) -> Vec<PrResponse> {
    layers.iter().map(|layer| layer.pr.clone()).collect()
}

fn pr(number: u32, title: &str, checks: Vec<PrCheckResponse>) -> PrResponse {
    PrResponse {
        number,
        url: format!("https://github.com/{REPO}/pull/{number}"),
        title: title.into(),
        is_draft: false,
        review_decision: None,
        reviews: vec![],
        review_requests: vec![],
        unresolved_threads: 0,
        checks,
    }
}

/// Fluent review and lifecycle adjustments for a fictional PR.
trait DemoPr {
    fn approved_by(self, login: &str) -> Self;
    fn changes_requested_by(self, login: &str) -> Self;
    fn requested(self, login: &str) -> Self;
    fn required(self) -> Self;
    fn bot_comment(self) -> Self;
    fn threads(self, count: usize) -> Self;
    fn draft(self) -> Self;
}

impl DemoPr for PrResponse {
    fn approved_by(mut self, login: &str) -> Self {
        if self.review_decision.is_none() {
            self.review_decision = Some("APPROVED".into());
        }
        self.reviews.push(review(login, "APPROVED", false));
        self
    }
    fn changes_requested_by(mut self, login: &str) -> Self {
        self.review_decision = Some("CHANGES_REQUESTED".into());
        self.reviews.push(review(login, "CHANGES_REQUESTED", false));
        self
    }
    fn requested(mut self, login: &str) -> Self {
        self.review_requests.push(ReviewRequest {
            author: login.into(),
            is_bot: false,
        });
        self
    }
    fn required(mut self) -> Self {
        self.review_decision = Some("REVIEW_REQUIRED".into());
        self
    }
    fn bot_comment(mut self) -> Self {
        self.reviews.push(review(BOT, "COMMENTED", true));
        self
    }
    fn threads(mut self, count: usize) -> Self {
        self.unresolved_threads = count;
        self
    }
    fn draft(mut self) -> Self {
        self.is_draft = true;
        self.review_decision = None;
        self
    }
}

fn review(login: &str, state: &str, is_bot: bool) -> ReviewResponse {
    ReviewResponse {
        author: login.into(),
        state: state.into(),
        is_bot,
        submitted_at: None,
    }
}

/// A finished pipeline: required jobs pass, a workflow-skipped job and a config-skipped
/// preview deploy are excluded from passed/total.
fn passing(now: DateTime<Utc>) -> Vec<PrCheckResponse> {
    vec![
        done(now, "build", "SUCCESS", 4),
        done(now, "unit-tests", "SUCCESS", 7),
        done(now, "e2e (ios)", "SUCCESS", 12),
        done(now, "e2e (android)", "SUCCESS", 14),
        done(now, "lint", "SUCCESS", 2),
        done(now, "security-scan", "SUCCESS", 3),
        done(now, "publish-docs", "SKIPPED", 0),
        done(now, "deploy-preview", "SUCCESS", 5),
    ]
}

fn with_failure(now: DateTime<Utc>, name: &str) -> Vec<PrCheckResponse> {
    let mut checks = passing(now);
    for check in &mut checks {
        if check.name == name {
            check.conclusion = Some("FAILURE".into());
        }
    }
    checks
}

fn done(now: DateTime<Utc>, name: &str, conclusion: &str, minutes: i64) -> PrCheckResponse {
    let completed = now - Age::minutes(20);
    check(
        name,
        "COMPLETED",
        Some(conclusion),
        completed - Age::minutes(minutes),
        Some(completed),
    )
}

fn running(now: DateTime<Utc>, name: &str, minutes: i64) -> PrCheckResponse {
    check(name, "IN_PROGRESS", None, now - Age::minutes(minutes), None)
}

fn check(
    name: &str,
    status: &str,
    conclusion: Option<&str>,
    started: DateTime<Utc>,
    completed: Option<DateTime<Utc>>,
) -> PrCheckResponse {
    PrCheckResponse {
        name: name.into(),
        workflow: "CI".into(),
        status: status.into(),
        conclusion: conclusion.map(str::to_owned),
        started_at: Some(started),
        completed_at: completed,
        details_url: format!("https://github.com/{REPO}/actions"),
    }
}
