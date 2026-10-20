//! Source-aware notices and confirmation-backed recovery of local stack tracking.

use stack_stat_core::{
    compose::StackOutcome,
    model::StackMode,
    providers::{GitHubError, GitHubProvider, GitProvider, ProviderAction, StatusStoreProvider},
};

use super::{App, Mode, Notice, NoticeLevel, Prompt};
use crate::message::{ActionSteps, Source};

impl<G: GitProvider, H: GitHubProvider, S: StatusStoreProvider> App<G, H, S> {
    pub(super) fn notify(&mut self, level: NoticeLevel, text: String) {
        self.set_notice(level, text, None);
    }

    fn set_notice(&mut self, level: NoticeLevel, text: String, source: Option<Source>) {
        if matches!(level, NoticeLevel::Info | NoticeLevel::Warn)
            && self
                .status
                .as_ref()
                .is_some_and(|notice| notice.level == NoticeLevel::Error)
        {
            return;
        }
        self.status = Some(Notice::new(level, text, self.now));
        self.notice_source = source;
    }

    pub(super) fn error(&mut self, source: Source, text: String) {
        self.set_notice(NoticeLevel::Error, text, Some(source));
    }

    pub(super) fn clear_error(&mut self, source: Source) {
        if self.notice_source == Some(source) {
            self.status = None;
            self.notice_source = None;
        }
    }

    pub(super) fn dismiss_error(&mut self) {
        if self
            .status
            .as_ref()
            .is_some_and(|notice| notice.level == NoticeLevel::Error)
        {
            self.status = None;
            self.notice_source = None;
        }
    }

    pub(super) fn complete_mutation(
        &mut self,
        steps: ActionSteps,
        result: Result<String, GitHubError>,
    ) {
        let browser_only =
            steps.before.is_none() && matches!(steps.action, ProviderAction::OpenPr(_));
        if browser_only && self.mutation_busy {
            // An independent browser request must not clear the running action's progress.
            return;
        }
        if !browser_only {
            self.mutation_busy = false;
        }
        match result {
            Ok(output) => {
                let text = if output.trim().is_empty() {
                    success_message(&steps.action)
                } else {
                    output
                        .lines()
                        .rev()
                        .find(|line| !line.is_empty())
                        .unwrap_or_default()
                        .into()
                };
                self.notify(NoticeLevel::Info, text);
            }
            Err(GitHubError::LocalTrackingMissing)
                if steps.before.is_none()
                    && matches!(
                        steps.action,
                        ProviderAction::PushStack
                            | ProviderAction::SyncStack
                            | ProviderAction::RebaseStack
                    ) =>
            {
                if let Some(target) = self.link_target() {
                    self.notify(
                        NoticeLevel::Warn,
                        "Local gh-stack tracking is missing".into(),
                    );
                    self.mode = Mode::Confirm(Prompt::link(steps.action, target));
                } else {
                    self.notify(
                        NoticeLevel::Error,
                        "! no GitHub stack for the current worktree branch; refresh before linking"
                            .into(),
                    );
                }
            }
            Err(GitHubError::RebaseConflict) => self.notify(
                NoticeLevel::Error,
                "! rebase conflict — resolve with gh stack rebase --continue or --abort".into(),
            ),
            Err(error) => self.notify(
                NoticeLevel::Error,
                format!("! {}", error.to_string().lines().next().unwrap_or_default()),
            ),
        }
    }

    fn link_target(&self) -> Option<String> {
        let StackOutcome::InStack(stack) = &self.github.as_ref()?.stack else {
            return None;
        };
        if stack.mode != StackMode::Stacked {
            return None;
        }
        let current_branch = self
            .git
            .as_ref()
            .map_or(stack.current_branch.as_str(), |git| {
                git.current_branch.as_str()
            });
        let branch = stack
            .branches
            .iter()
            .find(|branch| branch.name == current_branch)?;
        Some(
            branch
                .pr
                .as_ref()
                .map_or_else(|| branch.name.clone(), |pr| pr.number.to_string()),
        )
    }
}

fn success_message(action: &ProviderAction) -> String {
    match action {
        ProviderAction::OpenPr(number) => format!("Opened PR #{number}"),
        ProviderAction::Checkout(branch) => format!("Checked out {branch}"),
        ProviderAction::PushStack => "Pushed stack".into(),
        ProviderAction::PushBranch => "Pushed branch".into(),
        ProviderAction::SyncStack => "Synced stack".into(),
        ProviderAction::RebaseStack => "Rebased stack".into(),
        ProviderAction::ToggleDraft { number, make_ready } => format!(
            "PR #{number} is {}",
            if *make_ready { "ready" } else { "draft" }
        ),
    }
}
