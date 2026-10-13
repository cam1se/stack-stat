//! Verify a downloaded asset, then extract only its expected executable.

use std::{
    fs::File,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use semver::Version;
use sha2::{Digest, Sha256};

use super::{asset_name, expected_checksum};

/// Verify SHA-256 and extract the one binary expected for this platform.
///
/// Other archive members are ignored. No untrusted member path is written
/// directly to disk, so a malicious archive cannot traverse the output dir.
pub fn extract_verified(
    archive: &Path,
    checksums: &str,
    version: &Version,
    target: &str,
    output_dir: &Path,
) -> Result<PathBuf> {
    let name = asset_name(version, target);
    ensure!(
        archive
            .file_name()
            .is_some_and(|file| file == name.as_str()),
        "unexpected release archive name"
    );
    let expected = expected_checksum(checksums, &name)
        .context("checksum entry not found for release archive")?;
    let mut file = File::open(archive).with_context(|| format!("opening {}", archive.display()))?;
    let mut digest = Sha256::new();
    let mut buf = [0_u8; 16 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    let actual = format!("{:x}", digest.finalize());
    ensure!(actual == expected, "release checksum mismatch for {name}");

    let stem = name
        .strip_suffix(".tar.gz")
        .or_else(|| name.strip_suffix(".zip"))
        .context("unsupported release archive")?;
    let executable = if target.contains("-windows-") {
        "stack-stat.exe"
    } else {
        "stack-stat"
    };
    let member = format!("{stem}/{executable}");
    let destination = output_dir.join(executable);
    if target.contains("-windows-") {
        let file = File::open(archive)?;
        let mut zip = zip::ZipArchive::new(file).context("reading release ZIP")?;
        let mut source = zip
            .by_name(&member)
            .with_context(|| format!("missing {member} in ZIP"))?;
        let mut output = File::create(&destination)?;
        io::copy(&mut source, &mut output)?;
        output.flush()?;
    } else {
        let file = File::open(archive)?;
        let gzip = flate2::read::GzDecoder::new(file);
        let mut tar = tar::Archive::new(gzip);
        let mut found = false;
        for entry in tar.entries().context("reading release tarball")? {
            let mut entry = entry?;
            if entry.path()?.as_ref() != Path::new(&member) {
                continue;
            }
            ensure!(
                entry.header().entry_type().is_file(),
                "release binary member is not a file"
            );
            let mut output = File::create(&destination)?;
            io::copy(&mut entry, &mut output)?;
            output.flush()?;
            found = true;
            break;
        }
        if !found {
            bail!("missing {member} in tarball");
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(destination)
}
