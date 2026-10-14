use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{TimeZone, Utc};
use stack_stat_core::{
    compose::{GitHubSnapshot, GitSnapshot, StackOutcome, StoreSnapshot, compose},
    config::{Config, LocalCheckConfig},
    model::{
        CheckHealth, Issue, LocalCheckState, LocalSync, PrLifecycle, RebaseNeed, ReviewState,
        StackMode, StackStatus, Verdict,
    },
    providers::{
        CheckResult, ReportedState, StatusEntry,
        graphql::{StackDiscovery, parse_stack_response},
    },
};

fn now() -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(1_760_000_000, 0).single().unwrap()
}
fn git(current: &str) -> GitSnapshot {
    GitSnapshot {
        current_branch: current.into(),
        current_sync: LocalSync::InSync,
        branch_shas: HashMap::from([
            ("auth".into(), "a".repeat(40)),
            ("api".into(), "c".repeat(40)),
            ("ui".into(), "d".repeat(40)),
        ]),
        fetched_at: now(),
    }
}
fn github() -> GitHubSnapshot {
    let stack = StackDiscovery::new("api")
        .unwrap()
        .accept_response(0, include_str!("fixtures/stack_view.json"), "")
        .unwrap()
        .unwrap();
    let details = parse_stack_response(include_str!("fixtures/pr_details.json"), &stack).unwrap();
    GitHubSnapshot {
        repo: String::new(),
        stack: StackOutcome::InStack(stack),
        details,
        fetched_at: now(),
    }
}
fn single_github() -> GitHubSnapshot {
    let mut snapshot = github();
    let StackOutcome::InStack(stack) = &mut snapshot.stack else {
        unreachable!();
    };
    stack.mode = StackMode::Single;
    stack.trunk = "release".into();
    stack.branches.retain(|branch| branch.name == "api");
    snapshot.details.prs.retain(|pr| pr.number == 43);
    snapshot
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
fn composition_applies_local_sync_to_current_branch_only() {
    let mut snapshot = git("api");
    snapshot.current_sync = LocalSync::Diverged;

    let state = compose(Some(&snapshot), Some(&github()), None, &Config::default());

    assert_eq!(
        state
            .branches
            .iter()
            .map(|branch| (branch.name.as_str(), branch.local_sync))
            .collect::<Vec<_>>(),
        [
            ("auth", LocalSync::InSync),
            ("api", LocalSync::Diverged),
            ("ui", LocalSync::InSync),
        ]
    );
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
    assert_eq!(state.branches[2].rebase, RebaseNeed::None);
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
fn single_pr_composition_retains_full_readiness_and_its_base_rebase() {
    let mut snapshot = single_github();
    snapshot.details.remote_rebase_branches.insert("api".into());
    let mut git = git("api");
    git.current_sync = LocalSync::Ahead;
    let mut config = Config::default();
    config.rebase.onto_trunk = true;
    config.reviews.gate_on_unresolved_threads = true;
    config.checks.local.insert(
        "lint".into(),
        LocalCheckConfig {
            description: "clippy".into(),
        },
    );
    let store = StoreSnapshot {
        entries: vec![local("api", &"c".repeat(40))],
        warnings: vec![],
        fetched_at: now(),
    };

    let state = compose(Some(&git), Some(&snapshot), Some(&store), &config);

    assert_eq!(state.mode, StackMode::Single);
    assert_eq!(state.stack_status, StackStatus::Ready);
    assert_eq!(state.trunk, "release");
    assert_eq!(state.branches.len(), 1);
    let branch = &state.branches[0];
    assert_eq!(branch.lifecycle, PrLifecycle::Open);
    assert_eq!(branch.pr_number, Some(43));
    assert!(branch.is_current);
    assert_eq!(branch.local_sync, LocalSync::Ahead);
    assert_eq!(branch.rebase, RebaseNeed::Trunk);
    assert_eq!(branch.local_checks[0].state, LocalCheckState::Pass);
    assert!(!branch.local_checks[0].is_stale);
    let pr = branch.pr.as_ref().unwrap();
    assert!(pr.is_draft);
    assert_eq!(pr.check_summary().running, 1);
    assert_eq!(pr.check_summary().worst, CheckHealth::Failure);
    assert!(
        pr.reviews
            .iter()
            .any(|review| { review.author == "bob" && review.state == ReviewState::Pending })
    );
    assert_eq!(
        branch.issues(),
        vec![
            Issue::Draft,
            Issue::RebaseTrunk,
            Issue::FailingChecks(1),
            Issue::UnresolvedThreads(2),
            Issue::RunningChecks(1),
        ]
    );
    assert_eq!(branch.verdict(), Verdict::NoGo);
    assert_eq!(state.next_to_merge(), Some(branch));

    config.rebase.onto_trunk = false;
    let state = compose(Some(&git), Some(&snapshot), Some(&store), &config);
    assert_eq!(state.branches[0].rebase, RebaseNeed::Trunk);
    assert!(!state.branches[0].issues().contains(&Issue::RebaseTrunk));
}

#[test]
fn no_pr_composition_keeps_commit_local_evidence_sync_and_no_merge_candidate() {
    let mut snapshot = single_github();
    let StackOutcome::InStack(stack) = &mut snapshot.stack else {
        unreachable!();
    };
    stack.branches[0].pr = None;
    let mut git = git("api");
    git.current_sync = LocalSync::Unpushed;
    let mut config = Config::default();
    for name in ["lint", "test"] {
        config.checks.local.insert(
            name.into(),
            LocalCheckConfig {
                description: String::new(),
            },
        );
    }
    let mut old = local("api", &"b".repeat(40));
    old.checks.get_mut("lint").unwrap().state = ReportedState::Fail;
    let cases = [
        (
            vec![old.clone(), local("api", &"c".repeat(40))],
            LocalCheckState::Pass,
            false,
            vec![Issue::NoPr, Issue::MissingLocal(1)],
            Verdict::Wait,
        ),
        (
            vec![old],
            LocalCheckState::Fail,
            true,
            vec![
                Issue::LocalFailed(1),
                Issue::LocalStale(1),
                Issue::NoPr,
                Issue::MissingLocal(1),
            ],
            Verdict::NoGo,
        ),
        (
            vec![],
            LocalCheckState::Missing,
            false,
            vec![Issue::NoPr, Issue::MissingLocal(2)],
            Verdict::Wait,
        ),
    ];
    for (entries, expected_local, is_stale, issues, verdict) in cases {
        let store = StoreSnapshot {
            entries,
            warnings: vec![],
            fetched_at: now(),
        };

        let state = compose(Some(&git), Some(&snapshot), Some(&store), &config);

        assert_eq!(state.mode, StackMode::Single);
        assert_eq!(state.stack_status, StackStatus::Ready);
        assert_eq!(state.branches.len(), 1);
        let branch = &state.branches[0];
        assert!(branch.pr_number.is_none());
        assert!(branch.pr.is_none());
        assert!(branch.is_current);
        assert_eq!(branch.local_sync, LocalSync::Unpushed);
        assert_eq!(branch.head_sha, "c".repeat(40));
        assert_eq!(branch.local_checks.len(), 2);
        assert_eq!(branch.local_checks[0].state, expected_local);
        assert_eq!(branch.local_checks[0].is_stale, is_stale);
        assert_eq!(branch.local_checks[1].state, LocalCheckState::Missing);
        assert_eq!(branch.issues(), issues);
        assert_eq!(branch.verdict(), verdict);
        assert_eq!(state.summary().go, 0);
        assert!(state.next_to_merge().is_none());
    }
}

#[test]
fn genuine_one_entry_stack_stays_stacked_and_remains_a_merge_candidate() {
    let mut snapshot = github();
    let StackOutcome::InStack(stack) = &mut snapshot.stack else {
        unreachable!();
    };
    stack.branches.retain(|branch| branch.name == "api");

    let state = compose(Some(&git("api")), Some(&snapshot), None, &Config::default());

    assert_eq!(state.mode, StackMode::Stacked);
    assert_eq!(state.branches.len(), 1);
    assert_eq!(state.next_to_merge(), state.branches.first());
}

#[test]
fn composition_keeps_unresolved_threads_non_gating_by_default() {
    let state = compose(Some(&git("api")), Some(&github()), None, &Config::default());
    let pr = state.branches[1].pr.as_ref().unwrap();
    assert_eq!(pr.unresolved_threads, 2);
    assert!(!pr.threads_gate);
}

#[test]
fn composition_treats_missing_remote_comparison_as_no_rebase() {
    let state = compose(Some(&git("api")), Some(&github()), None, &Config::default());

    assert_eq!(state.branches[2].rebase, RebaseNeed::None);
}

#[test]
fn composition_treats_zero_remote_behind_by_as_no_rebase() {
    let mut snapshot = github();
    let raw = include_str!("fixtures/pr_details.json").replacen(
        "\"repository\": {",
        "\"repository\": {\"rebase1\": {\"compare\": {\"behindBy\": 0}},",
        1,
    );
    let details = {
        let StackOutcome::InStack(stack) = &snapshot.stack else {
            unreachable!();
        };
        parse_stack_response(&raw, stack).unwrap()
    };
    snapshot.details = details;

    let state = compose(Some(&git("api")), Some(&snapshot), None, &Config::default());

    assert_eq!(state.branches[2].rebase, RebaseNeed::None);
}

#[test]
fn composition_classifies_remote_rebase_targets_from_stack_order() {
    let mut snapshot = github();
    snapshot.details.remote_rebase_branches = BTreeSet::from(["api".into(), "ui".into()]);

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
    snapshot.details.remote_rebase_branches = BTreeSet::from(["api".into()]);
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
    stack.branches[1].is_merged = true;
    snapshot.details.remote_rebase_branches =
        BTreeSet::from(["auth".into(), "api".into(), "ui".into()]);
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
        details: Default::default(),
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
    github.details.prs.retain(|pr| pr.number != 43);
    let state = compose(Some(&git("api")), Some(&github), None, &Config::default());
    assert_eq!(state.branches[1].pr_number, Some(43));
    assert!(state.branches[1].pr.is_none());
    assert_eq!(state.branches[1].issues(), vec![Issue::LoadingDetails]);
    assert_eq!(state.branches[1].verdict(), Verdict::Wait);
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
    let pr = snapshot
        .details
        .prs
        .iter_mut()
        .find(|pr| pr.number == 43)
        .unwrap();
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
    let pr = snapshot
        .details
        .prs
        .iter_mut()
        .find(|pr| pr.number == 43)
        .unwrap();
    pr.checks.truncate(1);
    pr.checks[0].status = "COMPLETED".into();
    pr.checks[0].conclusion = Some("FUTURE_ERROR".into());
    let state = compose(Some(&git("api")), Some(&snapshot), None, &Config::default());
    assert_eq!(
        state.branches[1].pr.as_ref().unwrap().check_summary().worst,
        CheckHealth::Failure
    );
}
