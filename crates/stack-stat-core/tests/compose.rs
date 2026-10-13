use std::collections::{BTreeMap, HashMap};

use chrono::{TimeZone, Utc};
use stack_stat_core::{
    compose::{GitHubSnapshot, GitSnapshot, StackOutcome, StoreSnapshot, compose},
    config::{Config, LocalCheckConfig},
    model::{CheckHealth, LocalCheckState, PrLifecycle, RebaseNeed, ReviewState, StackStatus},
    providers::{CheckResult, GraphqlResponse, ReportedState, StackViewResponse, StatusEntry},
};

fn now() -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(1_760_000_000, 0).single().unwrap()
}
fn git(current: &str) -> GitSnapshot {
    GitSnapshot {
        current_branch: current.into(),
        branch_shas: HashMap::from([
            ("auth".into(), "a".repeat(40)),
            ("api".into(), "c".repeat(40)),
            ("ui".into(), "d".repeat(40)),
        ]),
        fetched_at: now(),
    }
}
fn github() -> GitHubSnapshot {
    GitHubSnapshot {
        repo: String::new(),
        stack: StackOutcome::InStack(
            serde_json::from_str::<StackViewResponse>(include_str!("fixtures/stack_view.json"))
                .unwrap(),
        ),
        prs: serde_json::from_str::<GraphqlResponse>(include_str!("fixtures/pr_details.json"))
            .unwrap()
            .into_prs(),
        fetched_at: now(),
    }
}
fn local(branch: &str, sha: &str) -> StatusEntry {
    StatusEntry {
        branch: branch.into(),
        sha: sha.into(),
        checks: BTreeMap::from([(
            "lint".into(),
            CheckResult {
                state: ReportedState::Pass,
                message: None,
                recorded_at: now(),
            },
        )]),
    }
}

#[test]
fn composition_retains_repository_identity_when_available() {
    let mut snapshot = github();
    snapshot.repo = "acme/web".into();

    assert_eq!(
        compose(Some(&git("api")), Some(&snapshot), None, &Config::default()).repo,
        "acme/web"
    );
    assert!(
        compose(Some(&git("api")), Some(&github()), None, &Config::default())
            .repo
            .is_empty()
    );
}

#[test]
fn git_only_loading_retains_worktree_identity_without_guessing_rows() {
    let state = compose(Some(&git("api")), None, None, &Config::default());
    assert_eq!(state.stack_status, StackStatus::Loading);
    assert_eq!(state.worktree_branch, "api");
    assert!(state.branches.is_empty());
    assert_eq!(state.last_git_fetch, Some(now()));
    assert!(state.last_github_fetch.is_none());
}

#[test]
fn composition_joins_stack_prs_checks_reviews_and_local_checks() {
    let mut config = Config::default();
    config.checks.skip.names.push("coverage".into());
    config.checks.local.insert(
        "lint".into(),
        LocalCheckConfig {
            description: "clippy".into(),
        },
    );
    config.checks.local.insert(
        "test".into(),
        LocalCheckConfig {
            description: "cargo test".into(),
        },
    );
    config.reviews.bots.push("alice".into());
    config.reviews.gate_on_unresolved_threads = true;
    let store = StoreSnapshot {
        entries: vec![local("api", &"c".repeat(40))],
        warnings: vec![],
        fetched_at: now(),
    };
    let state = compose(Some(&git("api")), Some(&github()), Some(&store), &config);
    assert_eq!(state.stack_status, StackStatus::Ready);
    assert_eq!(state.trunk, "main");
    assert_eq!(
        state
            .branches
            .iter()
            .map(|b| b.name.as_str())
            .collect::<Vec<_>>(),
        ["auth", "api", "ui"]
    );
    assert_eq!(state.branches[0].lifecycle, PrLifecycle::Merged);
    assert_eq!(state.branches[1].rebase, RebaseNeed::None);
    assert_eq!(state.branches[2].rebase, RebaseNeed::Parent);
    let api = &state.branches[1];
    assert!(api.is_current);
    assert_eq!(api.pr.as_ref().unwrap().unresolved_threads, 2);
    assert!(api.pr.as_ref().unwrap().threads_gate);
    assert!(
        api.pr
            .as_ref()
            .unwrap()
            .checks
            .iter()
            .find(|c| c.name == "coverage")
            .unwrap()
            .skipped_by_config
    );
    assert_eq!(api.pr.as_ref().unwrap().check_summary().total, 1);
    assert_eq!(
        api.pr.as_ref().unwrap().check_summary().worst,
        CheckHealth::Running
    );
    assert!(
        api.pr
            .as_ref()
            .unwrap()
            .reviews
            .iter()
            .any(|r| r.author == "lint-bot" && r.is_bot)
    );
    assert!(
        api.pr
            .as_ref()
            .unwrap()
            .reviews
            .iter()
            .any(|r| r.author == "bob" && r.state == ReviewState::Pending)
    );
    assert_eq!(
        api.local_checks
            .iter()
            .map(|check| (&*check.name, check.state))
            .collect::<Vec<_>>(),
        [
            ("lint", LocalCheckState::Pass),
            ("test", LocalCheckState::Missing)
        ]
    );
    assert!(!api.local_checks[0].is_stale);
    assert!(state.branches[0].pr.as_ref().unwrap().reviews[0].is_bot);
}

#[test]
fn composition_keeps_unresolved_threads_non_gating_by_default() {
    let state = compose(Some(&git("api")), Some(&github()), None, &Config::default());
    let pr = state.branches[1].pr.as_ref().unwrap();
    assert_eq!(pr.unresolved_threads, 2);
    assert!(!pr.threads_gate);
}

#[test]
fn composition_classifies_rebase_targets_from_stack_order() {
    let mut snapshot = github();
    let StackOutcome::InStack(stack) = &mut snapshot.stack else {
        unreachable!();
    };
    stack.branches[1].needs_rebase = true;
    stack.branches[2].needs_rebase = true;

    let state = compose(Some(&git("api")), Some(&snapshot), None, &Config::default());

    assert_eq!(state.branches[0].rebase, RebaseNeed::None);
    assert_eq!(state.branches[1].rebase, RebaseNeed::Trunk);
    assert_eq!(state.branches[2].rebase, RebaseNeed::Parent);
    assert!(
        state
            .branches
            .iter()
            .all(|branch| !branch.trunk_rebase_gates)
    );
}

#[test]
fn rebase_trigger_cascades_to_later_open_layers_but_not_merged_ones() {
    let mut snapshot = github();
    let StackOutcome::InStack(stack) = &mut snapshot.stack else {
        unreachable!();
    };
    stack.branches[1].needs_rebase = true;
    stack.branches[2].needs_rebase = false;
    let mut config = Config::default();
    config.rebase.onto_trunk = true;

    let state = compose(Some(&git("api")), Some(&snapshot), None, &config);

    assert_eq!(state.branches[0].rebase, RebaseNeed::None);
    assert_eq!(state.branches[1].rebase, RebaseNeed::Trunk);
    assert_eq!(state.branches[2].rebase, RebaseNeed::Stacked);

    config.rebase.onto_trunk = false;
    let state = compose(Some(&git("api")), Some(&snapshot), None, &config);
    assert_eq!(state.branches[2].rebase, RebaseNeed::None);
}

#[test]
fn composition_makes_first_open_layer_trunk_target_after_merged_layers() {
    let mut snapshot = github();
    let StackOutcome::InStack(stack) = &mut snapshot.stack else {
        unreachable!();
    };
    stack.branches[0].needs_rebase = true;
    stack.branches[1].is_merged = true;
    stack.branches[1].needs_rebase = true;
    stack.branches[2].needs_rebase = true;
    let mut config = Config::default();
    config.rebase.onto_trunk = true;

    let state = compose(Some(&git("ui")), Some(&snapshot), None, &config);

    assert_eq!(state.branches[0].rebase, RebaseNeed::None);
    assert_eq!(state.branches[1].rebase, RebaseNeed::None);
    assert_eq!(state.branches[2].rebase, RebaseNeed::Trunk);
    assert!(
        state
            .branches
            .iter()
            .all(|branch| branch.trunk_rebase_gates)
    );
}

#[test]
fn previous_commit_result_is_stale_and_declared_unreported_check_is_missing() {
    let mut config = Config::default();
    config.checks.local.insert(
        "lint".into(),
        LocalCheckConfig {
            description: "clippy".into(),
        },
    );
    config.checks.local.insert(
        "test".into(),
        LocalCheckConfig {
            description: "tests".into(),
        },
    );
    let store = StoreSnapshot {
        entries: vec![local("api", &"b".repeat(40))],
        warnings: vec![],
        fetched_at: now(),
    };
    let state = compose(Some(&git("api")), Some(&github()), Some(&store), &config);
    let checks = &state.branches[1].local_checks;
    assert!(checks[0].is_stale);
    assert_eq!(checks[0].sha, "b".repeat(40));
    assert_eq!(checks[1].state, LocalCheckState::Missing);
    assert!(checks[1].recorded_at.is_none());
}

#[test]
fn no_stack_and_off_stack_branch_do_not_mark_any_row_current() {
    let not_in_stack = GitHubSnapshot {
        repo: String::new(),
        stack: StackOutcome::NotInStack,
        prs: vec![],
        fetched_at: now(),
    };
    let state = compose(
        Some(&git("main")),
        Some(&not_in_stack),
        None,
        &Config::default(),
    );
    assert_eq!(state.stack_status, StackStatus::NotInStack);
    assert!(state.branches.is_empty());
    assert_eq!(state.worktree_branch, "main");
    let state = compose(
        Some(&git("main")),
        Some(&github()),
        None,
        &Config::default(),
    );
    assert!(state.branches.iter().all(|b| !b.is_current));
}

#[test]
fn remote_only_branch_has_missing_local_checks() {
    let mut snapshot = git("api");
    snapshot.branch_shas.remove("ui");
    let mut config = Config::default();
    config.checks.local.insert(
        "lint".into(),
        LocalCheckConfig {
            description: "clippy".into(),
        },
    );
    let store = StoreSnapshot {
        entries: vec![local("ui", &"d".repeat(40))],
        warnings: vec![],
        fetched_at: now(),
    };
    let state = compose(Some(&snapshot), Some(&github()), Some(&store), &config);
    let ui = &state.branches[2];
    assert!(ui.head_sha.is_empty());
    assert_eq!(ui.local_checks[0].state, LocalCheckState::Missing);
}

#[test]
fn missing_pr_details_retain_number_without_claiming_ready() {
    let mut github = github();
    github.prs.retain(|pr| pr.number != 43);
    let state = compose(Some(&git("api")), Some(&github), None, &Config::default());
    assert_eq!(state.branches[1].pr_number, Some(43));
    assert!(state.branches[1].pr.is_none());
}

#[test]
fn queued_stack_layer_is_not_mistaken_for_open() {
    let mut snapshot = github();
    if let StackOutcome::InStack(ref mut stack) = snapshot.stack {
        stack.branches[2].is_queued = true;
    }
    let state = compose(Some(&git("api")), Some(&snapshot), None, &Config::default());
    assert_eq!(state.branches[2].lifecycle, PrLifecycle::Queued);
}

#[test]
fn workflow_startup_failure_is_a_failing_check_not_neutral() {
    let mut snapshot = github();
    let pr = snapshot.prs.iter_mut().find(|pr| pr.number == 43).unwrap();
    pr.checks[0].status = "COMPLETED".into();
    pr.checks[0].conclusion = Some("STARTUP_FAILURE".into());
    pr.checks.truncate(1);
    let state = compose(Some(&git("api")), Some(&snapshot), None, &Config::default());
    let summary = state.branches[1].pr.as_ref().unwrap().check_summary();
    assert_eq!(summary.worst, CheckHealth::Failure);
}

#[test]
fn unrecognized_completed_conclusion_is_not_false_green() {
    let mut snapshot = github();
    let pr = snapshot.prs.iter_mut().find(|pr| pr.number == 43).unwrap();
    pr.checks.truncate(1);
    pr.checks[0].status = "COMPLETED".into();
    pr.checks[0].conclusion = Some("FUTURE_ERROR".into());
    let state = compose(Some(&git("api")), Some(&snapshot), None, &Config::default());
    assert_eq!(
        state.branches[1].pr.as_ref().unwrap().check_summary().worst,
        CheckHealth::Failure
    );
}
