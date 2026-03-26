# CVDTrader Changelog

## [Unreleased]
### Added
- Initial implementation of CVDTrader low-latency trading bot
- Complete modular architecture with separation of concerns
- CVDPoC (Cumulative Volume Delta - Point of Control) strategy
- Comprehensive risk management with position limits, leverage constraints, and circuit breaker
- Low-latency market data processing pipeline (WebSocket → candle building → indicators)
- Execution gateway with dry-run, testnet, and live modes
- Fill tracking and order TTL management
- Comprehensive test suite for all modules
- Structured logging with JSON and pretty output formats
- TOML-based configuration system
- Graceful shutdown handling

### Changed
- **Entry Price Calculation**: Changed from percentage-based to tick-based offsets
  - LONG signals now enter at POC + 1 tick
  - SHORT signals now enter at POC - 1 tick
  - Improved precision for different trading symbols with varying tick sizes

### Fixed
- **POC Calculation Bug**: Fixed incorrect POC values for ARB, DOGE, SUI by using appropriate tick sizes (0.00001) instead of coarse size-based decimals
- **Tick Size Retrieval Bug**: Fixed WebSocket metadata parsing to use proper price precision for volume profile binning
- **SignalEvaluator Constructor**: Updated all test cases to match new constructor signature (removed entry_offset_pct parameter)
- **Compilation Errors**: Fixed 13+ compilation errors in strategy tests after constructor changes

## [Session: March 24, 2026]
### Added
- **Trade History Persistence**: SQLite-based trade recording system
  - TradeRecord struct for completed trades (entry/exit times, PnL, exit reason)
  - TradeHistory module with database operations (record, query, statistics)
  - Automatic trade recording when positions close
  - Trade statistics calculation (win rate, profit factor, max win/loss)
- **Health Check Endpoint**: HTTP endpoint for monitoring bot status
  - /health endpoint with connection status, last message timestamp
  - Active positions count, pending orders count
  - WebSocket connection status
- **Execution Error Types**: Specific error types for execution operations
  - Network, Validation, Exchange, Timeout, RateLimited errors
  - Retryable vs non-retryable error classification
  - Retry delay recommendations
- **Configurable Parameters**:
  - Account balance for risk calculations (configurable in dryrun mode)
  - Channel buffer sizes (trade_buffer_size, candle_buffer_size)
  - TTL check interval (ttl_check_interval_secs)
  - Health check port configuration
- **Integration Tests**: Comprehensive test suite for bot lifecycle
  - Bot startup/shutdown tests
  - Global state operations tests
  - Concurrent access tests
  - Configuration validation tests
- **Failure Mode Tests**: Tests for production failure scenarios
  - Circuit breaker trip scenarios
  - Order TTL expiration
  - Risk manager validation edge cases
  - Concurrent state access under load

### Changed
- **RiskManager**: Now accepts account_balance and execution_mode parameters
- **Bot::new()**: Now returns Result<Self> to handle TradeHistory initialization
- **Orchestrator**: Integrated trade recording when positions close
- **WebSocket**: Added jitter to exponential backoff for reconnection
- **CvdPocStrategy**: Updated to use per-symbol tick sizes from exchange metadata

### Fixed
- **Unused Imports**: Cleaned up unused imports in cvd_poc.rs (Side, Signal, DateTime)
- **Test Fixtures**: Updated tests to use correct function signatures

## Version History
This is the initial release of the CVDTrader project. All features listed above are part of the initial implementation.

## Planned Future Releases
### v1.1.0
- Configuration hot-reloading capability
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