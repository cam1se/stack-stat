//! Terminal dashboard application and CLI actions for stacked PR status.
#![deny(missing_docs)]

/// Shared normal-mode keybindings.
pub mod action;
/// Interactive App state and input handling.
pub mod app;
/// Messages and effect requests.
pub mod message;
/// Background Git, GitHub, and local status-store refresh.
pub mod poll;
/// Dashboard prerequisites checked before entering terminal raw mode.
pub mod preflight;
/// Local check reporting, listing, and clearing.
pub mod report;
/// Terminal lifecycle and event loop.
pub mod runtime;
/// Semantic terminal colors and capability detection.
pub mod theme;
/// Pure ratatui widgets and dashboard layout.
pub mod ui;
/// Release lookup, archive verification, and self replacement.
pub mod update;
/// Character-counted word wrapping for terminal content.
pub mod wrap;
