use std::{
    path::Path,
    process::{Command, Output},
};

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn repo() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    git(temp.path(), &["init", "-b", "main"]);
    git(
        temp.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "--allow-empty",
            "-m",
            "first",
        ],
    );
    git(
        temp.path(),
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "--allow-empty",
            "-m",
            "second",
        ],
    );
    temp
}

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stack-stat"))
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap()
}

#[test]
fn report_writes_lists_and_clears_real_git_common_store() {
    let temp = repo();
    let result = run(
        temp.path(),
        &["report", "lint", "pass", "--message", "clean"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output = run(temp.path(), &["report", "--list", "--json"]);
    assert!(output.status.success());
    let entries: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(entries[0]["branch"], "main");
    assert_eq!(entries[0]["checks"]["lint"]["state"], "pass");
    assert_eq!(entries[0]["checks"]["lint"]["message"], "clean");
    assert_eq!(entries[0]["stale"], false);
    assert!(run(temp.path(), &["report", "--clear"]).status.success());
    let output = run(temp.path(), &["report", "--list", "--json"]);
    let entries: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(entries.as_array().unwrap().len(), 0);
}

#[test]
fn branch_override_and_revision_resolve_to_full_sha_and_stale_entry() {
    let temp = repo();
    git(temp.path(), &["branch", "feat/api"]);
    let first = git(temp.path(), &["rev-parse", "HEAD~1"]);
    let status = run(
        temp.path(),
        &[
            "report", "test", "fail", "--branch", "feat/api", "--sha", "HEAD~1",
        ],
    );
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let output = run(temp.path(), &["report", "--list", "--json"]);
    let entries: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(entries[0]["branch"], "feat/api");
    assert_eq!(entries[0]["sha"], first);
    assert_eq!(entries[0]["stale"], true);
    let other = run(
        temp.path(),
        &["report", "lint", "pass", "--branch", "feat/api"],
    );
    assert!(other.status.success());
    let entries: serde_json::Value =
        serde_json::from_slice(&run(temp.path(), &["report", "--list", "--json"]).stdout).unwrap();
    assert_eq!(entries.as_array().unwrap().len(), 2);
    assert!(
        entries
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["stale"] == false)
    );
}

#[test]
fn detached_head_without_branch_is_an_error_and_conflicting_flags_exit_two() {
    let temp = repo();
    git(temp.path(), &["checkout", "--detach", "HEAD"]);
    let status = run(temp.path(), &["report", "lint", "pass"]);
    assert_eq!(status.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&status.stderr).contains("detached"));
    let status = run(temp.path(), &["report", "--list", "--clear"]);
    assert_eq!(status.status.code(), Some(2));
    let status = run(temp.path(), &["report", "lint", "pass", "--list"]);
    assert_eq!(status.status.code(), Some(2));
}

#[test]
fn report_and_version_skip_gh_startup_checks() {
    let temp = repo();
    let status = run(temp.path(), &["version"]);
    assert!(status.status.success());
    assert!(String::from_utf8_lossy(&status.stdout).contains("stack-stat"));
    // The temp repo has no remote or gh stack, but reporting remains usable.
    assert!(
        run(temp.path(), &["report", "lint", "pass"])
            .status
            .success()
    );
}
