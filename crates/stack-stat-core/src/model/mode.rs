//! Discovery mode kept independent from the number of visible rows.

/// Whether the current branch belongs to a GitHub stack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StackMode {
    /// GitHub supplied stack metadata, even when the stack has only one entry.
    #[default]
    Stacked,
    /// One ordinary branch, with or without a pull request.
    Single,
}
