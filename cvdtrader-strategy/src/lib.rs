//! CVDTrader Strategy Library
//!
//! This crate provides trading strategy implementations including
//! the CVDPoC (Cumulative Volume Delta - Point of Control) strategy.

pub mod cvd_poc;
pub mod signals;

// Re-export commonly used types
pub use cvd_poc::CvdPocStrategy;
pub use signals::{SignalEvaluator, SignalGenerator};

/// Result type for strategy operations
pub type Result<T> = anyhow::Result<T>;

/// Error type for strategy operations
pub type Error = anyhow::Error;
