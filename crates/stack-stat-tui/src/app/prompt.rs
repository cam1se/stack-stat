//! Confirmation details and the exact ordered actions they authorize.

use stack_stat_core::providers::ProviderAction;

use crate::message::ActionSteps;

/// A pending user decision, kept separate from informational/error notices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    /// Concise question identifying the operation.
    pub title: String,
    /// Context explaining the operation or why local linking is needed.
    pub detail: String,
    /// Exact operations authorized by a positive response, in execution order.
    pub steps: ActionSteps,
}

impl Prompt {
    pub(super) fn for_action(action: ProviderAction) -> Self {
        let title = match &action {
            ProviderAction::OpenPr(number) => format!("Open PR #{number}"),
            ProviderAction::Checkout(branch) => format!("Checkout {branch}"),
            ProviderAction::PushStack => "Push stack".into(),
            ProviderAction::PushBranch => "Push branch".into(),
            ProviderAction::SyncStack => "Sync stack".into(),
            ProviderAction::ToggleDraft { number, make_ready } => format!(
                "Mark PR #{number} {}",
                if *make_ready { "ready" } else { "draft" }
            ),
        };
        let detail = match &action {
            ProviderAction::OpenPr(_) => "Open the pull request in your browser.",
            ProviderAction::Checkout(_) => "Switch the worktree to the selected stack branch.",
            ProviderAction::PushStack => "Push the stack branches to GitHub.",
            ProviderAction::PushBranch => "Run plain git push for the current branch.",
            ProviderAction::SyncStack => "Fetch, reconcile, rebase, and push the stack.",
            ProviderAction::ToggleDraft { .. } => {
                "Change the pull request's draft state on GitHub."
            }
        }
        .into();
        Self {
            title,
            detail,
            steps: ActionSteps::single(action),
        }
    }

    pub(super) fn link(action: ProviderAction, target: String) -> Self {
        let operation = match action {
            ProviderAction::PushStack => "push",
            ProviderAction::SyncStack => "sync",
            _ => unreachable!("only push and sync require local stack tracking"),
        };
        Self {
            title: format!("Link stack and {operation}"),
            detail:
                "Local gh-stack tracking is missing. Check out the GitHub stack before continuing."
                    .into(),
            steps: ActionSteps::linked(target, action),
        }
    }
}
