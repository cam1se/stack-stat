//! Test-controlled Git state for deterministic App scenarios.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::{Result, anyhow};
use async_trait::async_trait;

use super::GitProvider;

/// A cloneable fake for current branch and branch ref updates.
#[derive(Clone, Debug)]
pub struct FakeGitProvider {
    state: Arc<Mutex<FakeGitState>>,
    common_dir: PathBuf,
}

/// Git values changed by a test between poll cycles.
#[derive(Debug)]
pub struct FakeGitState {
    /// Current worktree branch.
    pub current_branch: String,
    /// Branch tip SHA by name.
    pub branches: HashMap<String, String>,
    /// Error returned once by the next query.
    pub fail_next: Option<String>,
}

impl FakeGitProvider {
    /// Create a fake with a shared common-dir identity.
    #[must_use]
    pub fn new(branch: &str, branches: HashMap<String, String>, common_dir: PathBuf) -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeGitState {
                current_branch: branch.into(),
                branches,
                fail_next: None,
            })),
            common_dir,
        }
    }

    /// Simulate the agent checking out a branch.
    pub fn set_current_branch(&self, branch: &str) {
        self.state
            .lock()
            .expect("fake Git mutex poisoned")
            .current_branch = branch.into();
    }

    /// Change a branch's tip SHA.
    pub fn set_branch_sha(&self, branch: &str, sha: &str) {
        self.state
            .lock()
            .expect("fake Git mutex poisoned")
            .branches
            .insert(branch.into(), sha.into());
    }

    /// Return an error once on the next query.
    pub fn fail_next(&self, message: &str) {
        self.state
            .lock()
            .expect("fake Git mutex poisoned")
            .fail_next = Some(message.into());
    }
}

#[async_trait]
impl GitProvider for FakeGitProvider {
    async fn current_branch(&self) -> Result<String> {
        let mut state = self.state.lock().expect("fake Git mutex poisoned");
        if let Some(message) = state.fail_next.take() {
            return Err(anyhow!(message));
        }
        Ok(state.current_branch.clone())
    }

    async fn branch_shas(&self) -> Result<HashMap<String, String>> {
        let mut state = self.state.lock().expect("fake Git mutex poisoned");
        if let Some(message) = state.fail_next.take() {
            return Err(anyhow!(message));
        }
        Ok(state.branches.clone())
    }

    fn common_dir(&self) -> &Path {
        &self.common_dir
    }
}
