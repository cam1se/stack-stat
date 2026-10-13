//! Terminal dashboard application and CLI actions for stacked PR status.
#![deny(missing_docs)]

/// Dashboard prerequisites checked before entering terminal raw mode.
pub mod preflight;
/// Local check reporting, listing, and clearing.
pub mod report;
