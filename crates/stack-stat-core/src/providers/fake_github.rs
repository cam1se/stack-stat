//! Test-controlled GitHub provider with observable effects.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::{
    GitHubError, GitHubProvider, PrResponse, ProviderAction, StackDetailsResponse,
    StackViewResponse,
};

/// Mutable fake GitHub stack and PR responses.
#[derive(Clone, Debug)]
pub struct FakeGitHubProvider {
    state: Arc<Mutex<FakeGitHubState>>,
}

/// GitHub response and recorded actions controlled by a test.
#[derive(Clone, Debug)]
pub struct FakeGitHubState {
    /// Current stack fetch result.
    pub stack: Result<StackViewResponse, GitHubError>,
    /// Remote PR details and rebase comparisons returned by the details query.
    pub details: StackDetailsResponse,
    /// `owner/name` of the GitHub repository, empty when unknown.
    pub repo: String,
    /// Mutations and browser-open actions in invocation order.
    pub actions: Vec<ProviderAction>,
    /// Result returned by subsequent action calls.
    pub mutation_result: Result<String, GitHubError>,
}

impl FakeGitHubProvider {
    /// Seed a fake with stack and PR details.
    #[must_use]
    pub fn new(stack: StackViewResponse, prs: Vec<PrResponse>) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeGitHubState {
                stack: Ok(stack),
                details: StackDetailsResponse {
                    prs,
                    ..StackDetailsResponse::default()
                },
                actions: Vec::new(),
                repo: String::new(),
                mutation_result: Ok(String::new()),
            })),
        }
    }

    /// Change the fake's stack response to an error.
    pub fn set_stack_error(&self, error: GitHubError) {
        self.state.lock().expect("fake GitHub mutex poisoned").stack = Err(error);
    }

    /// Change the fake's stack response to a successful stack.
    pub fn set_stack(&self, stack: StackViewResponse) {
        self.state.lock().expect("fake GitHub mutex poisoned").stack = Ok(stack);
    }

    /// Replace the PR details seen at the next fetch.
    pub fn set_prs(&self, prs: Vec<PrResponse>) {
        self.state
            .lock()
            .expect("fake GitHub mutex poisoned")
            .details
            .prs = prs;
    }

    /// Replace branches whose remote heads need a rebase at the next fetch.
    pub fn set_remote_rebase_branches(&self, branches: impl IntoIterator<Item = String>) {
        self.state
            .lock()
            .expect("fake GitHub mutex poisoned")
            .details
            .remote_rebase_branches = branches.into_iter().collect();
    }

    /// Change the repository identity seen at the next fetch.
    pub fn set_repo(&self, repo: impl Into<String>) {
        self.state.lock().expect("fake GitHub mutex poisoned").repo = repo.into();
    }

    /// Cause subsequent action calls to fail.
    pub fn set_mutation_error(&self, error: GitHubError) {
        self.state
            .lock()
            .expect("fake GitHub mutex poisoned")
            .mutation_result = Err(error);
    }

    /// Read the actions requested so far.
    #[must_use]
    pub fn recorded_actions(&self) -> Vec<ProviderAction> {
        self.state
            .lock()
            .expect("fake GitHub mutex poisoned")
            .actions
            .clone()
    }

    fn record(&self, action: ProviderAction) -> Result<String, GitHubError> {
        let mut state = self.state.lock().expect("fake GitHub mutex poisoned");
        state.actions.push(action);
        state.mutation_result.clone()
    }
}

#[async_trait]
impl GitHubProvider for FakeGitHubProvider {
    async fn stack_view(&self) -> Result<StackViewResponse, GitHubError> {
        self.state
            .lock()
            .expect("fake GitHub mutex poisoned")
            .stack
            .clone()
    }

    fn repo(&self) -> String {
        self.state
            .lock()
            .expect("fake GitHub mutex poisoned")
            .repo
            .clone()
    }

    async fn pr_details(
        &self,
        stack: &StackViewResponse,
    ) -> Result<StackDetailsResponse, GitHubError> {
        let state = self.state.lock().expect("fake GitHub mutex poisoned");
        let mut details = state.details.clone();
        details.prs.retain(|pr| {
            stack.branches.iter().any(|branch| {
                branch
                    .pr
                    .as_ref()
                    .is_some_and(|candidate| candidate.number == pr.number)
            })
        });
        details.remote_rebase_branches.retain(|name| {
            stack
                .branches
                .iter()
                .any(|branch| !branch.is_merged && branch.name == *name)
        });
        Ok(details)
    }

    async fn open_pr_in_browser(&self, number: u32) -> Result<(), GitHubError> {
        self.record(ProviderAction::OpenPr(number)).map(|_| ())
    }

    async fn checkout_branch(&self, branch: &str) -> Result<(), GitHubError> {
        self.record(ProviderAction::Checkout(branch.into()))
            .map(|_| ())
    }

    async fn push_stack(&self) -> Result<(), GitHubError> {
        self.record(ProviderAction::PushStack).map(|_| ())
    }

    async fn sync_stack(&self) -> Result<String, GitHubError> {
        self.record(ProviderAction::SyncStack)
    }

    async fn toggle_draft(&self, number: u32, make_ready: bool) -> Result<(), GitHubError> {
        self.record(ProviderAction::ToggleDraft { number, make_ready })
            .map(|_| ())
    }
}
