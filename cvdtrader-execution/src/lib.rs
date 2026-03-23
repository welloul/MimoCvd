//! CVDTrader Execution Library
//!
//! This crate provides order execution, TTL tracking, and fill confirmation
//! for the CVDTrader low-latency trading bot.

pub mod fills;
pub mod gateway;
pub mod ttl;

// Re-export commonly used types
pub use fills::FillTracker;
pub use gateway::ExecutionGateway;
pub use ttl::OrderTtlTracker;

/// Result type for execution operations
pub type Result<T> = anyhow::Result<T>;

/// Error type for execution operations
pub type Error = anyhow::Error;
