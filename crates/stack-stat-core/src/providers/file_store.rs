//! File-backed local check results under the Git common directory.

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use super::{CheckResult, StatusEntry, StatusStoreProvider, branch_dir, retained};

#[derive(Default, Deserialize, Serialize)]
struct StoredFile {
    checks: BTreeMap<String, CheckResult>,
}

/// Status store shared by all worktrees of a repository.
#[derive(Debug, Clone)]
pub struct FileStatusStore {
    root: PathBuf,
}

impl FileStatusStore {
    /// Use `<common-dir>/stack-stat/status` as the storage root.
    #[must_use]
    pub fn new(common_dir: &Path) -> Self {
        Self {
            root: common_dir.join("stack-stat").join("status"),
        }
    }

    /// Directory containing encoded branch subdirectories.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Read valid entries while returning warnings for unreadable or malformed files.
    ///
    /// A single bad status file never hides results for other branches or commits.
    pub fn read_all_with_warnings(&self) -> Result<(Vec<StatusEntry>, Vec<String>)> {
        let mut entries = Vec::new();
        let mut warnings = Vec::new();
        let branches = match fs::read_dir(&self.root) {
            Ok(branches) => branches,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok((entries, warnings));
            }
            Err(err) => {
                return Err(err).with_context(|| format!("reading {}", self.root.display()));
            }
        };
        for branch_result in branches {
            let branch = match branch_result {
                Ok(entry) => entry,
                Err(err) => {
                    warnings.push(err.to_string());
                    continue;
                }
            };
            let path = branch.path();
            let Ok(kind) = branch.file_type() else {
                warnings.push(format!("unreadable {}", path.display()));
                continue;
            };
            if !kind.is_dir() {
                continue;
            }
            let branch_name = match branch_dir::decode(&branch.file_name().to_string_lossy()) {
                Ok(name) => name,
                Err(err) => {
                    warnings.push(err.to_string());
                    continue;
                }
            };
            let files = match fs::read_dir(&path) {
                Ok(files) => files,
                Err(err) => {
                    warnings.push(format!("{}: {err}", path.display()));
                    continue;
                }
            };
            for file_result in files {
                let file = match file_result {
                    Ok(entry) => entry,
                    Err(err) => {
                        warnings.push(err.to_string());
                        continue;
                    }
                };
                let path = file.path();
                if path.extension().is_none_or(|extension| extension != "json") {
                    continue;
                }
                let Some(sha) = path.file_stem().and_then(|name| name.to_str()) else {
                    warnings.push(format!("invalid file name {}", path.display()));
                    continue;
                };
                match fs::read(&path).and_then(|bytes| {
                    serde_json::from_slice::<StoredFile>(&bytes).map_err(std::io::Error::other)
                }) {
                    Ok(data) => entries.push(StatusEntry {
                        branch: branch_name.clone(),
                        sha: sha.into(),
                        checks: data.checks,
                    }),
                    Err(err) => warnings.push(format!("{}: {err}", path.display())),
                }
            }
        }
        entries.sort_by(|a, b| (&a.branch, &a.sha).cmp(&(&b.branch, &b.sha)));
        Ok((entries, warnings))
    }

    fn file_path(&self, branch: &str, sha: &str) -> PathBuf {
        self.root
            .join(branch_dir::encode(branch))
            .join(format!("{sha}.json"))
    }
}

impl StatusStoreProvider for FileStatusStore {
    fn read_all(&self) -> Result<Vec<StatusEntry>> {
        self.read_all_with_warnings().map(|(entries, _)| entries)
    }

    fn write(&self, branch: &str, sha: &str, check: &str, result: &CheckResult) -> Result<()> {
        // SHA is used as a filename, so reject anything other than a full commit ID.
        anyhow::ensure!(
            sha.len() == 40 && sha.bytes().all(|ch| ch.is_ascii_hexdigit()),
            "expected full 40-hex commit SHA"
        );
        let path = self.file_path(branch, sha);
        let parent = path
            .parent()
            .expect("file path always has an encoded branch parent");
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        let mut data: StoredFile = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => StoredFile::default(),
            Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
        };
        data.checks.insert(check.into(), result.clone());
        let mut temp = NamedTempFile::new_in(parent).context("creating temporary check file")?;
        serde_json::to_writer(&mut temp, &data).context("serializing check result")?;
        temp.as_file().sync_all().context("syncing check result")?;
        temp.persist(&path)
            .with_context(|| format!("replacing {}", path.display()))?;
        Ok(())
    }

    fn clear(&self, branch: &str) -> Result<()> {
        let dir = self.root.join(branch_dir::encode(branch));
        match fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err).with_context(|| format!("removing {}", dir.display())),
        }
    }

    fn gc(&self, live_branches: &HashMap<String, String>) -> Result<usize> {
        let (entries, _) = self.read_all_with_warnings()?;
        let (_, deleted) = retained(entries, live_branches);
        let count = deleted.len();
        for entry in deleted {
            let path = self.file_path(&entry.branch, &entry.sha);
            fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
        }
        if let Ok(branches) = fs::read_dir(&self.root) {
            for branch in branches {
                let branch = branch?;
                if !branch.file_type()?.is_dir() {
                    continue;
                }
                let name = branch_dir::decode(&branch.file_name().to_string_lossy())?;
                if !live_branches.contains_key(&name) {
                    fs::remove_dir_all(branch.path())
                        .with_context(|| format!("removing branch {name}"))?;
                } else if fs::read_dir(branch.path())?.next().is_none() {
                    fs::remove_dir(branch.path())?;
                }
            }
        }
        Ok(count)
    }
}
