//! CVDTrader Risk Library
//!
//! This crate provides risk management including position limits,
//! leverage constraints, and circuit breaker functionality.

pub mod circuit_breaker;
pub mod manager;

// Re-export commonly used types
pub use circuit_breaker::CircuitBreaker;
pub use manager::RiskManager;

/// Result type for risk operations
pub type Result<T> = anyhow::Result<T>;

/// Error type for risk operations
pub type Error = anyhow::Error;
