# CVDTrader Project Handover Document

## Project Status
- **Current Phase**: Production-ready implementation
- **Stability**: Stable core functionality with comprehensive test coverage
- **Last Updated**: March 2026
- **Primary Language**: Rust 1.75+
- **Build System**: Cargo

## Context Injection for Next Developer
- **Do not modify** the Tokio runtime settings in `cvdtrader-bot/src/orchestrator.rs` without performance testing
- **Maintain** the separation of concerns between modules as defined in the architecture
- **Preserve** the low-latency design principles in hot paths (market data processing, signal generation)
- **Keep** the dry-run mode as the default execution mode for safety
- **Do not remove** the circuit breaker functionality - it's critical for production stability
- **Entry pricing is now tick-based**: LONG signals enter at POC + 1 tick, SHORT at POC - 1 tick - do not revert to percentage-based offsets
- **Tick sizes are fetched from exchange metadata**: Use `px_decimals` field, not `sz_decimals` for price-based tick calculations

## Known Failures & Technical Debt
1. **WebSocket Reconnection**: While implemented, the exponential backoff could be tuned for specific network conditions (jitter added for better distribution)
2. **Volume Profile Calculation**: Currently resets per candle - loses historical volume context between candles (now uses per-symbol builders)
3. **Configuration Validation**: Some edge cases in config validation could be strengthened
4. **Error Handling**: Some API calls in execution gateway could benefit from more specific error types (ExecutionError enum added)
5. **Unused Imports**: Some modules have unused imports (e.g., cvd_poc.rs) - cosmetic issue, doesn't affect functionality (cleaned up)
6. **Entry Price Precision**: Now uses tick-based offsets instead of percentage-based for precise positioning relative to POC

## Performance Constraints
- **Latency Target**: <1ms for indicator calculations
- **Throughput**: Designed for 10,000+ trades/second processing
- **Memory Usage**: <100MB typical usage
- **CPU**: Single-core sufficient for multi-pair monitoring
- **TTL Settings**: Order TTL default 120 seconds (configurable)
- **Circuit Breaker**: Latency threshold 500ms, failure threshold 3 consecutive failures

## Critical Architecture Notes
- The bot uses a layered architecture with clear separation:
  - Market Data Layer (WebSocket → Candle Building → Indicators)
  - Strategy Layer (Signal Generation → Position Management)
  - Risk Layer (Validation → Circuit Breaker)
  - Execution Layer (Order Placement → Fill Tracking → TTL)
  - State Layer (Global state management with async RwLocks)
- All communication between layers uses async channels for backpressure handling
- The design prioritizes fault isolation - failure in one layer doesn't crash the entire bot