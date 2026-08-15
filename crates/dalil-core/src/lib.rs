//! Typed, read-only repository analysis operations used by Dalil adapters.
//!
//! `dalil-core` performs no command-line parsing, output rendering, or protocol
//! handling. Callers provide a typed [`AnalysisRequest`] and receive typed report
//! models, warnings, and errors.

mod api;
mod history;
mod landmarks;
mod lifecycle;
mod manifests;
mod map;
mod report;
mod security;
mod utils;

pub use api::*;
pub use lifecycle::*;
pub use map::{CacheCommand, CacheControlReport, MapError, MapSettings, cache_control};
pub use report::*;
