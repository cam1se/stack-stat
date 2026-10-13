//! Bounded retention of local check results after branch movement.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::StatusEntry;

/// Partition owned entries into kept and deleted, avoiding copying check data.
///
/// Keep the current-HEAD entry plus the most recently reported older entry for
/// each live local branch. A deleted branch retains nothing.
#[must_use]
pub fn retained(
    entries: Vec<StatusEntry>,
    live_branches: &HashMap<String, String>,
) -> (Vec<StatusEntry>, Vec<StatusEntry>) {
    let mut keep = vec![false; entries.len()];
    {
        let mut newest_stale: HashMap<&str, (usize, DateTime<Utc>)> = HashMap::new();
        for (index, entry) in entries.iter().enumerate() {
            let Some(head) = live_branches.get(&entry.branch) else {
                continue;
            };
            if entry.sha == *head {
                keep[index] = true;
                continue;
            }
            let Some(reported_at) = entry.checks.values().map(|check| check.recorded_at).max()
            else {
                continue;
            };
            newest_stale
                .entry(&entry.branch)
                .and_modify(|latest| {
                    if reported_at > latest.1 {
                        *latest = (index, reported_at);
                    }
                })
                .or_insert((index, reported_at));
        }
        for (index, _) in newest_stale.values() {
            keep[*index] = true;
        }
    }
    let mut retained = Vec::new();
    let mut deleted = Vec::new();
    for (entry, is_kept) in entries.into_iter().zip(keep) {
        if is_kept {
            retained.push(entry);
        } else {
            deleted.push(entry);
        }
    }
    (retained, deleted)
}
