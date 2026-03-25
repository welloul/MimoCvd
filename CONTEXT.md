# CVDTrader Project Context

## Project Overview

**CVDTrader** is a production-ready, low-latency cryptocurrency trading bot built in Rust 1.75+. It implements the CVDPoC (Cumulative Volume Delta - Point of Control) strategy for automated trading on the Hyperliquid exchange. The system is designed for high-frequency trading with sub-millisecond latency targets and comprehensive risk management.

## Architecture Summary

### Module Structure
The project follows a modular, layered architecture with clear separation of concerns:

```
┌─────────────────────────────────────────────────────────────┐
│                    RUST TRADING BOT                         │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │ Market Data  │───▶│  Indicators  │───▶│   Strategy   │  │
│  │   Engine     │    │    Engine    │    │    Engine    │  │
│  └──────────────┘    └──────────────┘    └──────────────┘  │
│         │                   │                   │           │
│         ▼                   ▼                   ▼           │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │  WebSocket   │    │  CVD/POC     │    │  Signal      │  │
│  │  Connection  │    │  Calc        │    │  Evaluation  │  │
│  └──────────────┘    └──────────────┘    └──────────────┘  │
│                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │  Execution   │◀───│    Risk      │◀───│    State     │  │
│  │   Engine     │    │   Engine     │    │   Manager    │  │
│  └──────────────┘    └──────────────┘    └──────────────┘  │
│         │                   │                   │           │
│         ▼                   ▼                   ▼           │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │  Order       │    │  Position    │    │  Global      │  │
│  │  Gateway     │    │  Limits      │    │  State       │  │
│  └──────────────┘    └──────────────┘    └──────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Data Flow Patterns

1. **Market Data Ingestion**: WebSocket → mpsc channel → CandleBuilder → mpsc channel → Strategy
2. **Signal Processing**: Strategy → POC calculation → Indicator updates → Signal generation → Risk validation → Order placement
3. **Position Management**: Strategy monitors positions → Exit conditions → Position close via gateway
4. **Risk Management**: RiskManager validates signals → CircuitBreaker monitors health → OrderTtlTracker cancels stale orders

## Module Responsibilities

### cvdtrader-core
- **Purpose**: Foundational building blocks - shared types, state management, configuration
- **Key Components**:
  - Types: Trade, Candle, Position, Order, TradeSignal, TradeRecord
  - GlobalState: Thread-safe shared state using async RwLocks
  - Config: TOML-based configuration with validation
  - TradeHistory: SQLite-based trade persistence
  - Result/Error types for consistent error handling

### cvdtrader-market-data
- **Purpose**: Market data processing pipeline
- **Key Components**:
  - HyperliquidWs: WebSocket client with automatic reconnection
  - CandleBuilder: Aggregates trades into 1-minute candles
  - VolumeProfileBuilder: Calculates Point of Control (POC)
  - IndicatorCompute: CVD, RVOL, and historical indicators

### cvdtrader-strategy
- **Purpose**: Trading strategy implementation
- **Key Components**:
  - CvdPocStrategy: Main strategy orchestrator
  - SignalEvaluator: Core signal generation logic
  - SignalGenerator: Trait for signal evaluation
- **Strategy Logic**:
  - Swing detection with configurable lookback period
  - Exhaustion setup: CVD conviction weakening
  - Absorption setup: Price breaks extreme with range contraction
  - POC confirmation for direction
  - CVD-based trailing stop logic for position exits

### cvdtrader-execution
- **Purpose**: Order execution and tracking
- **Key Components**:
  - ExecutionGateway: Order placement/cancellation (DryRun/TestNet/Live modes)
  - FillTracker: Processes order fill events
  - OrderTtlTracker: Automatic order cancellation after TTL
  - ExecutionError: Specific error types (Network, Validation, Exchange, Timeout, RateLimited, etc.)

### cvdtrader-risk
- **Purpose**: Risk management and safety systems
- **Key Components**:
  - RiskManager: Validates signals (position size, leverage, drawdown limits)
  - CircuitBreaker: Monitors latency and failure rates, triggers shutdown

### cvdtrader-bot
- **Purpose**: Main orchestrator and event loop
- **Key Components**:
  - Bot: Main coordinator struct
  - Event loop: Processes trades and candles
  - Health check endpoint: HTTP monitoring
  - Graceful shutdown handling

## Performance Characteristics

- **Latency Target**: <1ms for indicator calculations
- **Throughput**: Designed for 10,000+ trades/second processing
- **Memory Usage**: <100MB typical usage
- **CPU**: Single-core sufficient for multi-pair monitoring
- **Order TTL**: Default 120 seconds (configurable)
- **Circuit Breaker**: Latency threshold 500ms, failure threshold 3 consecutive failures

## Known Issues & Technical Debt

### High Priority
1. **WebSocket Reconnection**: Exponential backoff may not be optimal for all network partition scenarios

### Medium Priority
4. **Configuration Hot Reloading**: Requires bot restart to pick up configuration changes
5. **Volume Profile Optimization**: Rebuilds volume profile per candle instead of maintaining rolling window

### Low Priority
6. **Unused Imports**: Some modules have unused imports (cosmetic issue)

### Technical Debt
- Hardcoded values throughout (channel sizes, intervals, account balance)
- Some API calls use unwrap() rather than proper error handling
- Tight coupling between Bot and all component types
- Limited integration testing of full bot lifecycle
- Configuration validation could be strengthened

## Critical Architecture Notes

1. **Layered Architecture**: Market Data → Strategy → Risk → Execution → State
2. **Async Communication**: All inter-layer communication uses async channels for backpressure handling
3. **Fault Isolation**: Failure in one layer doesn't crash the entire bot
4. **Thread Safety**: GlobalState uses async RwLocks for concurrent access
5. **Graceful Shutdown**: Broadcast channels coordinate shutdown across all components

## Configuration System

TOML-based configuration with sections for:
- **Exchange**: WebSocket/API URLs, symbols
- **Strategy**: Lookback periods, CVD thresholds, risk parameters
- **Risk**: Position limits, leverage, drawdown, circuit breaker settings
- **Execution**: Mode (DryRun/TestNet/Live), TTL, post-only flag
- **Logging**: Level and format

Supports environment variable overrides for all configuration values.

## Trade History & Persistence

SQLite database (`trades.db`) automatically records all completed trades:
- Trade ID, symbol, side, size, entry/exit prices
- Entry/exit timestamps, realized PnL, PnL percentage
- Exit reason (StopLoss, TakeProfit, CvdFlip, Manual)
- Setup type (Exhaustion, Absorption)

Provides comprehensive querying capabilities for post-trade analysis.

## Testing Infrastructure

- Unit tests for all modules
- Integration tests for bot lifecycle
- Failure mode tests for production scenarios
- Property-based testing capabilities
- Benchmark testing with criterion.rs

## Future Roadmap

### v1.1.0
- Configuration hot-reloading
- Enhanced error handling with specific error types
- Improved circuit breaker with automatic reset
- Optimized volume profile with rolling window

### v1.2.0
- Plugin architecture for exchange/strategy components
- Metrics collection and reporting (Prometheus endpoints)
- Health check endpoints and monitoring
- Configuration validation enhancements
- Extended technical indicator library

### v2.0.0
- Multi-exchange support
- Advanced order types (iceberg, TWAP, VWAP)
- Machine learning signal enhancement
- Portfolio-level risk optimization
- Real-time dashboard and analytics

## Development Guidelines

### Engineering Preferences (from plan-eng-review skill)
- DRY is important—flag repetition aggressively
- Well-tested code is non-negotiable
- "Engineered enough" — not under-engineered or over-engineered
- Handle more edge cases, not fewer
- Bias toward explicit over clever
- Minimal diff: achieve goal with fewest new abstractions

### Rust Best Practices (from rust-pro skill)
- Leverage type system for compile-time correctness
- Prioritize memory safety without sacrificing performance
- Use zero-cost abstractions and avoid runtime overhead
- Implement explicit error handling with Result types
- Write comprehensive tests including property-based tests
- Follow Rust idioms and community conventions
- Document unsafe code blocks with safety invariants

### Async Patterns (from rust-async-patterns skill)
- Use Tokio runtime for async I/O
- Implement proper error handling in async contexts
- Use channels (mpsc, broadcast, watch) for communication
- Handle backpressure and flow control
- Optimize async performance
- Debug async code issues systematically

## Context for Next Developer

### Critical Constraints
- **Do not modify** Tokio runtime settings without performance testing
- **Maintain** separation of concerns between modules
- **Preserve** low-latency design principles in hot paths
- **Keep** dry-run mode as default execution mode for safety
- **Do not remove** circuit breaker functionality - critical for production stability

### Performance Constraints
- Latency target: <1ms for indicator calculations
- Throughput: 10,000+ trades/second
- Memory: <100MB typical usage
- CPU: Single-core sufficient for multi-pair monitoring

### Known Failures
1. WebSocket reconnection exponential backoff tuning
2. Volume profile calculation resets per candle (loses historical context)
3. Configuration validation edge cases
4. Error handling could use more specific types
5. Unused imports in some modules

## ASCII Diagrams for Key Flows

### Order Lifecycle
```
Trade Signal → Risk Validation → Order Placement → 
Order Tracking → Fill Processing → Position Update → 
Exit Signal Generation → Position Close
```

### Market Data Processing
```
WebSocket Trade → Candle Aggregation → 
Completed Candle → POC Calculation → 
Indicator Updates → Signal Generation
```

### Circuit Breaker States
```
Normal → (latency/failure threshold exceeded) → Tripped → 
(shutdown signal sent) → Bot Graceful Shutdown
```

## Production Failure Scenarios

1. **Network Partition**: WebSocket disconnects - handled by reconnection logic
2. **Exchange API Downtime**: Order placement fails - handled by circuit breaker and TTL
3. **Memory Leak**: Gradual memory increase - mitigated by bounded data structures
4. **Logic Error in Strategy**: Incorrect signal generation - mitigated by risk validation
5. **State Corruption**: Rare but possible - mitigated by async boundaries and testing

## Security Architecture

- **Authentication**: API keys managed through configuration (not checked into repo)
- **Data Protection**: No sensitive data stored persistently
- **Network Security**: Outbound HTTPS/WSS connections only
- **Rate Limiting**: Respects exchange rate limits through built-in throttling
