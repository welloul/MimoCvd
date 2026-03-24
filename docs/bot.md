# CVDTrader Bot Module Documentation

## Responsibility
The cvdtrader-bot module serves as the main orchestrator that coordinates all other modules into a cohesive trading system. It handles initialization, configuration, component lifecycle management, and the main event loop that processes market data and executes trades.

## Key Logic & Functions

### Bot Struct (orchestrator.rs)
- **Bot**: Main coordinator struct that holds references to all system components
- **Key Components**:
  - Config: System configuration
  - GlobalState: Shared state accessible to all components
  - TradeHistory: SQLite-based trade persistence
  - Shutdown Signal: Broadcast channel for graceful shutdown coordination
- **Responsibility**: Initialize components, start/stop systems, manage main event loop, record completed trades

### Initialization Process
1. **Bot::new(config)**:
   - Creates broadcast channel for shutdown signaling
   - Initializes fresh GlobalState
   - Initializes TradeHistory database (trades.db)
   - Stores configuration reference
   - Returns configured Bot instance (Result<Self>)

2. **Component Initialization (in start() method)**:
   - Logging initialization based on config
   - Channel creation for trade and candle data flow (configurable buffer sizes)
   - WebSocket connection setup (HyperliquidWs) with metadata fetching
   - CandleBuilder initialization with tick sizes from exchange
   - Strategy (CvdPocStrategy) initialization with per-symbol tick sizes
   - ExecutionGateway setup
   - FillTracker initialization
   - OrderTtlTracker setup with configurable check interval
   - RiskManager initialization with account balance and execution mode
   - CircuitBreaker initialization
   - Health check server initialization (configurable port)

### Main Event Loop (run_event_loop method)
The core processing loop that handles real-time trading operations:

#### Trade Processing Path
1. **Receive Trade**: From WebSocket via mpsc channel
2. **Latency Measurement**: Start timing for performance monitoring
3. **Candle Building**: Process trade through CandleBuilder
   - May return completed candle if minute boundary crossed
4. **Signal Generation**: If completed candle, process through strategy
   - Returns TradeSignal if conditions met
5. **Risk Validation**: Validate signal with RiskManager
6. **Order Placement**: If valid, place order via ExecutionGateway
7. **Exit Checking**: Check exit conditions for existing positions
8. **Position Close**: If exit signal, close position via gateway
9. **Latency Recording**: Record processing time in CircuitBreaker
10. **Circuit Breaker Check**: Monitor for trip conditions

#### Candle Processing Path
1. **Receive Completed Candle**: From CandleBuilder via mpsc channel
2. **Indicator Updates**: Update indicators with candle data (TODO in current implementation)

#### Shutdown Handling
1. **Broadcast Channel**: Listen for shutdown signal
2. **Ctrl+C Handling**: Handle keyboard interrupt
3. **Circuit Breaker Trip**: Automatic shutdown on protection activation
4. **Graceful Shutdown Sequence**:
   - Set bot running state to false
   - Wait for fill tracker to finish processing
   - Final cleanup and exit

### Logging Initialization (init_logging method)
- **Configuration-Driven**: Uses config.logging.level and format
- **Flexible Output**: Supports JSON, pretty, or compact formats
- **Rich Context**: Includes file, line number, thread IDs, and timestamps
- **Environment Override**: Supports RUST_LOG environment variable

### Accessor Methods
- **state()**: Provides read-only access to GlobalState
- **config()**: Provides read-only access to configuration

## The "Hurdles"
### Known Limitations
1. **Tight Coupling**: Bot has direct knowledge of all component types - makes plugging alternatives difficult
2. **Configuration Passing**: Some components receive config values indirectly rather than full config object
3. **Error Propagation**: Some errors in component initialization use unwrap() rather than proper error handling
4. **Account Balance**: Now configurable via RiskConfig.account_balance (hybrid approach)
5. **Channel Sizing**: Buffer sizes now configurable via BotConfig (trade_buffer_size, candle_buffer_size)
6. **Startup Sequence**: Health check server added, but no dependency validation during startup

### Technical Debt
1. **TODO Comments**: Multiple TODO items in event loop (indicator updates with candles, candle export to JSON)
2. **Magic Numbers**: Some hardcoded values remain (intervals, etc.) - channel sizes now configurable
3. **Resource Cleanup**: Shutdown sequence could be more robust in error scenarios
4. **Testing Infrastructure**: Integration tests added for bot lifecycle
5. **Configuration Validation**: Relies on individual component validation rather than holistic validation

### Performance Considerations
1. **Channel Contention**: High-frequency trading could cause channel backpressure under extreme load
2. **Async Task Management**: Multiple concurrent tasks (WebSocket, TTL tracker, fill tracker, main loop)
3. **Memory Usage**: Each task has its own stack and resources - could be optimized
4. **Lock Contention**: Frequent GlobalState access from multiple concurrent tasks
5. **Serialization Overhead**: JSON serialization for logging could be optimized

## Future Roadmap
### Immediate Improvements
1. Complete TODO items in event loop (candle-based indicator updates)
2. Channel sizes and account balance now configurable
3. Enhance error handling during component initialization
4. Health check server added (configurable port)
5. Implement more graceful error recovery in shutdown sequence

### Medium-term Enhancements
1. Implement plugin architecture for exchange/strategy/risk components
2. Add comprehensive metrics collection and reporting (Prometheus endpoints)
3. Implement configuration hot-reloading without restart
4. Health check endpoint added (/health on configurable port)
5. Optimize channel usage with dynamic sizing based on load

### Long-term Goals
1. Micro-services architecture for independent component scaling
2. Advanced orchestration with Kubernetes-style health checks and restart policies
3. Real-time dashboard for bot performance and system metrics
4. Machine learning-based performance optimization and auto-tuning
5. Multi-bot coordination and fleet management capabilities