//! CVDTrader Market Data Library
//!
//! This crate provides market data processing including WebSocket connection,
//! candle building, volume profile, and indicator calculations.

pub mod candle_builder;
pub mod indicators;
pub mod volume_profile;
pub mod websocket;

// Re-export commonly used types
pub use candle_builder::CandleBuilder;
pub use indicators::IndicatorCompute;
pub use volume_profile::VolumeProfileBuilder;
pub use websocket::HyperliquidWs;

/// Result type for market data operations
pub type Result<T> = anyhow::Result<T>;

/// Error type for market data operations
pub type Error = anyhow::Error;
