

## Project Overview

__CVDTrader__ is a production-ready, low-latency cryptocurrency trading bot built in Rust 1.75+. It implements the CVDPoC (Cumulative Volume Delta - Point of Control) strategy for automated trading on the Hyperliquid exchange.

## Architecture Summary

### Module Structure (6 crates)

```javascript
cvdtrader-rust/
├── cvdtrader-core/          # Core types, state management, configuration
├── cvdtrader-market-data/   # WebSocket, candle building, indicators
├── cvdtrader-strategy/      # CVDPoC strategy implementation
├── cvdtrader-execution/     # Order execution and fill tracking
├── cvdtrader-risk/          # Risk management and circuit breaker
└── cvdtrader-bot/           # Main bot orchestrator
```

### Data Flow

1. __Market Data Ingestion__: WebSocket → mpsc channel → CandleBuilder → mpsc channel → Strategy
2. __Signal Processing__: Strategy → POC calculation → Indicator updates → Signal generation → Risk validation → Order placement
3. __Position Management__: Strategy monitors positions → Exit conditions → Position close via gateway
4. __Risk Management__: RiskManager validates signals → CircuitBreaker monitors health → OrderTtlTracker cancels stale orders

## Key Components

### cvdtrader-core

- __Types__: Trade, Candle, Position, Order, TradeSignal, TradeRecord
- __GlobalState__: Thread-safe shared state using async RwLocks
- __Config__: TOML-based configuration with validation
- __TradeHistory__: SQLite-based trade persistence

### cvdtrader-market-data

- __HyperliquidWs__: WebSocket client with automatic reconnection
- __CandleBuilder__: Aggregates trades into 1-minute candles
- __VolumeProfileBuilder__: Calculates Point of Control (POC)
- __IndicatorCompute__: CVD, RVOL, and historical indicators

### cvdtrader-strategy

- __CvdPocStrategy__: Main strategy orchestrator
- __SignalEvaluator__: Core signal generation logic
- __Strategy Logic__: Swing detection, exhaustion/absorption setups, POC confirmation, CVD-based trailing stops

### cvdtrader-execution

- __ExecutionGateway__: Order placement/cancellation (DryRun/TestNet/Live modes)
- __FillTracker__: Processes order fill events
- __OrderTtlTracker__: Automatic order cancellation after TTL
- __ExecutionError__: Specific error types (Network, Validation, Exchange, Timeout, RateLimited, etc.)

### cvdtrader-risk

- __RiskManager__: Validates signals (position size, leverage, drawdown limits)
- __CircuitBreaker__: Monitors latency and failure rates, triggers shutdown

### cvdtrader-bot

- __Bot__: Main coordinator struct
- __Event loop__: Processes trades and candles
- __Health check endpoint__: HTTP monitoring
- __Graceful shutdown handling__

## Performance Characteristics

- __Latency Target__: <1ms for indicator calculations
- __Throughput__: Designed for 10,000+ trades/second processing
- __Memory Usage__: <100MB typical usage
- __CPU__: Single-core sufficient for multi-pair monitoring
- __Order TTL__: Default 120 seconds (configurable)
- __Circuit Breaker__: Latency threshold 500ms, failure threshold 3 consecutive failures

## Configuration System

TOML-based configuration with sections for:

- __Exchange__: WebSocket/API URLs, symbols
- __Strategy__: Lookback periods, CVD thresholds, risk parameters
- __Risk__: Position limits, leverage, drawdown, circuit breaker settings
- __Execution__: Mode (DryRun/TestNet/Live), TTL, post-only flag
- __Logging__: Level and format

## Testing Infrastructure

- Unit tests for all modules
- Integration tests for bot lifecycle
- Failure mode tests for production scenarios
- Property-based testing capabilities
- Benchmark testing with criterion.rs

## Known Issues & Technical Debt

1. __WebSocket Reconnection__: Exponential backoff may not be optimal for all network partition scenarios
2. __Configuration Hot Reloading__: Requires bot restart to pick up configuration changes
3. __Volume Profile Optimization__: Rebuilds volume profile per candle instead of maintaining rolling window
4. __Unused Imports__: Some modules have unused imports (cosmetic issue)

## Critical Architecture Notes

1. __Layered Architecture__: Market Data → Strategy → Risk → Execution → State
2. __Async Communication__: All inter-layer communication uses async channels for backpressure handling
3. __Fault Isolation__: Failure in one layer doesn't crash the entire bot
4. __Thread Safety__: GlobalState uses async RwLocks for concurrent access
5. __Graceful Shutdown__: Broadcast channels coordinate shutdown across all components

## Context for Next Developer

- __Do not modify__ Tokio runtime settings without performance testing
- __Maintain__ separation of concerns between modules
- __Preserve__ low-latency design principles in hot paths
- __Keep__ dry-run mode as default execution mode for safety
- __Do not remove__ circuit breaker functionality - critical for production stability
