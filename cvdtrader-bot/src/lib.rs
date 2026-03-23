//! CVDTrader Bot Library
//!
//! This crate provides the main bot orchestrator for the CVDTrader
//! low-latency trading bot.

pub mod orchestrator;

// Re-export commonly used types
pub use orchestrator::Bot;

/// Result type for bot operations
pub type Result<T> = anyhow::Result<T>;

/// Error type for bot operations
pub type Error = anyhow::Error;
