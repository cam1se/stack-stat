//! Released binary discovery and checksum-verified self-update.

use std::{fs, path::Path, process::Stdio, time::Duration};

use anyhow::{Context, Result, bail};
use semver::Version;
use tokio::{process::Command, time::timeout};

mod archive;
mod release;
pub use archive::extract_verified;
pub use release::{ReleaseTag, asset_name, expected_checksum, is_newer, latest_stable};

/// GitHub repository that publishes stack-stat binaries.
pub const RELEASE_REPO: &str = "tcamise-gpsw/stack-stat";

/// Target triple embedded at compile time for release asset selection.
#[must_use]
pub fn compiled_target() -> &'static str {
    env!("STACK_STAT_TARGET")
}

async fn gh(args: &[&str], deadline: Duration) -> Result<String> {
    let output = timeout(
        deadline,
        Command::new("gh")
            .args(args)
            .env("GH_PROMPT_DISABLED", "1")
            .env("NO_COLOR", "1")
            .env("GH_NO_UPDATE_NOTIFIER", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .context("gh release command timed out")??;
    if !output.status.success() {
        bail!(
            "gh {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("gh release output was not UTF-8")
}

async fn latest(repo: &str) -> Result<Option<Version>> {
    let raw = gh(
        &[
            "release",
            "list",
            "--repo",
            repo,
            "--json",
            "tagName,isDraft",
            "--limit",
            "1000",
        ],
        Duration::from_secs(20),
    )
    .await?;
    let tags: Vec<ReleaseTag> = serde_json::from_str(&raw).context("parsing release list")?;
    Ok(latest_stable(&tags))
}

/// Check for a newer stable version without downloading or modifying anything.
pub async fn check_latest(repo: &str) -> Result<Option<String>> {
    let current = Version::parse(env!("CARGO_PKG_VERSION"))?;
    Ok(latest(repo)
        .await?
        .filter(|version| is_newer(&current, version))
        .map(|version| format!("v{version}")))
}

/// Download, verify, and replace this running binary with the newest stable release.
pub async fn run_update(repo: &str) -> Result<String> {
    let current = Version::parse(env!("CARGO_PKG_VERSION"))?;
    let version = latest(repo)
        .await?
        .with_context(|| format!("no stable release found in {repo}"))?;
    if !is_newer(&current, &version) {
        return Ok(format!("Already up to date (v{current})"));
    }
    let tag = format!("v{version}");
    let asset = asset_name(&version, compiled_target());
    let temp = tempfile::tempdir().context("creating update staging directory")?;
    let directory = temp.path().to_string_lossy().into_owned();
    gh(
        &[
            "release",
            "download",
            &tag,
            "--repo",
            repo,
            "--pattern",
            &asset,
            "--pattern",
            "checksums.sha256",
            "--dir",
            &directory,
            "--clobber",
        ],
        Duration::from_secs(120),
    )
    .await?;
    let checksum_path = temp.path().join("checksums.sha256");
    let checksums = fs::read_to_string(&checksum_path)
        .with_context(|| format!("reading {}", checksum_path.display()))?;
    let binary = extract_verified(
        &temp.path().join(&asset),
        &checksums,
        &version,
        compiled_target(),
        temp.path(),
    )?;
    self_replace::self_replace(Path::new(&binary))
        .context("replacing installed stack-stat binary")?;
    Ok(format!("Updated stack-stat v{current} → v{version}"))
}
