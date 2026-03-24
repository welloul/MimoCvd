//! CVDTrader Core Library
//!
//! This crate provides core types, state management, and configuration
//! for the CVDTrader low-latency trading bot.

pub mod config;
pub mod history;
pub mod state;
pub mod types;

// Re-export commonly used types
pub use config::Config;
pub use history::{TradeHistory, TradeStatistics};
pub use state::GlobalState;
pub use types::{
    Candle, ExecutionMode, ExitReason, Order, OrderSide, OrderStatus, Position, PositionSide,
    SetupType, Side, Signal, Trade, TradeRecord, TradeSignal,
};

/// Result type for core operations
pub type Result<T> = anyhow::Result<T>;

/// Error type for core operations
pub type Error = anyhow::Error;
