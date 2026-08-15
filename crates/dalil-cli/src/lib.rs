mod bundle;
mod cli;
mod html;
mod render;
pub mod report;
#[cfg(test)]
mod report_tests;
mod utils;

/// CLI entry points and process integration for the `dalil` package.
pub use cli::{command, run};
/// Typed analysis operations for native, CLI, and future transport adapters.
pub use dalil_core::*;
