# CVDTrader Improvement Implementation Plan

Based on the engineering review, the following improvements have been approved for implementation:

## Architecture Improvements

### 1. WebSocket Reconnection - Add Jitter and Subscription Tracking
**Status**: Pending
**Files to modify**:
- `cvdtrader-market-data/src/websocket.rs`
- `cvdtrader-core/src/config.rs`

**Changes**:
- Add random jitter to exponential backoff (0-50% of delay)
- Track subscription state per symbol
- Only resubscribe to symbols that lost subscription

### 2. Health Check Endpoint
**Status**: Pending
**Files to create**:
- `cvdtrader-bot/src/health.rs`

**Files to modify**:
- `cvdtrader-bot/src/orchestrator.rs`
- `cvdtrader-bot/Cargo.toml`

**Changes**:
- Add axum HTTP server for health endpoint
- Expose /health endpoint with connection status, last message timestamp
- Add health check port to config

### 3. Channel Buffer Sizing - Make Configurable
**Status**: Pending
**Files to modify**:
- `cvdtrader-core/src/config.rs`
- `cvdtrader-bot/src/orchestrator.rs`
- `config.toml`

**Changes**:
- Add `trade_buffer_size` and `candle_buffer_size` to config
- Use config values instead of hardcoded 1000/100

## Code Quality Improvements

### 4. Account Balance - Hybrid Approach
**Status**: Pending
**Files to modify**:
- `cvdtrader-core/src/config.rs`
- `cvdtrader-risk/src/manager.rs`
- `cvdtrader-bot/src/orchestrator.rs`
- `cvdtrader-execution/src/gateway.rs`
- `config.toml`

**Changes**:
- Add `account_balance` to RiskConfig
- Add method to fetch balance from exchange API
- Use config value for dryrun, fetch from API for live/testnet

### 5. Tick Size - Fetch from Exchange
**Status**: Pending
**Files to modify**:
- `cvdtrader-market-data/src/websocket.rs`
- `cvdtrader-strategy/src/cvd_poc.rs`
- `cvdtrader-strategy/src/signals.rs`

**Changes**:
- Fetch metadata from exchange on startup
- Store tick sizes per symbol
- Pass to SignalEvaluator

### 6. Specific Error Types for Critical Paths
**Status**: Pending
**Files to create**:
- `cvdtrader-execution/src/errors.rs`

**Files to modify**:
- `cvdtrader-execution/src/lib.rs`
- `cvdtrader-execution/src/gateway.rs`

**Changes**:
- Define ExecutionError enum (Network, Validation, Exchange, Timeout)
- Update gateway to return specific errors
- Keep anyhow for non-critical paths

## Test Improvements

### 7. Full Integration Test Suite
**Status**: Pending
**Files to create**:
- `tests/integration_test.rs`
- `tests/bot_lifecycle_test.rs`
- `tests/signal_to_order_test.rs`

**Changes**:
- Test full bot startup/shutdown
- Test component interaction
- Test concurrent access patterns

### 8. Comprehensive Failure Mode Tests
**Status**: Pending
**Files to create**:
- `tests/failure_modes_test.rs`

**Changes**:
- Test WebSocket disconnection scenarios
- Test order placement timeout
- Test circuit breaker trip
- Test GlobalState concurrent access
- Test exchange API rate limiting

## Performance Improvements

### 9. TTL Checker - Make Configurable
**Status**: Pending
**Files to modify**:
- `cvdtrader-core/src/config.rs`
- `cvdtrader-execution/src/ttl.rs`
- `cvdtrader-bot/src/orchestrator.rs`
- `config.toml`

**Changes**:
- Add `ttl_check_interval` to ExecutionConfig
- Use config value instead of hardcoded 10 seconds

## Implementation Order

1. ✅ Config updates (support all new fields)
2. ✅ Error types (foundation for other changes)
3. ✅ Account balance hybrid approach
4. ✅ Tick size from exchange
5. ✅ Channel buffer sizing
6. ✅ TTL checker configurable
7. ✅ WebSocket jitter and subscription tracking
8. ✅ Health check endpoint
9. ✅ Integration tests
10. ✅ Failure mode tests

## Estimated Effort

- ✅ Config updates: 30 minutes
- ✅ Error types: 45 minutes
- ✅ Account balance: 1 hour
- ✅ Tick size: 1 hour
- ✅ Channel buffer: 30 minutes
- ✅ TTL checker: 30 minutes
- ✅ WebSocket improvements: 1.5 hours
- ✅ Health check: 1.5 hours
- ✅ Integration tests: 2 hours
- ✅ Failure mode tests: 2 hours

**Completed**: ~10.75 hours
**Remaining**: 0 hours
