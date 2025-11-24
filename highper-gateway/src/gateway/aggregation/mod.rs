//! API Aggregation module
//!
//! Provides request composition and response merging for aggregating multiple backend calls.

pub mod config;
pub mod executor;
pub mod merger;

pub use config::{AggregationConfig, BackendCall, MergeStrategy, ErrorStrategy};
pub use executor::AggregationExecutor;
pub use merger::ResponseMerger;
