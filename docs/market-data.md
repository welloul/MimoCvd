# CVDTrader Market Data Module Documentation

## Responsibility
The cvdtrader-market-data module handles all market data processing, including WebSocket connectivity, candle building, volume profile calculations, VWAP tracking, and technical indicator computations. It transforms raw trade data into actionable market intelligence for the strategy layer.

## Key Logic & Functions

### WebSocket Connection (websocket.rs)
- **HyperliquidWs**: Main WebSocket client for Hyperliquid exchange
- **Connection Management**: Automatic reconnection with exponential backoff (with jitter for better distribution)
- **Subscription Handling**: Subscribes to trade data for configured symbols (tracks subscribed symbols to avoid duplicates)
- **Message Parsing**: Converts WebSocket messages to internal Trade structs
- **Error Handling**: Comprehensive error handling with logging and recovery
- **Metadata Fetching**: Fetches exchange metadata (tick sizes) using hyperliquid-rust-sdk InfoClient
- **Key Features**:
  - Async connection using tokio-tungstenite
  - Automatic reconnection with configurable retry limits and jitter
  - Graceful shutdown handling via broadcast channels
  - Trade data parsing and validation
  - Latency-insensitive message processing
  - Per-symbol tick size management from exchange metadata

### Candle Builder (candle_builder.rs)
- **CandleBuilder**: Aggregates individual trades into time-based candles
- **1-Minute Candles**: Standard candle size for strategy processing
- **Real-time Updates**: Updates candle OHLCV as trades arrive
- **Completed Candle Detection**: Identifies when a candle period ends
- **Global CVD Tracking**: Updates cumulative volume delta in global state
- **Key Features**:
  - Efficient hashmap-based current candle tracking
  - Automatic finalization and emission of completed candles
  - Integration with global state for CVD updates
  - Memory-efficient candle storage (keeps recent candles only)
  - Test coverage for single/multi-candle scenarios

### Volume Profile (volume_profile.rs)
- **VolumeProfileBuilder**: Calculates Point of Control (POC) from volume distribution
- **Price Binning**: Groups trades into price bins using configurable tick size
- **Volume Aggregation**: Sums volume at each price level
- **POC Calculation**: Identifies price level with maximum volume
- **Key Features**:
  - Configurable tick size for different instruments (now fetched from exchange metadata)
  - Efficient hashmap-based volume profiling
  - Per-symbol volume profiles (strategy maintains separate builders per symbol)
  - POC calculation on demand (when candle is completed)
  - Memory cleanup mechanisms (clear symbol/all)
  - Comprehensive test suite

### VWAP Tracker (vwap.rs)
- **DailyVWAPTracker**: Calculates volume-weighted average price with daily reset
- **Cumulative Calculation**: Maintains running PV (price*volume) and volume sums
- **Daily Reset**: Automatically resets at midnight UTC
- **Symbol Tracking**: Independent VWAP calculation per symbol
- **Key Features**:
  - Date-based reset detection
  - Efficient incremental updates
  - Thread-safe-ish design (single-threaded access assumed)
  - Symbol-specific tracking
  - Reset capabilities (symbol-specific and global)
  - Extensive test coverage including daily reset scenarios

### Indicators (indicators.rs)
- **IndicatorCompute**: Calculates various technical indicators
- **Global CVD**: Tracks cumulative volume delta per symbol
- **CVD History**: Maintains historical CVD values for percentile calculations
- **Volume History**: Tracks historical volume for RVOL calculations
- **Configurable History**: Adjustable lookback periods for calculations
- **Key Features**:
  - Global CVD tracking and updates
  - CVD percentile calculation (0.0 to 1.0 scale)
  - CVD in top percentile detection
  - Relative Volume (RVOL) calculation
  - Average CVD magnitude calculation
  - History management with automatic trimming
  - Symbol-specific data isolation
  - Clear/reset functionality
  - Comprehensive test suite

## The "Hurdles"
### Known Limitations
1. **WebSocket Backpressure**: If processing lags behind message rate, memory usage could grow in channels
2. **Volume Profile Reset**: Currently resets per candle - loses historical volume context between candles
3. **Indicator History Bounded**: Fixed maximum history may not suit all trading strategies
4. **VWAP Drift**: No mechanism to handle exchange time drift or daylight saving changes
5. **Tick Size Assumption**: Volume profile uses fixed tick size rather than dynamic exchange-provided values

### Technical Debt
1. **WebSocket Subscription State**: Now tracks subscribed symbols to avoid duplicate subscriptions on reconnection
2. **Candle Finalization POC**: Currently placeholder - POC calculation delegated to VolumeProfileBuilder but could be integrated
3. **Indicator Redundancy**: Some duplication between IndicatorCompute and strategy-specific calculations
4. **Error Propagation**: Some functions use unwrap()/expect() in non-test code
5. **Configuration Coupling**: Some modules access config values indirectly through parameters rather than direct config access

### Performance Considerations
1. **Channel Sizing**: mpsc channels use fixed buffer sizes (1000 for trades, 100 for candles) - may need tuning
2. **Hashmap Resizing**: Frequent hashmap operations could cause reallocations under high load
3. **String Operations**: WebSocket message parsing involves string allocations - could be optimized with custom parsers
4. **Clone Operations**: Trade structs are cloned when passed between components - could use references where appropriate
5. **Lock Contention**: Global state updates from market data module could cause contention under extreme load

## Future Roadmap
### Immediate Improvements
1. Implement true POC calculation in candle finalization step
2. WebSocket subscription state tracking implemented (tracks subscribed symbols)
3. Enhance error handling to use custom error types where appropriate
4. Add metrics collection for message processing latency
5. Configurable channel buffer sizes implemented (via BotConfig)

### Medium-term Enhancements
1. Replace volume profile reset-per-candle with rolling window approach
2. Adaptive tick size resolution from exchange metadata implemented
3. Implement exchange time synchronization for VWAP reset accuracy
4. Add historical data replay capability for strategy testing
5. Optimize hashmap usage with pre-sizing where possible

### Long-term Goals
1. Pluggable data sources (WebSocket, REST, file-based for testing)
2. Advanced technical indicator library (TA-Lib integration or custom)
3. Real-time market data visualization/debugging tools
4. Machine learning feature extraction pipeline
5. Multi-timeframe analysis capabilities