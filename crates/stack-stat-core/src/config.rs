//! Repository filters and user display preferences.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// Combined, validated settings for the current repository and user.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    /// Repository-level CI and local check settings.
    pub checks: ChecksConfig,
    /// Repository-level reviewer display defaults.
    pub reviews: ReviewsConfig,
    /// Repository-level rebase readiness policy.
    pub rebase: RebaseConfig,
    /// Effective refresh intervals.
    pub poll: PollConfig,
    /// User-specific presentation preferences.
    pub display: DisplayConfig,
    /// Non-fatal ignored user overrides.
    pub warnings: Vec<String>,
}

/// CI filters and expected local checks.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ChecksConfig {
    /// Exact and glob-style names excluded from aggregates.
    pub skip: SkipChecks,
    /// Named expected checks, displayed as missing until reported.
    pub local: BTreeMap<String, LocalCheckConfig>,
}

/// Exact and pattern-based GitHub check exclusion rules.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct SkipChecks {
    /// Exact CI check names.
    pub names: Vec<String>,
    /// Glob patterns in which `*` matches any span and `?` one character.
    pub patterns: Vec<String>,
}

/// Description for a check supplied by an external reporter.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LocalCheckConfig {
    /// Human-readable check purpose, not an executable command.
    pub description: String,
}

/// Known automated reviewers and initial visibility.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct ReviewsConfig {
    /// Review logins the repository identifies as bots.
    pub bots: Vec<String>,
    /// Whether unresolved review threads block readiness.
    pub gate_on_unresolved_threads: bool,
    /// Whether bot reviews are visible before toggling them.
    pub show_bots: bool,
}

/// Repository policy for whether trunk drift blocks readiness.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct RebaseConfig {
    /// Whether a layer behind trunk blocks readiness.
    pub onto_trunk: bool,
}

/// Refresh cadence in whole seconds.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct PollConfig {
    /// Git branch/commit refresh seconds.
    pub git: u64,
    /// Idle GitHub API refresh seconds.
    pub github: u64,
    /// Local status store refresh seconds.
    pub store: u64,
    /// GitHub API refresh seconds while checks are running.
    pub github_active: u64,
}

impl Default for PollConfig {
    fn default() -> Self {
        Self {
            git: 2,
            github: 30,
            store: 2,
            github_active: 15,
        }
    }
}

/// User-specific presentation settings.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct DisplayConfig {
    /// How reviewers appear initially in the branch table.
    pub reviews_default: ReviewsDefault,
}

/// Initial density of the review column.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewsDefault {
    /// One aggregate status per PR.
    #[default]
    Collapsed,
    /// Each reviewer appears inline.
    Expanded,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RepoFile {
    checks: ChecksConfig,
    reviews: ReviewsConfig,
    rebase: RebaseConfig,
    poll: PollConfig,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct UserFile {
    checks: Option<ChecksConfig>,
    reviews: Option<ReviewsConfig>,
    rebase: Option<RebaseConfig>,
    poll: Option<UserPoll>,
    display: DisplayConfig,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct UserPoll {
    git: Option<u64>,
    github: Option<u64>,
    store: Option<u64>,
    github_active: Option<u64>,
}

impl Config {
    /// Load the repository config and optional personal file, applying allowed overrides.
    ///
    /// A missing file is harmless. Invalid or unknown keys include their path in the error.
    pub fn load(repo_root: &Path, user_config_path: Option<&Path>) -> Result<Self> {
        let repo_path = repo_root.join(".stack-stat.toml");
        let repo: RepoFile = read_toml(&repo_path)?.unwrap_or_default();
        let user_path = user_config_path
            .map(Path::to_path_buf)
            .or_else(Self::user_config_path);
        let user: UserFile = match user_path.as_deref() {
            Some(path) => read_toml(path)?.unwrap_or_default(),
            None => UserFile::default(),
        };

        let mut config = Self {
            checks: repo.checks,
            reviews: repo.reviews,
            rebase: repo.rebase,
            poll: repo.poll,
            display: user.display,
            warnings: Vec::new(),
        };
        if let Some(checks) = user.checks {
            if !checks.skip.names.is_empty()
                || !checks.skip.patterns.is_empty()
                || has_skip_key(user_path.as_deref())?
            {
                config
                    .warnings
                    .push("user [checks.skip] is ignored; set filters in the repository".into());
            }
            if !checks.local.is_empty() {
                config.warnings.push(
                    "user [checks.local] is ignored; declare checks in the repository".into(),
                );
            }
        }
        if user.reviews.is_some() {
            config
                .warnings
                .push("user [reviews] is ignored; the repository sets review defaults".into());
        }
        if user.rebase.is_some() {
            config
                .warnings
                .push("user [rebase] is ignored; the repository sets rebase policy".into());
        }
        if let Some(poll) = user.poll {
            if let Some(value) = poll.git {
                config.poll.git = value;
            }
            if let Some(value) = poll.github {
                config.poll.github = value;
            }
            if let Some(value) = poll.store {
                config.poll.store = value;
            }
            if let Some(value) = poll.github_active {
                config.poll.github_active = value;
            }
        }
        for (key, value) in [
            ("git", config.poll.git),
            ("github", config.poll.github),
            ("store", config.poll.store),
            ("github_active", config.poll.github_active),
        ] {
            if value == 0 {
                bail!("poll.{key} must be greater than zero");
            }
        }
        Ok(config)
    }

    /// Locate personal configuration, respecting XDG on Unix and APPDATA on Windows.
    #[must_use]
    pub fn user_config_path() -> Option<PathBuf> {
        if let Some(root) = std::env::var_os("XDG_CONFIG_HOME") {
            return Some(PathBuf::from(root).join("stack-stat/config.toml"));
        }
        #[cfg(windows)]
        if let Some(root) = std::env::var_os("APPDATA") {
            return Some(PathBuf::from(root).join("stack-stat/config.toml"));
        }
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".config/stack-stat/config.toml"))
    }

    /// Whether a GitHub check is excluded from summary counts.
    #[must_use]
    pub fn is_check_skipped(&self, name: &str) -> bool {
        self.checks.skip.names.iter().any(|exact| exact == name)
            || self
                .checks
                .skip
                .patterns
                .iter()
                .any(|pattern| glob_matches(pattern, name))
    }

    /// Whether the repository designates a reviewer login as automated.
    #[must_use]
    pub fn is_bot(&self, login: &str) -> bool {
        self.reviews.bots.iter().any(|bot| bot == login)
    }
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>> {
    match fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text)
            .map(Some)
            .map_err(|err| anyhow::anyhow!("reading {}: {err}", path.display())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
    }
}

fn has_skip_key(path: Option<&Path>) -> Result<bool> {
    let Some(path) = path else {
        return Ok(false);
    };
    let Some(value) = read_toml::<toml::Value>(path)? else {
        return Ok(false);
    };
    Ok(value
        .get("checks")
        .and_then(|checks| checks.get("skip"))
        .is_some())
}

fn glob_matches(pattern: &str, candidate: &str) -> bool {
    let (mut pat, mut text) = (pattern, candidate);
    let mut star: Option<(&str, &str)> = None;
    loop {
        match (pat.chars().next(), text.chars().next()) {
            (Some('*'), _) => {
                pat = &pat[1..];
                star = Some((pat, text));
            }
            (Some('?'), Some(ch)) => {
                pat = &pat[1..];
                text = &text[ch.len_utf8()..];
            }
            (Some(p), Some(ch)) if p == ch => {
                pat = &pat[p.len_utf8()..];
                text = &text[ch.len_utf8()..];
            }
            (None, None) => return true,
            _ => {
                let Some((star_pat, star_text)) = star else {
                    return false;
                };
                let Some(ch) = star_text.chars().next() else {
                    return false;
                };
                text = &star_text[ch.len_utf8()..];
                pat = star_pat;
                star = Some((star_pat, text));
            }
        }
    }
}
