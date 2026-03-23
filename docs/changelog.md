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
- N/A (initial release)

### Fixed
- N/A (initial release)

## Version History
This is the initial release of the CVDTrader project. All features listed above are part of the initial implementation.

## Planned Future Releases
### v1.1.0
- Configuration hot-reloading capability
- Enhanced error handling with specific error types
- Improved circuit breaker with automatic reset
- VWAP integration in strategy signals
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