//! Leveled feedback with deterministic, clock-driven expiry.

use chrono::{DateTime, Duration, Utc};

/// Severity and presentation role of a dashboard notice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeLevel {
    /// Brief informational feedback.
    Info,
    /// A warning that remains visible longer than informational feedback.
    Warn,
    /// Persistent failure, cleared explicitly or by recovery of its source.
    Error,
    /// An operation is running; its result replaces this notice.
    Progress,
}

/// Footer feedback independent of any pending confirmation prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// Severity and corresponding color role.
    pub level: NoticeLevel,
    /// Human-readable feedback.
    pub text: String,
    /// Expiration deadline; errors and progress have no automatic expiry.
    pub expires_at: Option<DateTime<Utc>>,
}

impl Notice {
    /// Create feedback using an injected clock for informational/warning expiry.
    #[must_use]
    pub fn new(level: NoticeLevel, text: String, now: DateTime<Utc>) -> Self {
        let expires_at = match level {
            NoticeLevel::Info => Some(now + Duration::seconds(4)),
            NoticeLevel::Warn => Some(now + Duration::seconds(8)),
            NoticeLevel::Error | NoticeLevel::Progress => None,
        };
        Self {
            level,
            text,
            expires_at,
        }
    }
}
