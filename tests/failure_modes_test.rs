//! Failure mode tests for CVDTrader bot
//!
//! These tests verify the bot handles various failure scenarios correctly.

use cvdtrader_core::{Config, ExecutionMode, GlobalState, OrderStatus};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

/// Test circuit breaker behavior under consecutive failures
#[tokio::test]
async fn test_circuit_breaker_trip() {
    let state = GlobalState::new();
    let (shutdown_tx, _) = broadcast::channel(1);

    let circuit_breaker = cvdtrader_risk::CircuitBreaker::new(
        500, // latency threshold ms
        3,   // failure threshold
        shutdown_tx.clone(),
    );

    // Record failures
    circuit_breaker.record_failure().await;
    assert!(!circuit_breaker.is_tripped().await);

    circuit_breaker.record_failure().await;
    assert!(!circuit_breaker.is_tripped().await);

    circuit_breaker.record_failure().await;
    assert!(circuit_breaker.is_tripped().await);

    // Verify shutdown signal was sent
    let mut shutdown_rx = shutdown_tx.subscribe();
    let result = tokio::time::timeout(Duration::from_millis(100), shutdown_rx.recv()).await;
    assert!(result.is_ok());
}

/// Test circuit breaker reset on success
#[tokio::test]
async fn test_circuit_breaker_reset() {
    let (shutdown_tx, _) = broadcast::channel(1);

    let circuit_breaker = cvdtrader_risk::CircuitBreaker::new(
        500, // latency threshold ms
        3,   // failure threshold
        shutdown_tx.clone(),
    );

    // Record failures
    circuit_breaker.record_failure().await;
    circuit_breaker.record_failure().await;

    // Record success - should reset failure count
    circuit_breaker.record_success().await;

    // Need 3 more failures to trip
    circuit_breaker.record_failure().await;
    circuit_breaker.record_failure().await;
    assert!(!circuit_breaker.is_tripped().await);

    circuit_breaker.record_failure().await;
    assert!(circuit_breaker.is_tripped().await);
}

/// Test circuit breaker latency monitoring
#[tokio::test]
async fn test_circuit_breaker_latency() {
    let (shutdown_tx, _) = broadcast::channel(1);

    let circuit_breaker = cvdtrader_risk::CircuitBreaker::new(
        100, // latency threshold ms
        3,   // failure threshold
        shutdown_tx.clone(),
    );

    // Record high latency
    circuit_breaker.record_latency(150).await;
    assert!(circuit_breaker.is_tripped().await);

    // Verify shutdown signal was sent
    let mut shutdown_rx = shutdown_tx.subscribe();
    let result = tokio::time::timeout(Duration::from_millis(100), shutdown_rx.recv()).await;
    assert!(result.is_ok());
}

/// Test order TTL expiration
#[tokio::test]
async fn test_order_ttl_expiration() {
    let state = GlobalState::new();

    // Create an order
    let mut order = cvdtrader_core::Order::new(
        "BTC".to_string(),
        cvdtrader_core::OrderSide::Buy,
        50000.0,
        1.0,
    );

    // Manually set created_at to 2 seconds ago
    order.created_at = chrono::Utc::now() - chrono::Duration::seconds(2);
    state.add_order(order.clone()).await;

    // Check if order is expired with 1 second TTL
    assert!(order.is_expired(1));

    // Check if order is not expired with 5 second TTL
    assert!(!order.is_expired(5));
}

/// Test risk manager position size validation
#[tokio::test]
async fn test_risk_manager_position_size() {
    let state = GlobalState::new();
    let risk_manager = cvdtrader_risk::RiskManager::new(
        state.clone(),
        1000.0,  // max_position_usd
        10.0,    // max_leverage
        0.05,    // max_drawdown_pct
        10000.0, // account_balance
        ExecutionMode::DryRun,
    );

    // Valid signal
    let valid_signal = cvdtrader_core::TradeSignal::new(
        cvdtrader_core::Signal::Long,
        None,
        "BTC".to_string(),
        50000.0,
        49000.0,
        52000.0,
        0.02, // 1000 USD
    );

    assert!(risk_manager.validate_signal(&valid_signal).await.is_ok());

    // Invalid signal - exceeds position size
    let invalid_signal = cvdtrader_core::TradeSignal::new(
        cvdtrader_core::Signal::Long,
        None,
        "BTC".to_string(),
        50000.0,
        49000.0,
        52000.0,
        0.03, // 1500 USD - exceeds limit
    );

    assert!(risk_manager.validate_signal(&invalid_signal).await.is_err());
}

/// Test risk manager leverage validation
#[tokio::test]
async fn test_risk_manager_leverage() {
    let state = GlobalState::new();
    let risk_manager = cvdtrader_risk::RiskManager::new(
        state.clone(),
        1000.0, // max_position_usd
        2.0,    // max_leverage
        0.05,   // max_drawdown_pct
        1000.0, // account_balance
        ExecutionMode::DryRun,
    );

    // Add existing position
    let position = cvdtrader_core::Position::new(
        "ETH".to_string(),
        cvdtrader_core::PositionSide::Long,
        1.0,
        3000.0,
        2900.0,
        3200.0,
    );
    state.set_position("ETH".to_string(), position).await;

    // Signal that would exceed leverage
    let signal = cvdtrader_core::TradeSignal::new(
        cvdtrader_core::Signal::Long,
        None,
        "BTC".to_string(),
        50000.0,
        49000.0,
        52000.0,
        0.02, // 1000 USD - total exposure 4000 USD, leverage 4x
    );

    assert!(risk_manager.validate_signal(&signal).await.is_err());
}

/// Test risk manager existing position check
#[tokio::test]
async fn test_risk_manager_existing_position() {
    let state = GlobalState::new();
    let risk_manager = cvdtrader_risk::RiskManager::new(
        state.clone(),
        1000.0,  // max_position_usd
        10.0,    // max_leverage
        0.05,    // max_drawdown_pct
        10000.0, // account_balance
        ExecutionMode::DryRun,
    );

    // Add existing position
    let position = cvdtrader_core::Position::new(
        "BTC".to_string(),
        cvdtrader_core::PositionSide::Long,
        1.0,
        50000.0,
        49000.0,
        52000.0,
    );
    state.set_position("BTC".to_string(), position).await;

    // Signal for same symbol
    let signal = cvdtrader_core::TradeSignal::new(
        cvdtrader_core::Signal::Long,
        None,
        "BTC".to_string(),
        50000.0,
        49000.0,
        52000.0,
        0.02,
    );

    assert!(risk_manager.validate_signal(&signal).await.is_err());
}

/// Test concurrent state access under load
#[tokio::test]
async fn test_concurrent_state_access() {
    let state = GlobalState::new();
    let mut handles = vec![];

    // Spawn 100 tasks that concurrently access state
    for i in 0..100 {
        let state_clone = state.clone();
        let handle = tokio::spawn(async move {
            // Update CVD
            state_clone
                .update_global_cvd("BTC".to_string(), i as f64)
                .await;

            // Add candle
            let candle = cvdtrader_core::Candle::new("BTC".to_string(), chrono::Utc::now());
            state_clone.add_candle("BTC".to_string(), candle).await;

            // Check running state
            let _ = state_clone.is_running().await;

            // Get positions
            let _ = state_clone.get_all_positions().await;
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify state is consistent
    let cvd = state.get_global_cvd("BTC").await;
    assert!(cvd > 0.0);
}

/// Test configuration validation edge cases
#[test]
fn test_config_edge_cases() {
    let mut config = Config::default();

    // Test boundary values
    config.strategy.lookback = 1;
    assert!(config.validate().is_ok());

    config.strategy.cvd_exhaustion_ratio = 0.0;
    assert!(config.validate().is_err());

    config.strategy.cvd_exhaustion_ratio = 1.0;
    assert!(config.validate().is_ok());

    config.strategy.cvd_exhaustion_ratio = 1.01;
    assert!(config.validate().is_err());

    config.risk.max_position_usd = 0.0;
    assert!(config.validate().is_err());

    config.risk.max_leverage = 0.0;
    assert!(config.validate().is_err());

    config.execution.ttl_seconds = 0;
    assert!(config.validate().is_err());

    config.execution.ttl_check_interval_secs = 0;
    assert!(config.validate().is_err());
}

/// Test execution error types
#[test]
fn test_execution_errors() {
    use cvdtrader_execution::ExecutionError;

    // Test retryable errors
    let network_err = ExecutionError::Network {
        message: "connection refused".to_string(),
        retryable: true,
    };
    assert!(network_err.is_retryable());
    assert_eq!(network_err.retry_delay_secs(), Some(2));

    let timeout_err = ExecutionError::Timeout {
        message: "request timed out".to_string(),
    };
    assert!(timeout_err.is_retryable());
    assert_eq!(timeout_err.retry_delay_secs(), Some(1));

    let rate_limited = ExecutionError::RateLimited {
        retry_after_secs: 30,
    };
    assert!(rate_limited.is_retryable());
    assert_eq!(rate_limited.retry_delay_secs(), Some(30));

    // Test non-retryable errors
    let validation_err = ExecutionError::Validation {
        message: "invalid size".to_string(),
    };
    assert!(!validation_err.is_retryable());
    assert_eq!(validation_err.retry_delay_secs(), None);

    let insufficient_balance = ExecutionError::InsufficientBalance {
        required: 1000.0,
        available: 500.0,
    };
    assert!(!insufficient_balance.is_retryable());
}
