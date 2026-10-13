//! Pure release tag, archive-name, and checksum contracts.

use semver::Version;
use serde::Deserialize;

/// Minimal entry returned by `gh release list`.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseTag {
    /// GitHub release tag name.
    pub tag_name: String,
    /// Whether the release is still a draft.
    pub is_draft: bool,
}

/// Highest non-draft, `vX.Y.Z` stable release, compared numerically.
#[must_use]
pub fn latest_stable(tags: &[ReleaseTag]) -> Option<Version> {
    tags.iter()
        .filter(|tag| !tag.is_draft)
        .filter_map(|tag| {
            let suffix = tag.tag_name.strip_prefix('v')?;
            if suffix.split('.').count() != 3
                || !suffix
                    .split('.')
                    .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
            {
                return None;
            }
            Version::parse(suffix).ok()
        })
        .max()
}

/// Whether a published version is newer than this compiled binary.
#[must_use]
pub fn is_newer(current: &Version, latest: &Version) -> bool {
    latest > current
}

/// Published archive name for a release version and target triple.
#[must_use]
pub fn asset_name(version: &Version, target: &str) -> String {
    let extension = if target.contains("-windows-") {
        "zip"
    } else {
        "tar.gz"
    };
    format!("stack-stat-{version}-{target}.{extension}")
}

/// SHA-256 hex digest for exactly one archive from `checksums.sha256`.
#[must_use]
pub fn expected_checksum(file: &str, asset: &str) -> Option<String> {
    file.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');
        if name == asset
            && parts.next().is_none()
            && hash.len() == 64
            && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            Some(hash.to_ascii_lowercase())
        } else {
            None
        }
    })
}
