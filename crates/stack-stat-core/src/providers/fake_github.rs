//! Test-controlled GitHub provider with observable effects.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::{GitHubError, GitHubProvider, PrResponse, ProviderAction, StackViewResponse};

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
    /// PRs returned by the details query.
    pub prs: Vec<PrResponse>,
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
                prs,
                actions: Vec::new(),
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
        self.state.lock().expect("fake GitHub mutex poisoned").prs = prs;
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

    async fn pr_details(&self, numbers: &[u32]) -> Result<Vec<PrResponse>, GitHubError> {
        let state = self.state.lock().expect("fake GitHub mutex poisoned");
        Ok(state
            .prs
            .iter()
            .filter(|pr| numbers.contains(&pr.number))
            .cloned()
            .collect())
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
