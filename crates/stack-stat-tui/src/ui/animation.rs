//! Frame-indexed animation state and consumer-visible cell change detection.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use ratatui::style::Color;
use stack_stat_core::model::{BranchState, StackState};

/// Branch table column that may flash when its displayed value changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Column {
    /// Branch name or checked-out marker.
    Branch,
    /// Rebase status.
    Rebase,
    /// PR identity.
    Pr,
    /// Draft, ready, merged, or queued state.
    State,
    /// CI check outcome.
    Checks,
    /// Review decision or reviewer list.
    Reviews,
    /// Unresolved review-thread count.
    Threads,
    /// Local report results.
    Local,
}

/// Identity of one changing table cell.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CellId {
    /// Stack branch name.
    pub branch: String,
    /// Visible table column.
    pub column: Column,
}
impl CellId {
    /// Identify one branch/column combination.
    #[must_use]
    pub fn new(branch: &str, column: Column) -> Self {
        Self {
            branch: branch.into(),
            column,
        }
    }
}

/// Presentation-only tick and flash expiry state.
#[derive(Default, Debug)]
pub struct AnimationState {
    tick: u64,
    flashes: HashMap<CellId, u64>,
}

impl AnimationState {
    /// Current animation tick; production advances every 100ms.
    #[must_use]
    pub fn current_tick(&self) -> u64 {
        self.tick
    }

    /// Advance one tick and discard completed flashes.
    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        self.flashes.retain(|_, expiry| *expiry > self.tick);
    }

    /// Phase between 0 and 1 for a repeating period of ticks.
    #[must_use]
    pub fn phase(&self, period: u64) -> f32 {
        if period == 0 {
            return 0.0;
        }
        (self.tick % period) as f32 / period as f32
    }

    /// One frame of a braille activity spinner.
    #[must_use]
    pub fn spinner_char(&self) -> char {
        const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        FRAMES[(self.tick as usize) % FRAMES.len()]
    }

    /// One loading-skeleton frame, offset so adjacent cells form a wave.
    #[must_use]
    pub fn skeleton_char(&self, offset: usize) -> char {
        const FRAMES: [char; 7] = ['░', '▒', '▓', '█', '▓', '▒', '░'];
        FRAMES[((self.tick as usize).wrapping_add(offset)) % FRAMES.len()]
    }

    /// Pulse between RGB colors on a 20-tick sine wave; indexed colors alternate.
    #[must_use]
    pub fn pulse_color(&self, base: Color, peak: Color, period: u64) -> Color {
        let phase = self.phase(period);
        match (base, peak) {
            (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
                let intensity = ((phase * std::f32::consts::TAU).sin() + 1.0) * 0.5;
                let blend = |a: u8, b: u8| {
                    (f32::from(a) + (f32::from(b) - f32::from(a)) * intensity).round() as u8
                };
                Color::Rgb(blend(r1, r2), blend(g1, g2), blend(b1, b2))
            }
            (Color::Indexed(_), Color::Indexed(_)) => {
                if phase < 0.5 {
                    base
                } else {
                    peak
                }
            }
            _ => base,
        }
    }

    /// Flash a changed cell for the requested number of ticks.
    pub fn flash(&mut self, cell: CellId, ticks: u64) {
        if ticks > 0 {
            self.flashes.insert(cell, self.tick.saturating_add(ticks));
        }
    }

    /// Whether the identified cell is within its flash lifetime.
    #[must_use]
    pub fn is_flashing(&self, cell: &CellId) -> bool {
        self.flashes
            .get(cell)
            .is_some_and(|expiry| *expiry > self.tick)
    }

    /// Test a visible cell without allocating a branch-name copy each frame.
    #[must_use]
    pub fn is_flashing_for(&self, branch: &str, column: Column) -> bool {
        self.flashes.iter().any(|(cell, expiry)| {
            cell.branch == branch && cell.column == column && *expiry > self.tick
        })
    }
}

/// Return the cells whose consumer-visible values differ between two snapshots.
#[must_use]
pub fn diff_cells(old: &StackState, new: &StackState) -> Vec<CellId> {
    let mut changed = Vec::new();
    for branch in &new.branches {
        let old_branch = old.branches.iter().find(|prior| prior.name == branch.name);
        let Some(prior) = old_branch else {
            for column in [
                Column::Branch,
                Column::Pr,
                Column::State,
                Column::Checks,
                Column::Reviews,
                Column::Threads,
                Column::Rebase,
                Column::Local,
            ] {
                changed.push(CellId::new(&branch.name, column));
            }
            continue;
        };
        compare_branch(prior, branch, &mut changed);
    }
    changed
}

fn compare_branch(old: &BranchState, new: &BranchState, changed: &mut Vec<CellId>) {
    let name = &new.name;
    if old.is_current != new.is_current || old.local_sync != new.local_sync {
        changed.push(CellId::new(name, Column::Branch));
    }
    if old.rebase != new.rebase || old.trunk_rebase_gates != new.trunk_rebase_gates {
        changed.push(CellId::new(name, Column::Rebase));
    }
    if old.pr_number != new.pr_number {
        changed.push(CellId::new(name, Column::Pr));
    }
    if old.lifecycle != new.lifecycle
        || old.pr.as_ref().map(|pr| pr.is_draft) != new.pr.as_ref().map(|pr| pr.is_draft)
    {
        changed.push(CellId::new(name, Column::State));
    }
    if old.pr.as_ref().map(|pr| &pr.checks) != new.pr.as_ref().map(|pr| &pr.checks) {
        changed.push(CellId::new(name, Column::Checks));
    }
    if old.pr.as_ref().map(|pr| (&pr.reviews, pr.review_decision))
        != new.pr.as_ref().map(|pr| (&pr.reviews, pr.review_decision))
    {
        changed.push(CellId::new(name, Column::Reviews));
    }
    if old
        .pr
        .as_ref()
        .map(|pr| (pr.unresolved_threads, pr.threads_gate))
        != new
            .pr
            .as_ref()
            .map(|pr| (pr.unresolved_threads, pr.threads_gate))
    {
        changed.push(CellId::new(name, Column::Threads));
    }
    if old.local_checks != new.local_checks {
        changed.push(CellId::new(name, Column::Local));
    }
}

/// Whether a source age exceeds twice its expected refresh interval.
#[must_use]
pub fn stale_after(now: DateTime<Utc>, fetched_at: DateTime<Utc>, interval_secs: u64) -> bool {
    (now - fetched_at).num_seconds() > (interval_secs.saturating_mul(2).min(i64::MAX as u64)) as i64
}
