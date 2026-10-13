use std::{fs, io::Write, path::Path};

use semver::Version;
use sha2::{Digest, Sha256};
use stack_stat_tui::update::{
    ReleaseTag, asset_name, expected_checksum, extract_verified, is_newer, latest_stable,
};

#[test]
fn release_selection_ignores_drafts_and_non_stable_tags() {
    let tags = [
        "v0.9.0",
        "v0.10.0",
        "v0.11.0-rc.1",
        "not-a-release",
        "v01.0.0",
    ];
    let tags: Vec<_> = tags
        .iter()
        .map(|name| ReleaseTag {
            tag_name: (*name).into(),
            is_draft: false,
        })
        .chain([ReleaseTag {
            tag_name: "v9.0.0".into(),
            is_draft: true,
        }])
        .collect();
    assert_eq!(
        latest_stable(&tags).unwrap(),
        Version::parse("0.10.0").unwrap()
    );
    assert!(is_newer(
        &Version::parse("0.9.0").unwrap(),
        &Version::parse("0.10.0").unwrap()
    ));
    assert!(!is_newer(
        &Version::parse("0.10.0").unwrap(),
        &Version::parse("0.10.0").unwrap()
    ));
    assert!(!is_newer(
        &Version::parse("0.11.0").unwrap(),
        &Version::parse("0.10.0").unwrap()
    ));
}

#[test]
fn archive_names_match_every_release_target_and_checksum_lookup_is_exact() {
    let version = Version::parse("0.2.0").unwrap();
    for target in [
        "aarch64-apple-darwin",
        "x86_64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
    ] {
        assert_eq!(
            asset_name(&version, target),
            format!("stack-stat-0.2.0-{target}.tar.gz")
        );
    }
    assert_eq!(
        asset_name(&version, "x86_64-pc-windows-msvc"),
        "stack-stat-0.2.0-x86_64-pc-windows-msvc.zip"
    );
    let hash = "a".repeat(64);
    let list = format!("{hash}  stack-stat-0.2.0-x86_64-unknown-linux-gnu.tar.gz\n");
    assert_eq!(
        expected_checksum(&list, "stack-stat-0.2.0-x86_64-unknown-linux-gnu.tar.gz"),
        Some(hash)
    );
    assert_eq!(
        expected_checksum(&list, "stack-stat-0.2.0-x86_64-unknown-linux-gnu.tar"),
        None
    );
}

fn make_tar(archive: &Path, stem: &str) {
    let file = fs::File::create(archive).unwrap();
    let gzip = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(gzip);
    let bytes = b"real-new-binary";
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    tar.append_data(&mut header, format!("{stem}/stack-stat"), &bytes[..])
        .unwrap();
    tar.finish().unwrap();
}

fn make_zip(archive: &Path, stem: &str) {
    let file = fs::File::create(archive).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file(
        format!("{stem}/stack-stat.exe"),
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(b"real-new-binary").unwrap();
    zip.finish().unwrap();
}

#[test]
fn verified_extraction_supports_unix_and_windows_archives() {
    let temp = tempfile::tempdir().unwrap();
    let version = Version::parse("0.2.0").unwrap();
    for target in ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"] {
        let name = asset_name(&version, target);
        let archive = temp.path().join(&name);
        let stem = name.trim_end_matches(".tar.gz").trim_end_matches(".zip");
        if target.contains("windows") {
            make_zip(&archive, stem);
        } else {
            make_tar(&archive, stem);
        }
        let hash = format!("{:x}", Sha256::digest(fs::read(&archive).unwrap()));
        let checksums = format!("{hash}  {name}\n");
        let binary = extract_verified(&archive, &checksums, &version, target, temp.path()).unwrap();
        assert_eq!(fs::read(binary).unwrap(), b"real-new-binary");
    }
}

#[test]
fn checksum_mismatch_never_places_replacement_binary() {
    let temp = tempfile::tempdir().unwrap();
    let version = Version::parse("0.2.0").unwrap();
    let target = "x86_64-unknown-linux-gnu";
    let name = asset_name(&version, target);
    let archive = temp.path().join(&name);
    make_tar(&archive, name.trim_end_matches(".tar.gz"));
    let error = extract_verified(
        &archive,
        &format!("{}  {name}\n", "0".repeat(64)),
        &version,
        target,
        temp.path(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("checksum"));
    assert!(!temp.path().join("stack-stat").exists());
}
