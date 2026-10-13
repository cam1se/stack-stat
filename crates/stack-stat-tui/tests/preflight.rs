use std::{io, sync::Mutex};

use stack_stat_tui::preflight::{CommandOutput, CommandRunner, PreflightError, run};

#[derive(Default)]
struct FakeRunner {
    calls: Mutex<Vec<String>>,
    missing: Option<&'static str>,
    auth_ok: bool,
    repo_ok: bool,
}

impl CommandRunner for FakeRunner {
    fn run(&self, program: &str, args: &[&str]) -> io::Result<CommandOutput> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{program} {}", args.join(" ")));
        if self.missing == Some(program) {
            return Err(io::Error::new(io::ErrorKind::NotFound, "missing"));
        }
        let output = match (program, args) {
            ("gh", ["--version"]) => CommandOutput { success: true },
            ("gh", ["auth", "status"]) => CommandOutput {
                success: self.auth_ok,
            },
            ("git", ["rev-parse", "--show-toplevel"]) => CommandOutput {
                success: self.repo_ok,
            },
            _ => panic!("unexpected: {program} {args:?}"),
        };
        Ok(output)
    }
}

#[test]
fn preflight_reports_first_missing_prerequisite_and_stops() {
    let fake = FakeRunner {
        missing: Some("gh"),
        ..FakeRunner::default()
    };
    let error = run(&fake).unwrap_err();
    assert_eq!(error, PreflightError::MissingGh);
    assert!(error.to_string().contains("https://cli.github.com/"));
    assert_eq!(*fake.calls.lock().unwrap(), ["gh --version"]);
}

#[test]
fn preflight_requires_auth_and_git_repository() {
    let auth = FakeRunner {
        auth_ok: false,
        ..FakeRunner::default()
    };
    assert!(matches!(run(&auth), Err(PreflightError::Unauthenticated)));
    let missing_repo = FakeRunner {
        auth_ok: true,
        ..FakeRunner::default()
    };
    assert!(matches!(
        run(&missing_repo),
        Err(PreflightError::NotGitRepository)
    ));
}

#[test]
fn preflight_succeeds_without_gh_stack_and_never_lists_extensions() {
    let runner = FakeRunner {
        auth_ok: true,
        repo_ok: true,
        ..FakeRunner::default()
    };

    run(&runner).unwrap();

    let calls = runner.calls.lock().unwrap();
    assert!(!calls.iter().any(|call| call == "gh extension list"));
    assert_eq!(
        *calls,
        [
            "gh --version",
            "gh auth status",
            "git rev-parse --show-toplevel"
        ]
    );
}
