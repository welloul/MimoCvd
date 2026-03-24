# CVDTrader Execution Module Documentation

## Responsibility
The cvdtrader-execution module handles order execution, fill tracking, and time-to-live (TTL) management for orders. It provides the interface between the trading strategy and the exchange, managing the complete order lifecycle from placement to fill confirmation or cancellation.

## Key Logic & Functions

### Execution Gateway (gateway.rs)
- **ExecutionGateway**: Main interface for placing and managing orders on the exchange
- **Modes of Operation**:
  - **DryRun**: Simulates order placement and immediate filling (for testing)
  - **TestNet/Live**: Places real orders via Hyperliquid API
- **Key Features**:
  - Order placement from TradeSignal to Exchange order format
  - Order cancellation functionality
  - Position closing via market orders
  - Post-only order support (maker-only orders)
  - Integration with GlobalState for order tracking
  - Comprehensive error handling and logging
  - API request/response handling with proper error conversion

### Error Types (errors.rs)
- **ExecutionError**: Specific error types for execution operations
- **Error Categories**:
  - **Network**: Connection issues (retryable)
  - **Validation**: Order validation failures (not retryable)
  - **Exchange**: API errors with error codes (conditionally retryable)
  - **Timeout**: Request timeouts (retryable)
  - **RateLimited**: Rate limit exceeded (retryable with backoff)
  - **InsufficientBalance**: Not enough funds (not retryable)
  - **Rejected**: Order rejected by exchange (not retryable)
  - **PositionNotFound**: Position doesn't exist (not retryable)
  - **OrderNotFound**: Order doesn't exist (not retryable)
- **Key Methods**:
  - is_retryable(): Check if error can be retried
  - retry_delay_secs(): Get recommended retry delay

#### Order Placement Process
1. Convert TradeSignal to OrderRequest format for Hyperliquid API
2. Set order parameters (symbol, side, price, size, order type)
3. Apply execution mode logic:
   - DryRun: Simulate immediate fill
   - TestNet/Live: Send HTTP request to exchange API
4. Update order status based on response
5. Store order in GlobalState for tracking
6. Return placed Order object

#### Order Cancellation
1. Retrieve order from GlobalState by ID
2. Apply execution mode logic:
   - DryRun: Simulate cancellation
   - TestNet/Live: Send cancel request to exchange API
3. Update order status to Cancelled
4. Update GlobalState with cancelled order

#### Position Closing
1. Retrieve position from GlobalState by symbol
2. Apply execution mode logic:
   - DryRun: Simulate position close
   - TestNet/Live: Send market close order to exchange API
3. Remove position from GlobalState

### Fill Tracker (fills.rs)
- **FillTracker**: Processes order fill events and updates positions/orders accordingly
- **Event-Driven**: Uses mpsc channels for asynchronous fill processing
- **Key Features**:
  - Background task that continuously processes fill events
  - Order status updates based on fill information
  - Position creation/update from fill events
  - Comprehensive logging and error handling
  - Test simulation capabilities

#### Fill Processing Flow
1. Receive FillEvent via mpsc channel
2. Locate corresponding order in GlobalState
3. Update order with fill information (filled size/price, status)
4. Create or update position based on fill
5. Log processing results
6. Handle edge cases (missing orders, etc.)

### TTL Tracker (ttl.rs)
- **OrderTtlTracker**: Automatically cancels orders that exceed their time-to-live
- **Periodic Checking**: Uses tokio::time::interval for regular checks (configurable interval)
- **Key Features**:
  - Configurable TTL duration (default: 120 seconds)
  - Configurable check interval (ttl_check_interval_secs, default: 10 seconds)
  - Background task that runs independently
  - Shutdown handling via broadcast channels
  - Only cancels orders in Pending status
  - Comprehensive logging of cancellation events

#### TTL Checking Process
1. Wake up on interval tick (configurable, default: 10 seconds)
2. Retrieve all orders from GlobalState
3. For each Pending order:
   - Check if order.is_expired(ttl_seconds)
   - If expired: log warning, update status to Cancelled, update GlobalState
4. Continue monitoring until shutdown signal received

## The "Hurdles"
### Known Limitations
1. **API Coupling**: Directly coupled to Hyperliquid API format - would require changes for other exchanges
2. **DryRun Limitations**: DryRun mode simulates immediate fills - doesn't test order lifecycle or partial fills
3. **TTL Granularity**: TTL checking every 10 seconds could lead to up to 10 seconds of excess latency
4. **Error Handling Simplification**: Some API errors are converted to generic anyhow::Error rather than specific types
5. **Order ID Assumptions**: Assumes order IDs are unique and persistent - relies on exchange guarantees

### Technical Debt
1. **Hardcoded API Endpoints**: Uses "/exchange" endpoint directly rather than configurable base
2. **Order Type Hardcoding**: Post-only and IOC order types hardcoded rather than configurable
3. **Limited Retry Logic**: API failures don't implement retry mechanisms with exponential backoff
4. **Sync/Async Boundaries**: Some blocking operations in async contexts (though minimal)
5. **Configuration Access**: Some config values accessed indirectly through parameters rather than direct config

### Performance Considerations
1. **HTTP Client Reuse**: Uses reqwest::Client which is efficient for connection reuse
2. **JSON Serialization**: Order serialization/deserialization could be optimized for frequency
3. **Channel Buffer Sizes**: FillTracker uses fixed-size channels (100) - may need tuning under high fill rates
4. **TTL Interval**: Fixed 10-second interval - could be made configurable
5. **State Access Frequency**: TTL tracker accesses GlobalState every 10 seconds - adds minimal load

## Future Roadmap
### Immediate Improvements
1. Add configurable API endpoints and order types
2. Implement retry logic with exponential backoff for API failures
3. Enhance DryRun mode to simulate realistic order lifecycles (pending → filled/cancelled)
4. TTL checking interval now configurable (ttl_check_interval_secs)
5. Specific error types added (ExecutionError enum)

### Medium-term Enhancements
1. Add order amendment/modification capabilities
2. Implement advanced order types (stop-loss, take-profit, trailing stops)
3. Add exchange rate limit awareness and throttling
4. Implement order batching for improved efficiency
5. Add exchange-specific fee calculation and tracking

### Long-term Goals
1. Pluggable exchange abstraction layer (support multiple exchanges)
2. Advanced order management (iceberg orders, TWAP/VWAP execution)
3. Real-time exchange connectivity monitoring and failover
4. Exchange fee optimization and rebate tracking
5. Institutional-grade order execution algorithms