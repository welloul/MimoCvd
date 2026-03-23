# CVDTrader Risk Module Documentation

## Responsibility
The cvdtrader-risk module provides risk management and safety systems for the trading bot. It enforces trading constraints, monitors system health, and implements circuit breaker functionality to prevent catastrophic losses and ensure stable operation under adverse conditions.

## Key Logic & Functions

### Risk Manager (manager.rs)
- **RiskManager**: Enforces trading constraints and validates signals before execution
- **Key Validation Checks**:
  1. **Position Size Limit**: Ensures individual position value doesn't exceed max_position_usd
  2. **Leverage Limit**: Calculates total exposure and ensures leverage stays below max_leverage
  3. **Drawdown Limit**: Monitors unrealized PnL and prevents trading if drawdown exceeds max_drawdown_pct
  4. **Position Existence**: Prevents opening new positions when one already exists for a symbol
- **Key Features**:
  - Thread-safe access to GlobalState for position/exposure calculations
  - Configurable limits for all risk parameters
  - Comprehensive logging of validation decisions
  - Returns detailed error messages for failed validations
  - Account balance tracking for leverage calculations

#### Validation Process
1. Calculate position value (entry_price * size)
2. Check against max_position_usd limit
3. Calculate total exposure (current positions + new position)
4. Calculate leverage (total_exposure / account_balance)
5. Check against max_leverage limit
6. Calculate current drawdown (negative PnL / account_balance)
7. Check against max_drawdown_pct limit
8. Check for existing position in symbol
9. Return success or detailed failure reason

### Circuit Breaker (circuit_breaker.rs)
- **CircuitBreaker**: Monitors system health and halts trading on detected issues
- **Protection Mechanisms**:
  1. **High Latency Detection**: Trips if latency exceeds max_latency_ms threshold
  2. **Consecutive Failure Detection**: Trips if failures exceed max_failures threshold
  3. **Automatic Shutdown Signaling**: Sends broadcast signal to initiate graceful shutdown
- **Key Features**:
  - Async-safe state management using RwLocks and Arc
  - Latency monitoring and recording
  - Failure counting and tracking
  - Automatic reset capability
  - Shutdown signal propagation via broadcast channel
  - Comprehensive state tracking (last latency, failure count, timestamps)

#### Circuit Breaker States
- **Normal**: Standard operation, monitoring active
- **Tripped**: Protection activated, shutdown signal sent
- **Resetting**: Transition state during recovery (not fully implemented in current version)

#### Monitoring Process
1. **Latency Monitoring**:
   - Record latency measurements via record_latency()
   - Compare against max_latency_ms threshold
   - Trip if exceeded
2. **Failure Monitoring**:
   - Increment failure count via record_failure()
   - Compare against max_failures threshold
   - Trip if exceeded
   - Reset failure count via record_success()
3. **State Query**:
   - Check current state via state() or is_tripped()
   - Access last latency and failure counts
   - Manual reset capability

## The "Hurdles"
### Known Limitations
1. **Drawdown Calculation Simplification**: Uses unrealized PnL only - doesn't consider realized PnL or fees
2. **Leverage Calculation Assumption**: Assumes all positions use same collateral - doesn't account for margin requirements
3. **Circuit Breaker Granularity**: Monitors at bot level - doesn't isolate issues to specific components/symbols
4. **Reset Mechanism**: Manual reset required - no automatic recovery after cooling period
5. **Risk Manager Coupling**: Tightly coupled to specific GlobalState access patterns

### Technical Debt
1. **Hardcoded Account Balance**: RiskManager uses hardcoded 10000.0 balance rather than configurable value
2. **Limited Risk Metrics**: Only basic position size, leverage, and drawdown - no VaR, stress testing, etc.
3. **Circuit Breaker State**: Resetting state defined but not fully utilized in state transitions
4. **Error Handling**: Risk validation returns String errors rather than specific error types
5. **Configuration Access**: Some risk parameters accessed indirectly through parameters

### Performance Considerations
1. **State Access Frequency**: Risk validation accesses GlobalState for each signal - adds minimal latency
2. **Calculation Overhead**: Simple arithmetic operations - negligible performance impact
3. **Lock Contention**: Circuit breaker uses RwLocks - designed for high read, low write scenarios
4. **Memory Usage**: Minimal - only stores essential state variables
5. **Async Safety**: All state access is async-safe using proper locking mechanisms

## Future Roadmap
### Immediate Improvements
1. Make account balance configurable rather than hardcoded
2. Add more specific error types for risk validation failures
3. Implement automatic circuit breaker reset after cooling period
4. Enhance drawdown calculation to include realized PnL
5. Add leverage calculation that accounts for margin requirements

### Medium-term Enhancements
1. Add position correlation monitoring to prevent over-concentration
2. Implement value-at-risk (VaR) calculations for portfolio risk
3. Add stress testing capabilities for extreme market scenarios
4. Implement component-specific circuit breakers (isolate WebSocket vs strategy issues)
5. Add risk metrics reporting and monitoring capabilities

### Long-term Goals
1. Dynamic risk adjustment based on market volatility
2. Machine learning-based risk prediction and prevention
3. Portfolio-level risk optimization (not just per-symbol limits)
4. Regulatory compliance features (position reporting, audit trails)
5. Advanced order types for risk mitigation (iceberg, TWAP, etc.)