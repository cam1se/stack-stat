//! One-directory encoding for branch names shared across operating systems.

use anyhow::{Context, Result};
use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};

const UNSAFE: &AsciiSet = &CONTROLS
    .add(b'%')
    .add(b'/')
    .add(b'\\')
    .add(b':')
    .add(b'*')
    .add(b'?')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'|')
    .add(b'.')
    .add(b' ');

/// Encode a branch as one safe directory segment, including slash branches.
#[must_use]
pub fn encode(branch: &str) -> String {
    let encoded = utf8_percent_encode(branch, UNSAFE).to_string();
    // Windows reserves these names, even when the filesystem is case-insensitive.
    let upper = encoded.to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"];
    if reserved.contains(&upper.as_str())
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.as_bytes()[3].is_ascii_digit()
            && upper.as_bytes()[3] != b'0')
    {
        format!("%{:02X}{}", encoded.as_bytes()[0], &encoded[1..])
    } else {
        encoded
    }
}

/// Recover a branch name from a status-store directory name.
pub fn decode(segment: &str) -> Result<String> {
    percent_decode_str(segment)
        .decode_utf8()
        .map(|value| value.into_owned())
        .with_context(|| format!("invalid encoded branch directory: {segment}"))
}
