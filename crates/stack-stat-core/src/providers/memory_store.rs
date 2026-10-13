//! Shared in-memory local status store for deterministic tests.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::Result;

use super::{CheckResult, StatusEntry, StatusStoreProvider, retained};

/// Cloneable fake check store with the same retention rule as the file store.
#[derive(Clone, Debug, Default)]
pub struct MemoryStatusStore {
    entries: Arc<Mutex<HashMap<(String, String), StatusEntry>>>,
}

impl StatusStoreProvider for MemoryStatusStore {
    fn read_all(&self) -> Result<Vec<StatusEntry>> {
        let mut entries: Vec<_> = self
            .entries
            .lock()
            .expect("fake store mutex poisoned")
            .values()
            .cloned()
            .collect();
        entries.sort_by(|a, b| (&a.branch, &a.sha).cmp(&(&b.branch, &b.sha)));
        Ok(entries)
    }

    fn write(&self, branch: &str, sha: &str, check: &str, result: &CheckResult) -> Result<()> {
        let mut entries = self.entries.lock().expect("fake store mutex poisoned");
        entries
            .entry((branch.into(), sha.into()))
            .or_insert_with(|| StatusEntry {
                branch: branch.into(),
                sha: sha.into(),
                checks: Default::default(),
            })
            .checks
            .insert(check.into(), result.clone());
        Ok(())
    }

    fn clear(&self, branch: &str) -> Result<()> {
        self.entries
            .lock()
            .expect("fake store mutex poisoned")
            .retain(|(name, _), _| name != branch);
        Ok(())
    }

    fn gc(&self, live_branches: &HashMap<String, String>) -> Result<usize> {
        let mut store = self.entries.lock().expect("fake store mutex poisoned");
        let entries = std::mem::take(&mut *store).into_values().collect();
        let (keep, delete) = retained(entries, live_branches);
        for entry in keep {
            store.insert((entry.branch.clone(), entry.sha.clone()), entry);
        }
        Ok(delete.len())
    }
}
