//! Domain and provider contracts for the stack-stat terminal dashboard.
#![deny(missing_docs)]

/// Pure composition of successful provider snapshots.
pub mod compose;
/// Repository and user configuration.
pub mod config;
/// Domain view presented by the dashboard.
pub mod model;
/// Git, GitHub, and local check storage interfaces.
pub mod providers;
