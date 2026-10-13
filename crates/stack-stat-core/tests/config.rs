use std::{fs, path::Path};

use stack_stat_core::config::{Config, ReviewsDefault};

fn write(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
}

#[test]
fn absent_files_use_defaults() {
    let repo = tempfile::tempdir().unwrap();
    let missing_user = repo.path().join("missing.toml");
    let config = Config::load(repo.path(), Some(&missing_user)).unwrap();
    assert_eq!(config.poll.git, 2);
    assert_eq!(config.poll.github, 30);
    assert_eq!(config.poll.github_active, 15);
    assert_eq!(config.poll.store, 2);
    assert!(!config.reviews.show_bots);
    assert!(!config.reviews.gate_on_unresolved_threads);
    assert_eq!(config.display.reviews_default, ReviewsDefault::Collapsed);
    assert!(config.checks.local.is_empty());
}

#[test]
fn repo_filters_and_local_checks_are_loaded() {
    let repo = tempfile::tempdir().unwrap();
    write(
        &repo.path().join(".stack-stat.toml"),
        "[checks.skip]\nnames = ['secrets-scan']\npatterns = ['codecov/*', 'test-?']\n\
         [checks.local]\nlint = { description = 'cargo clippy' }\n\
         [reviews]\nbots = ['check-bot']\nshow_bots = true\ngate_on_unresolved_threads = true\n",
    );
    let config = Config::load(repo.path(), None).unwrap();
    assert!(config.is_check_skipped("secrets-scan"));
    assert!(config.is_check_skipped("codecov/patch"));
    assert!(config.is_check_skipped("test-a"));
    assert!(!config.is_check_skipped("test-ab"));
    assert!(!config.is_check_skipped("codecov"));
    assert!(config.is_bot("check-bot"));
    assert_eq!(config.checks.local["lint"].description, "cargo clippy");
    assert!(config.reviews.show_bots);
    assert!(config.reviews.gate_on_unresolved_threads);
}

#[test]
fn personal_preferences_cannot_override_repo_filters() {
    let repo = tempfile::tempdir().unwrap();
    write(
        &repo.path().join(".stack-stat.toml"),
        "[checks.skip]\nnames = ['required-check']\n[reviews]\nbots = ['ci-bot']\ngate_on_unresolved_threads = true\n",
    );
    let user = repo.path().join("config.toml");
    write(
        &user,
        "[checks.skip]\nnames = []\n[checks.local]\nother = { description = 'local' }\n\
         [reviews]\nbots = []\ngate_on_unresolved_threads = false\n[poll]\ngithub = 60\n\
         [display]\nreviews_default = 'expanded'\n",
    );
    let config = Config::load(repo.path(), Some(&user)).unwrap();
    assert!(config.is_check_skipped("required-check"));
    assert!(config.is_bot("ci-bot"));
    assert!(config.reviews.gate_on_unresolved_threads);
    assert!(config.checks.local.is_empty());
    assert_eq!(config.poll.github, 60);
    assert_eq!(config.display.reviews_default, ReviewsDefault::Expanded);
    assert_eq!(config.warnings.len(), 3);
}

#[test]
fn malformed_and_unknown_keys_name_the_file() {
    let repo = tempfile::tempdir().unwrap();
    let path = repo.path().join(".stack-stat.toml");
    write(&path, "[checks]\nunknown = true\n");
    let err = Config::load(repo.path(), None).unwrap_err().to_string();
    assert!(err.contains(".stack-stat.toml"), "{err}");
    assert!(err.contains("unknown"), "{err}");
    write(&path, "[invalid\n");
    let err = Config::load(repo.path(), None).unwrap_err().to_string();
    assert!(err.contains(".stack-stat.toml"), "{err}");
}

#[test]
fn zero_poll_interval_is_rejected() {
    let repo = tempfile::tempdir().unwrap();
    write(&repo.path().join(".stack-stat.toml"), "[poll]\nstore = 0\n");
    let err = Config::load(repo.path(), None).unwrap_err().to_string();
    assert!(err.contains("store"), "{err}");
}
