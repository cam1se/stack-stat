//! Conditional REST requests (`If-None-Match`) against `api.github.com`.
//!
//! GitHub answers an unchanged resource with `304 Not Modified`, which does not count
//! against the primary rate limit. A pull request is watched through the resources whose
//! ETags move when the data behind the GraphQL query moves: the pull request itself (head
//! moves, draft, title, base, auto-merge), its reviews, its review comments, and the check
//! runs and commit statuses of its head commit. Review-thread resolution does not change
//! any of them, so the gate pairs this probe with a periodic full refresh.

use std::{
    collections::HashMap,
    process::Stdio,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use async_trait::async_trait;
use tokio::{process::Command, task::JoinSet};

use super::{ChangeProbe, GitHubError, ProbeOutcome, classify_rest_status};

const API: &str = "https://api.github.com";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Probe that remembers ETags in memory for one process.
pub struct RestChangeProbe {
    owner: String,
    repo: String,
    agent: ureq::Agent,
    state: Arc<Mutex<ProbeState>>,
}

#[derive(Default)]
struct ProbeState {
    token: Option<String>,
    etags: HashMap<String, String>,
    heads: HashMap<u32, String>,
}

/// Whether one conditional request returned a new body.
enum Fetched {
    NotModified,
    Modified(String),
}

impl RestChangeProbe {
    /// Bind a probe to one `github.com` repository; credentials come from `gh auth token`.
    #[must_use]
    pub fn new(owner: String, repo: String) -> Self {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build()
            .into();
        Self {
            owner,
            repo,
            agent,
            state: Arc::default(),
        }
    }

    async fn token(&self) -> Result<String, GitHubError> {
        if let Some(token) = lock(&self.state).token.clone() {
            return Ok(token);
        }
        let output = Command::new("gh")
            .args(["auth", "token"])
            .env("GH_PROMPT_DISABLED", "1")
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output()
            .await
            .map_err(|error| GitHubError::Network(error.to_string()))?;
        let token = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !output.status.success() || token.is_empty() {
            return Err(GitHubError::Auth);
        }
        lock(&self.state).token = Some(token.clone());
        Ok(token)
    }
}

#[async_trait]
impl ChangeProbe for RestChangeProbe {
    async fn probe(&self, prs: &[u32]) -> Result<ProbeOutcome, GitHubError> {
        let token = self.token().await?;
        let mut tasks = JoinSet::new();
        for &number in prs {
            let (agent, state) = (self.agent.clone(), Arc::clone(&self.state));
            let (base, token) = (
                format!("{API}/repos/{}/{}", self.owner, self.repo),
                token.clone(),
            );
            tasks.spawn_blocking(move || probe_pr(&agent, &state, &base, &token, number));
        }
        let mut outcome = ProbeOutcome::default();
        let mut failure = None;
        while let Some(joined) = tasks.join_next().await {
            match joined.map_err(|error| GitHubError::Network(error.to_string())) {
                Ok(Ok(pr)) => {
                    outcome.changed |= pr.modified > 0;
                    outcome.modified += pr.modified;
                    outcome.not_modified += pr.not_modified;
                }
                Ok(Err(error)) | Err(error) => {
                    if matches!(error, GitHubError::Auth) {
                        lock(&self.state).token = None;
                    }
                    // A rate limit outranks other failures so the poller backs off.
                    if failure.is_none() || matches!(error, GitHubError::RateLimited { .. }) {
                        failure = Some(error);
                    }
                }
            }
        }
        failure.map_or(Ok(outcome), Err)
    }
}

fn lock(state: &Mutex<ProbeState>) -> MutexGuard<'_, ProbeState> {
    state.lock().expect("probe state mutex poisoned")
}

/// Probe every resource of one pull request, even after the first change, so each ETag is
/// refreshed and the next pass compares against the current state.
fn probe_pr(
    agent: &ureq::Agent,
    state: &Mutex<ProbeState>,
    base: &str,
    token: &str,
    number: u32,
) -> Result<ProbeOutcome, GitHubError> {
    let mut outcome = ProbeOutcome::default();
    let mut record = |fetched: &Fetched| match fetched {
        Fetched::NotModified => outcome.not_modified += 1,
        Fetched::Modified(_) => {
            outcome.modified += 1;
            outcome.changed = true;
        }
    };
    let pull = conditional_get(agent, state, token, &format!("{base}/pulls/{number}"))?;
    record(&pull);
    if let Fetched::Modified(body) = &pull {
        let head = head_sha(body)?;
        let mut state = lock(state);
        if let Some(old) = state.heads.insert(number, head.clone())
            && old != head
        {
            state.etags.retain(|url, _| !url.contains(&old));
        }
    }
    let head = lock(state).heads.get(&number).cloned().ok_or_else(|| {
        GitHubError::Parse(format!("pull request {number} has no recorded head commit"))
    })?;
    for path in [
        format!("pulls/{number}/reviews"),
        format!("pulls/{number}/comments"),
        format!("commits/{head}/check-runs"),
        format!("commits/{head}/status"),
    ] {
        record(&conditional_get(
            agent,
            state,
            token,
            &format!("{base}/{path}"),
        )?);
    }
    Ok(outcome)
}

fn head_sha(body: &str) -> Result<String, GitHubError> {
    serde_json::from_str::<serde_json::Value>(body)
        .map_err(|error| GitHubError::Parse(error.to_string()))?
        .pointer("/head/sha")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| GitHubError::Parse("pull request response omitted head.sha".into()))
}

/// GET with the stored ETag; a 200 replaces the ETag, a 304 keeps it.
fn conditional_get(
    agent: &ureq::Agent,
    state: &Mutex<ProbeState>,
    token: &str,
    url: &str,
) -> Result<Fetched, GitHubError> {
    let mut request = agent
        .get(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "stack-stat");
    if let Some(etag) = lock(state).etags.get(url) {
        request = request.header("If-None-Match", etag);
    }
    let mut response = request
        .call()
        .map_err(|error| GitHubError::Network(error.to_string()))?;
    let status = response.status().as_u16();
    match status {
        304 => Ok(Fetched::NotModified),
        200 => {
            let etag = response
                .headers()
                .get("etag")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let body = response
                .body_mut()
                .read_to_string()
                .map_err(|error| GitHubError::Network(error.to_string()))?;
            let mut state = lock(state);
            match etag {
                Some(etag) => state.etags.insert(url.to_owned(), etag),
                None => state.etags.remove(url),
            };
            Ok(Fetched::Modified(body))
        }
        _ => {
            let header = |name: &str| {
                response
                    .headers()
                    .get(name)
                    .and_then(|value| value.to_str().ok())
            };
            Err(classify_rest_status(
                status,
                header("x-ratelimit-remaining"),
                header("x-ratelimit-reset"),
                header("retry-after"),
            ))
        }
    }
}
