use std::{io, sync::Mutex};

use stack_stat_tui::preflight::{CommandOutput, CommandRunner, PreflightError, run};

#[derive(Default)]
struct FakeRunner {
    calls: Mutex<Vec<String>>,
    missing: Option<&'static str>,
    auth_ok: bool,
    extension: &'static str,
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
            ("gh", ["--version"]) => CommandOutput {
                success: true,
                stdout: "gh version 2".into(),
            },
            ("gh", ["auth", "status"]) => CommandOutput {
                success: self.auth_ok,
                stdout: String::new(),
            },
            ("gh", ["extension", "list"]) => CommandOutput {
                success: true,
                stdout: self.extension.into(),
            },
            ("git", ["rev-parse", "--show-toplevel"]) => CommandOutput {
                success: self.repo_ok,
                stdout: "/tmp/repo".into(),
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
    assert!(matches!(run(&fake), Err(PreflightError::MissingGh)));
    assert!(
        run(&fake)
            .unwrap_err()
            .to_string()
            .contains("https://cli.github.com/")
    );
    assert_eq!(fake.calls.lock().unwrap()[0], "gh --version");
    assert_eq!(fake.calls.lock().unwrap().len(), 2);
}

#[test]
fn preflight_requires_auth_extension_and_git_repository() {
    let auth = FakeRunner {
        auth_ok: false,
        ..FakeRunner::default()
    };
    assert!(matches!(run(&auth), Err(PreflightError::Unauthenticated)));
    let missing_extension = FakeRunner {
        auth_ok: true,
        extension: "gh dash\tother\tv1\n",
        ..FakeRunner::default()
    };
    assert!(matches!(
        run(&missing_extension),
        Err(PreflightError::MissingExtension)
    ));
    let missing_repo = FakeRunner {
        auth_ok: true,
        extension: "gh stack\tgithub/gh-stack\tv0.1.1\n",
        ..FakeRunner::default()
    };
    assert!(matches!(
        run(&missing_repo),
        Err(PreflightError::NotGitRepository)
    ));
    let ok = FakeRunner {
        repo_ok: true,
        auth_ok: true,
        extension: "gh stack\tgithub/gh-stack\tv0.1.1\n",
        ..FakeRunner::default()
    };
    run(&ok).unwrap();
    assert_eq!(ok.calls.lock().unwrap().len(), 4);
}
