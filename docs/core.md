# CVDTrader Core Module Documentation

## Responsibility
The cvdtrader-core module provides the foundational building blocks for the entire trading bot system. It contains shared types, state management mechanisms, and configuration handling that are used by all other modules.

## Key Logic & Functions

### Types System
The module defines all core data structures used throughout the bot:

#### Trade
Represents a single trade from the exchange with:
- Symbol (trading pair)
- Price and size
- Side (Buy/Sell)
- Timestamp
- Delta calculation (positive for buy, negative for sell)

#### TradeRecord
Represents a completed trade with entry and exit details:
- Unique ID (UUID)
- Symbol and side (Long/Short)
- Size, entry price, exit price
- Entry time and exit time
- Realized PnL and PnL percentage
- Exit reason (StopLoss, TakeProfit, CvdFlip, Manual)
- Setup type (Exhaustion, Absorption)
- Methods for duration calculation and profitability check

#### Candle
OHLCV data with additional CVD and POC fields:
- Open, High, Low, Close prices
- Volume and Cumulative Volume Delta (CVD)
- Point of Control (POC) from volume profile
- Timestamp and associated trades
- Methods for adding trades and calculating derived values

#### Position
Tracks open trading positions:
- Unique ID, symbol, and side (Long/Short)
- Size, entry price, stop loss, take profit
- Unrealized PnL and flip streak tracking
- Methods for PnL updates and stop loss/take profit checks

#### Order
Represents orders sent to the exchange:
- ID, symbol, side, price, size
- Status (Pending, Filled, PartiallyFilled, Cancelled, Rejected)
- Fill tracking and expiration logic

#### Enumerations
- Side/OrderSide/PositionSide (Buy/Sell, Long/Short)
- OrderStatus (Pending, Filled, etc.)
- ExecutionMode (DryRun, TestNet, Live)
- Signal (Long, Short, None)
- SetupType (Exhaustion, Absorption)
- ExitReason (StopLoss, TakeProfit, CvdFlip, Manual)

#### TradeSignal
Combines signal information with trade parameters:
- Signal type and setup classification
- Entry price, stop loss, take profit
- Position size and timestamp
- Validation logic

### State Management
GlobalState provides thread-safe shared state using async RwLocks:

#### Components
- Positions: Symbol → Position mapping
- Orders: OrderID → Order mapping  
- Candles: Symbol → Vec<Candle> (last 100 candles per symbol)
- Global CVD: Symbol → cumulative CVD value
- Bot running state and last update timestamp

#### Key Operations
- Position/get/set/remove operations
- Order tracking and retrieval
- Candle storage and retrieval (last N candles)
- Global CVD updates and queries
- State clearing (for testing/reset)

### Configuration
Config module handles TOML-based configuration with validation:

#### Configuration Sections
- Exchange: WebSocket/API URLs, symbols
- Strategy: Lookback periods, CVD thresholds, risk parameters
- Risk: Position limits, leverage, drawdown, circuit breaker settings
- Execution: Mode, TTL, post-only flag
- Logging: Level and format

#### Validation
- Symbol lists must not be empty
- URLs must be configured
- Numeric values within valid ranges
- Log levels and formats validated

### Result Types
Consistent error handling throughout:
- Result<T> = anyhow::Result<T>
- Error = anyhow::Error
- Provides ergonomic error propagation with context

## The "Hurdles"
### Known Limitations
1. **Configuration Hot Reloading**: Not implemented - requires bot restart for config changes
2. **Complex Type Serialization**: Some types use serde but may need custom serialization for specific use cases
3. **State Persistence**: GlobalState is purely in-memory - no persistence/restore mechanism
4. **UUID Generation**: Position/Order IDs use UUIDv4 - acceptable but could be optimized for performance

### Technical Debt
1. **Tick Size Resolution**: Tick sizes are now fetched from exchange metadata on startup (via WebSocket.fetch_metadata())
2. **Validation Error Messages**: Could be more specific about which validation failed
3. **Default Values**: Some defaults may not be optimal for all trading pairs/scenarios

### Performance Considerations
1. **GlobalState Lock Contention**: High-frequency access could cause contention - mitigated by async design and short critical sections
2. **Memory Allocation**: Frequent cloning of types in state operations - could be optimized with references where appropriate
3. **String Operations**: Some string manipulation in enums for display - minimal impact but noted

## Future Roadmap
### Immediate Improvements
1. Add configuration hot-reloading capability
2. Implement state persistence/snapshot mechanism
3. Enhance tick size resolution to fetch from exchange
4. Add more specific error types for better error handling

### Medium-term Enhancements
1. Optimize GlobalState for reduced lock contention
2. Add metrics collection for state access patterns
3. Implement configurable history lengths for different data types
4. Add schema versioning to configuration files

### Long-term Goals
1. Pluggable state backends (memory, Redis, database)
2. Advanced configuration environments (dev/staging/prod)
3. Comprehensive type serialization framework
4. Performance profiling and optimization tooling