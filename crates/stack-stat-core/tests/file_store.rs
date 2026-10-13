use std::{collections::HashMap, fs, sync::Arc, thread};

use chrono::{TimeZone, Utc};
use stack_stat_core::providers::{
    CheckResult, FileStatusStore, ReportedState, StatusStoreProvider, branch_dir,
};

fn result(secs: i64, state: ReportedState) -> CheckResult {
    CheckResult {
        state,
        message: None,
        recorded_at: Utc.timestamp_opt(secs, 0).single().unwrap(),
    }
}

#[test]
fn slash_branch_round_trips_and_two_checks_merge_into_one_file() {
    let temp = tempfile::tempdir().unwrap();
    let store = FileStatusStore::new(temp.path());
    let sha = "a".repeat(40);
    store
        .write("feat/api", &sha, "lint", &result(1, ReportedState::Pass))
        .unwrap();
    store
        .write("feat/api", &sha, "test", &result(2, ReportedState::Fail))
        .unwrap();
    assert_eq!(
        branch_dir::decode(&branch_dir::encode("feat/api")).unwrap(),
        "feat/api"
    );
    assert!(!store.root().join("feat").exists());
    let entries = store.read_all().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].branch, "feat/api");
    assert_eq!(entries[0].checks.len(), 2);
    assert_eq!(entries[0].checks["test"].state, ReportedState::Fail);
    store.clear("feat/api").unwrap();
    assert!(store.read_all().unwrap().is_empty());
}

#[test]
fn corrupt_entry_warns_without_hiding_valid_entry() {
    let temp = tempfile::tempdir().unwrap();
    let store = FileStatusStore::new(temp.path());
    store
        .write(
            "auth",
            &"a".repeat(40),
            "lint",
            &result(1, ReportedState::Pass),
        )
        .unwrap();
    let dir = store.root().join(branch_dir::encode("auth"));
    fs::write(dir.join(format!("{}.json", "b".repeat(40))), "broken json").unwrap();
    let (entries, warnings) = store.read_all_with_warnings().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains(".json"));
}

#[test]
fn gc_retains_tip_and_newest_prior_file_and_removes_deleted_branch() {
    let temp = tempfile::tempdir().unwrap();
    let store = FileStatusStore::new(temp.path());
    for (sha, secs) in [("a", 1), ("b", 2), ("c", 3)] {
        store
            .write(
                "auth",
                &sha.repeat(40),
                "lint",
                &result(secs, ReportedState::Pass),
            )
            .unwrap();
    }
    store
        .write(
            "gone",
            &"d".repeat(40),
            "lint",
            &result(4, ReportedState::Pass),
        )
        .unwrap();
    let live = HashMap::from([("auth".into(), "c".repeat(40))]);
    assert_eq!(store.gc(&live).unwrap(), 2);
    let entries = store.read_all().unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|e| e.sha == "b".repeat(40)));
    assert!(entries.iter().any(|e| e.sha == "c".repeat(40)));
    assert!(!store.root().join(branch_dir::encode("gone")).exists());
}

#[test]
fn concurrent_reader_never_observes_partial_write() {
    let temp = tempfile::tempdir().unwrap();
    let store = Arc::new(FileStatusStore::new(temp.path()));
    let sha = "a".repeat(40);
    store
        .write("auth", &sha, "lint", &result(0, ReportedState::Pass))
        .unwrap();
    let reader = Arc::clone(&store);
    let handle = thread::spawn(move || {
        for _ in 0..200 {
            let (entries, warnings) = reader.read_all_with_warnings().unwrap();
            assert!(warnings.is_empty(), "{warnings:?}");
            assert_eq!(entries.len(), 1);
            assert!(entries[0].checks.contains_key("lint"));
        }
    });
    for n in 1..=200 {
        store
            .write("auth", &sha, "lint", &result(n, ReportedState::Pass))
            .unwrap();
    }
    handle.join().unwrap();
}
