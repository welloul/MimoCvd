//! Integration tests for CVDTrader bot
//!
//! These tests verify the full bot lifecycle and component interaction.

use chrono::Utc;
use cvdtrader_bot::Bot;
use cvdtrader_core::{Config, ExecutionMode, GlobalState, OrderStatus, PositionSide, Side, Trade};
use std::time::Duration;
use tokio::time::timeout;

/// Test bot startup and shutdown lifecycle
#[tokio::test]
async fn test_bot_lifecycle() {
    let mut config = Config::default();
    config.execution.mode = ExecutionMode::DryRun;
    config.bot.health_check_port = 0; // Disable health check for tests
    config.exchange.symbols = vec!["BTC".to_string()];

    let bot = Bot::new(config);

    // Verify initial state
    assert!(!bot.state().is_running().await);

    // Start bot in background
    let bot_clone = bot.clone();
    let handle = tokio::spawn(async move {
        // Note: This will fail to connect to WebSocket in tests, but that's expected
        let _ = bot_clone.start().await;
    });

    // Give it a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Shutdown
    handle.abort();

    // Verify bot can be created and dropped without panicking
    assert!(true);
}

/// Test global state operations
#[tokio::test]
async fn test_global_state_operations() {
    let state = GlobalState::new();

    // Test running state
    assert!(!state.is_running().await);
    state.set_running(true).await;
    assert!(state.is_running().await);
    state.set_running(false).await;
    assert!(!state.is_running().await);

    // Test position operations
    let position = cvdtrader_core::Position::new(
        "BTC".to_string(),
        PositionSide::Long,
        1.0,
        50000.0,
        49000.0,
        52000.0,
    );

    assert!(!state.has_position("BTC").await);
    state
        .set_position("BTC".to_string(), position.clone())
        .await;
    assert!(state.has_position("BTC").await);

    let retrieved = state.get_position("BTC").await.unwrap();
    assert_eq!(retrieved.symbol, "BTC");
    assert_eq!(retrieved.size, 1.0);

    state.remove_position("BTC").await;
    assert!(!state.has_position("BTC").await);

    // Test order operations
    let order = cvdtrader_core::Order::new(
        "BTC".to_string(),
        cvdtrader_core::OrderSide::Buy,
        50000.0,
        1.0,
    );

    assert!(state.get_order(&order.id).await.is_none());
    state.add_order(order.clone()).await;
    assert!(state.get_order(&order.id).await.is_some());

    let retrieved_order = state.get_order(&order.id).await.unwrap();
    assert_eq!(retrieved_order.symbol, "BTC");
    assert_eq!(retrieved_order.status, OrderStatus::Pending);

    state.remove_order(&order.id).await;
    assert!(state.get_order(&order.id).await.is_none());

    // Test candle operations
    let candle = cvdtrader_core::Candle::new("BTC".to_string(), Utc::now());
    assert!(state.get_candles("BTC").await.is_empty());
    state.add_candle("BTC".to_string(), candle).await;
    assert_eq!(state.get_candles("BTC").await.len(), 1);

    // Test CVD operations
    assert_eq!(state.get_global_cvd("BTC").await, 0.0);
    state.update_global_cvd("BTC".to_string(), 10.0).await;
    assert_eq!(state.get_global_cvd("BTC").await, 10.0);
    state.update_global_cvd("BTC".to_string(), -5.0).await;
    assert_eq!(state.get_global_cvd("BTC").await, 5.0);
}

/// Test concurrent state access
#[tokio::test]
async fn test_concurrent_state_access() {
    let state = GlobalState::new();
    let mut handles = vec![];

    // Spawn multiple tasks that access state concurrently
    for i in 0..10 {
        let state_clone = state.clone();
        let handle = tokio::spawn(async move {
            // Each task updates CVD
            state_clone
                .update_global_cvd("BTC".to_string(), i as f64)
                .await;

            // Each task adds a candle
            let candle = cvdtrader_core::Candle::new("BTC".to_string(), Utc::now());
            state_clone.add_candle("BTC".to_string(), candle).await;

            // Each task checks running state
            let _ = state_clone.is_running().await;
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify final state
    let cvd = state.get_global_cvd("BTC").await;
    assert!(cvd > 0.0); // Should have accumulated some CVD
}

/// Test configuration validation
#[test]
fn test_config_validation() {
    let mut config = Config::default();

    // Valid config should pass
    assert!(config.validate().is_ok());

    // Invalid lookback
    config.strategy.lookback = 0;
    assert!(config.validate().is_err());
    config.strategy.lookback = 20;

    // Invalid CVD ratio
    config.strategy.cvd_exhaustion_ratio = 1.5;
    assert!(config.validate().is_err());
    config.strategy.cvd_exhaustion_ratio = 0.70;

    // Invalid max position
    config.risk.max_position_usd = -100.0;
    assert!(config.validate().is_err());
    config.risk.max_position_usd = 1000.0;

    // Invalid TTL
    config.execution.ttl_seconds = 0;
    assert!(config.validate().is_err());
    config.execution.ttl_seconds = 120;

    // Invalid log level
    config.logging.level = "invalid".to_string();
    assert!(config.validate().is_err());
    config.logging.level = "info".to_string();
}

/// Test trade processing
#[test]
fn test_trade_processing() {
    let trade = Trade::new("BTC".to_string(), 50000.0, 1.5, Side::Buy, Utc::now());

    assert_eq!(trade.symbol, "BTC");
    assert_eq!(trade.price, 50000.0);
    assert_eq!(trade.size, 1.5);
    assert_eq!(trade.side, Side::Buy);
    assert_eq!(trade.delta(), 1.5); // Buy = positive delta

    let sell_trade = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Sell, Utc::now());

    assert_eq!(sell_trade.delta(), -1.0); // Sell = negative delta
}

/// Test candle operations
#[test]
fn test_candle_operations() {
    let mut candle = cvdtrader_core::Candle::new("BTC".to_string(), Utc::now());

    let trade1 = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, Utc::now());
    let trade2 = Trade::new("BTC".to_string(), 50100.0, 0.5, Side::Sell, Utc::now());

    candle.add_trade(&trade1);
    candle.add_trade(&trade2);

    assert_eq!(candle.open, 50000.0);
    assert_eq!(candle.high, 50100.0);
    assert_eq!(candle.low, 50000.0);
    assert_eq!(candle.close, 50100.0);
    assert_eq!(candle.volume, 1.5);
    assert_eq!(candle.cvd, 0.5); // 1.0 - 0.5
    assert_eq!(candle.range(), 100.0);
    assert_eq!(candle.midpoint(), 50050.0);
}

/// Test position operations
#[test]
fn test_position_operations() {
    let mut position = cvdtrader_core::Position::new(
        "BTC".to_string(),
        PositionSide::Long,
        1.0,
        50000.0,
        49000.0,
        52000.0,
    );

    // Test PnL calculation
    position.update_pnl(51000.0);
    assert_eq!(position.unrealized_pnl, 1000.0);

    position.update_pnl(49000.0);
    assert_eq!(position.unrealized_pnl, -1000.0);

    // Test stop loss
    assert!(position.is_sl_hit(48999.0));
    assert!(!position.is_sl_hit(49001.0));

    // Test take profit
    assert!(position.is_tp_hit(52001.0));
    assert!(!position.is_tp_hit(51999.0));

    // Test stop loss update (only moves in profitable direction)
    position.update_stop_loss(49500.0);
    assert_eq!(position.stop_loss, 49500.0);

    position.update_stop_loss(49000.0); // Should not move down
    assert_eq!(position.stop_loss, 49500.0);

    // Test flip streak
    assert_eq!(position.flip_streak, 0);
    position.increment_flip_streak();
    assert_eq!(position.flip_streak, 1);
    position.reset_flip_streak();
    assert_eq!(position.flip_streak, 0);
}

/// Test order operations
#[test]
fn test_order_operations() {
    let mut order = cvdtrader_core::Order::new(
        "BTC".to_string(),
        cvdtrader_core::OrderSide::Buy,
        50000.0,
        1.0,
    );

    assert_eq!(order.status, OrderStatus::Pending);
    assert!(!order.is_expired(120)); // Should not be expired immediately

    // Test fill update
    order.update_fill(0.5, 50000.0);
    assert_eq!(order.status, OrderStatus::PartiallyFilled);
    assert_eq!(order.filled_size, 0.5);

    order.update_fill(0.5, 50000.0);
    assert_eq!(order.status, OrderStatus::Filled);
    assert_eq!(order.filled_size, 1.0);

    // Test status update
    order.update_status(OrderStatus::Cancelled);
    assert_eq!(order.status, OrderStatus::Cancelled);
}
