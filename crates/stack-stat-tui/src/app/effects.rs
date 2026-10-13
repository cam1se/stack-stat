//! Test-drivable provider calls and mutation execution.

use stack_stat_core::providers::{
    GitHubError, GitHubProvider, GitProvider, ProviderAction, StatusStoreProvider,
};

use super::App;
use crate::{
    message::{AppMessage, Effect, Source},
    poll::{fetch_git, fetch_github, fetch_store},
};

/// Perform one user-requested GitHub action without interacting with terminal state.
pub async fn execute<H: GitHubProvider>(
    github: &H,
    action: &ProviderAction,
) -> Result<String, GitHubError> {
    match action {
        ProviderAction::OpenPr(number) => github
            .open_pr_in_browser(*number)
            .await
            .map(|()| String::new()),
        ProviderAction::Checkout(branch) => {
            github.checkout_branch(branch).await.map(|()| String::new())
        }
        ProviderAction::PushStack => github.push_stack().await.map(|()| String::new()),
        ProviderAction::SyncStack => github.sync_stack().await,
        ProviderAction::ToggleDraft { number, make_ready } => github
            .toggle_draft(*number, *make_ready)
            .await
            .map(|()| String::new()),
    }
}

impl<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider> App<G, H, S> {
    /// Execute one fetch and apply its result without timers or terminal I/O.
    pub async fn poll_once(&mut self, source: Source) {
        if source == Source::All {
            for source in [Source::Git, Source::GitHub, Source::Store] {
                self.poll_one(source).await;
            }
        } else {
            self.poll_one(source).await;
        }
    }

    async fn poll_one(&mut self, source: Source) {
        match source {
            Source::Git => {
                self.apply(AppMessage::FetchStarted(Source::Git));
                let result = fetch_git(self.git_provider.as_ref(), self.now)
                    .await
                    .map_err(|err| err.to_string());
                self.apply(AppMessage::Git(result));
            }
            Source::GitHub => {
                self.apply(AppMessage::FetchStarted(Source::GitHub));
                let result = fetch_github(self.github_provider.as_ref(), self.now).await;
                self.apply(AppMessage::GitHub(result));
            }
            Source::Store => {
                self.apply(AppMessage::FetchStarted(Source::Store));
                let live = self.git.as_ref().map(|snapshot| &snapshot.branch_shas);
                let result = fetch_store(self.store_provider.as_ref(), live, self.now)
                    .map_err(|err| err.to_string());
                self.apply(AppMessage::Store(result));
            }
            Source::All => unreachable!("all sources are expanded by poll_once"),
        }
    }

    /// Execute an effect inline, then refresh data after a completed mutation.
    pub async fn run_effect(&mut self, effect: Effect) {
        match effect {
            Effect::Refresh(source) => self.poll_once(source).await,
            Effect::Mutate(action) => {
                let result = execute(self.github_provider.as_ref(), &action).await;
                self.apply(AppMessage::MutationDone { action, result });
                self.poll_once(Source::All).await;
            }
            Effect::Quit => {}
        }
    }

    /// Whether a queued or running GitHub check calls for faster polling.
    #[must_use]
    pub fn ci_active(&self) -> bool {
        self.state
            .branches
            .iter()
            .flat_map(|branch| branch.pr.as_ref())
            .flat_map(|pr| pr.checks.iter())
            .any(|check| {
                !check.skipped_by_config
                    && matches!(
                        check.status,
                        stack_stat_core::model::CheckStatus::Queued
                            | stack_stat_core::model::CheckStatus::InProgress
                    )
            })
    }
}
