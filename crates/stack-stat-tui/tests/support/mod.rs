#![allow(dead_code)]

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use chrono::{TimeZone, Utc};
use stack_stat_core::{
    config::{Config, LocalCheckConfig},
    providers::{
        CheckResult, FakeGitHubProvider, FakeGitProvider, MemoryStatusStore, PrCheckResponse,
        PrResponse, ProviderAction, ReportedState, ReviewResponse, StackBranch, StackPr,
        StackViewResponse, StatusStoreProvider,
    },
};
use stack_stat_tui::app::App;

pub type FakeApp = App<FakeGitProvider, FakeGitHubProvider, MemoryStatusStore>;

#[derive(Clone)]
pub struct Handles {
    pub git: FakeGitProvider,
    pub github: FakeGitHubProvider,
    pub store: MemoryStatusStore,
}

#[derive(Default)]
pub struct ConfigBuilder(Config);
impl ConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn skip_check(mut self, name: &str) -> Self {
        self.0.checks.skip.names.push(name.into());
        self
    }
    pub fn bot(mut self, name: &str) -> Self {
        self.0.reviews.bots.push(name.into());
        self
    }
    pub fn local(mut self, name: &str) -> Self {
        self.0.checks.local.insert(
            name.into(),
            LocalCheckConfig {
                description: name.into(),
            },
        );
        self
    }
    pub fn build(self) -> Config {
        self.0
    }
}

pub struct PrBuilder(PrResponse);
impl PrBuilder {
    pub fn new(number: u32) -> Self {
        Self(PrResponse {
            number,
            url: format!("https://example.test/pr/{number}"),
            title: format!("Change {number}"),
            is_draft: false,
            review_decision: None,
            reviews: vec![],
            review_requests: vec![],
            checks: vec![],
        })
    }
    pub fn draft(mut self) -> Self {
        self.0.is_draft = true;
        self
    }
    pub fn ready(mut self) -> Self {
        self.0.is_draft = false;
        self
    }
    pub fn approved(mut self) -> Self {
        self.0.review_decision = Some("APPROVED".into());
        self
    }
    pub fn pending_review(mut self) -> Self {
        self.0
            .review_requests
            .push(stack_stat_core::providers::ReviewRequest {
                author: "alice".into(),
                is_bot: false,
            });
        self
    }
    pub fn changes_requested(mut self) -> Self {
        self.0.reviews.push(ReviewResponse {
            author: "bob".into(),
            state: "CHANGES_REQUESTED".into(),
            is_bot: false,
            submitted_at: None,
        });
        self
    }
    pub fn bot_review(mut self, login: &str) -> Self {
        self.0.reviews.push(ReviewResponse {
            author: login.into(),
            state: "APPROVED".into(),
            is_bot: true,
            submitted_at: None,
        });
        self
    }
    pub fn check(mut self, name: &str, status: &str, conclusion: Option<&str>) -> Self {
        self.0.checks.push(PrCheckResponse {
            name: name.into(),
            workflow: "CI".into(),
            status: status.into(),
            conclusion: conclusion.map(str::to_owned),
            started_at: None,
            completed_at: None,
            details_url: "https://example.test/check".into(),
        });
        self
    }
    pub fn checks_all_pass(self) -> Self {
        self.check("ci/build", "COMPLETED", Some("SUCCESS"))
    }
    pub fn build(self) -> PrResponse {
        self.0
    }
}

pub struct TestScenario {
    trunk: String,
    branches: Vec<String>,
    prs: Vec<(String, PrResponse)>,
    current_branch: String,
    repo: String,
    local: Vec<(String, String, String, ReportedState)>,
    config: Config,
}
impl TestScenario {
    pub fn builder() -> Self {
        Self {
            trunk: "main".into(),
            branches: vec![],
            prs: vec![],
            current_branch: String::new(),
            local: vec![],
            repo: String::new(),
            config: Config::default(),
        }
    }
    pub fn stack(mut self, trunk: &str, branches: &[&str]) -> Self {
        self.trunk = trunk.into();
        self.branches = branches.iter().map(|s| (*s).into()).collect();
        self
    }
    pub fn pr(mut self, branch: &str, pr: PrBuilder) -> Self {
        self.prs.push((branch.into(), pr.build()));
        self
    }
    pub fn current_branch(mut self, branch: &str) -> Self {
        self.current_branch = branch.into();
        self
    }
    pub fn repo(mut self, repo: &str) -> Self {
        self.repo = repo.into();
        self
    }
    pub fn local_check(
        mut self,
        branch: &str,
        sha: &str,
        check: &str,
        state: ReportedState,
    ) -> Self {
        self.local
            .push((branch.into(), sha.into(), check.into(), state));
        self
    }
    pub fn config(mut self, builder: ConfigBuilder) -> Self {
        self.config = builder.build();
        self
    }
    pub fn build(self) -> Self {
        self
    }
    pub fn into_app(self) -> (FakeApp, Handles) {
        let shas: HashMap<String, String> = self
            .branches
            .iter()
            .enumerate()
            .map(|(i, name)| (name.clone(), format!("{:040x}", i + 1)))
            .collect();
        let stack = StackViewResponse {
            trunk: self.trunk,
            current_branch: self.current_branch.clone(),
            branches: self
                .branches
                .iter()
                .map(|name| StackBranch {
                    name: name.clone(),
                    head: shas.get(name).cloned(),
                    base: "0".repeat(40),
                    is_current: name == &self.current_branch,
                    is_merged: false,
                    is_queued: false,
                    needs_rebase: false,
                    pr: self
                        .prs
                        .iter()
                        .find(|(branch, _)| branch == name)
                        .map(|(_, pr)| StackPr {
                            number: pr.number,
                            url: pr.url.clone(),
                            state: "OPEN".into(),
                        }),
                })
                .collect(),
        };
        let github =
            FakeGitHubProvider::new(stack, self.prs.into_iter().map(|(_, pr)| pr).collect());
        github.set_repo(self.repo);
        let git = FakeGitProvider::new(
            &self.current_branch,
            shas,
            PathBuf::from("/tmp/stack-stat-test"),
        );
        let store = MemoryStatusStore::default();
        for (branch, sha, name, state) in self.local {
            store
                .write(
                    &branch,
                    &sha,
                    &name,
                    &CheckResult {
                        state,
                        message: None,
                        recorded_at: Utc.timestamp_opt(1_760_000_000, 0).single().unwrap(),
                    },
                )
                .unwrap();
        }
        let handles = Handles {
            git: git.clone(),
            github: github.clone(),
            store: store.clone(),
        };
        let app = App::new(
            Arc::new(git),
            Arc::new(github),
            Arc::new(store),
            self.config,
        );
        (app, handles)
    }
}

pub fn assert_cell(buffer: &ratatui::buffer::Buffer, x: u16, y: u16, expected: &str) {
    assert_eq!(buffer[(x, y)].symbol(), expected);
}
pub fn find_row(buffer: &ratatui::buffer::Buffer, text: &str) -> Option<u16> {
    (0..buffer.area.height).find(|&y| {
        (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>()
            .contains(text)
    })
}
pub fn assert_row_contains(buffer: &ratatui::buffer::Buffer, row: u16, parts: &[&str]) {
    let line = (0..buffer.area.width)
        .map(|x| buffer[(x, row)].symbol())
        .collect::<String>();
    let mut offset = 0;
    for part in parts {
        let found = line[offset..]
            .find(part)
            .unwrap_or_else(|| panic!("'{part}' not found in row: {line}"));
        offset += found + part.len();
    }
}

pub fn stack_order_actions(actions: &[ProviderAction]) -> Vec<ProviderAction> {
    actions.to_vec()
}
