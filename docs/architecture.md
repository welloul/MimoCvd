# CVDTrader Architecture Overview

## High-Level Design

CVDTrader follows a modular, layered architecture designed for low-latency trading operations. Each module has a single responsibility and communicates through well-defined interfaces.

### Module Dependencies
```
┌─────────────────────────────────────────────────────────────┐
│                    RUST TRADING BOT                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │ Market Data  │───▶│  Indicators  │───▶│   Strategy   │  │
│  │   Engine     │    │    Engine    │    │    Engine    │  │
│  └──────────────┘    └──────────────┘    └──────────────┘  │
│         │                   │                   │           │
│         ▼                   ▼                   ▼           │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │  WebSocket   │    │  CVD/POC/    │    │  Signal      │  │
│  │  Connection  │    │  VWAP Calc   │    │  Evaluation  │  │
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
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow Patterns

1. **Market Data Ingestion**:
   - WebSocket connection receives trade data from Hyperliquid
   - Trade data is sent via mpsc channel to CandleBuilder
   - CandleBuilder aggregates trades into 1-minute candles
   - Completed candles are sent via mpsc channel to strategy

2. **Signal Processing**:
   - Strategy receives completed candles
   - Calculates POC using VolumeProfileBuilder
   - Updates indicators using IndicatorCompute
   - Generates trading signals if conditions are met
   - Validates signals with RiskManager
   - Places orders via ExecutionGateway

3. **Position Management**:
   - Strategy monitors existing positions for exit conditions
   - Manages position exits based on CVD behavior
   - Updates global state with position changes
   - ExecutionGateway handles order placement/cancellation

4. **Risk Management**:
   - RiskManager validates all signals before execution
   - CircuitBreaker monitors latency and failure rates
   - OrderTtlTracker automatically cancels stale orders
   - FillTracker processes order fills and updates positions

## Component Boundaries

### cvdtrader-core
- **Responsibility**: Shared types, state management, configuration
- **Key Components**: 
  - Types (Trade, Candle, Position, Order, etc.)
  - GlobalState (thread-safe shared state)
  - Config (TOML-based configuration)
  - Result/Error types

### cvdtrader-market-data
- **Responsibility**: Market data processing pipeline
- **Key Components**:
  - WebSocket (Hyperliquid connection)
  - CandleBuilder (trade aggregation)
  - VolumeProfileBuilder (POC calculation)
  - VWAPTracker (volume-weighted average price)
  - Indicators (CVD, RVOL, etc.)

### cvdtrader-strategy
- **Responsibility**: Trading strategy implementation
- **Key Components**:
  - CvdPocStrategy (main strategy logic)
  - SignalEvaluator (signal generation logic)
  - SignalGenerator (trait for signal evaluation)

### cvdtrader-execution
- **Responsibility**: Order execution and tracking
- **Key Components**:
  - ExecutionGateway (order placement/cancellation)
  - FillTracker (order fill processing)
  - OrderTtlTracker (automatic order cancellation)

### cvdtrader-risk
- **Responsibility**: Risk management and safety systems
- **Key Components**:
  - RiskManager (position limits, leverage, drawdown)
  - CircuitBreaker (latency and failure monitoring)

### cvdtrader-bot
- **Responsibility**: Main orchestrator and event loop
- **Key Components**:
  - Bot (main struct coordinating all components)
  - Event loop (trade/candle processing)
  - Component initialization and shutdown

## Scaling Characteristics

- **Horizontal Scaling**: Not designed for horizontal scaling - single instance per trading pair
- **Vertical Scaling**: Can handle multiple trading pairs on single instance
- **Bottlenecks**: 
  - WebSocket message processing (single-threaded per connection)
  - Global state access (mitigated with async RwLocks)
  - Signal computation (optimized for <1ms latency)

## Single Points of Failure

1. **WebSocket Connection**: If connection drops and reconnection fails, bot stops receiving data
2. **Global State Corruption**: Though protected by RwLocks, panic in state handling could cause issues
3. **Execution Gateway Failure**: If order placement consistently fails, bot may accumulate risk

## Security Architecture

- **Authentication**: API keys managed through configuration (not checked into repo)
- **Data Protection**: No sensitive data stored persistently
- **Network Security**: Outbound HTTPS/WSS connections only
- **Rate Limiting**: Respects exchange rate limits through built-in throttling

## Key Flows Requiring ASCII Diagrams

The architecture diagram above illustrates the main data flow. Additional important flows:

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

## Production Failure Scenarios

1. **Network Partition**: WebSocket disconnects - handled by reconnection logic
2. **Exchange API Downtime**: Order placement fails - handled by circuit breaker and TTL
3. **Memory Leak**: Gradual memory increase - mitigated by bounded data structures
4. **Logic Error in Strategy**: Incorrect signal generation - mitigated by risk validation
5. **State Corruption**: Rare but possible - mitigated by async boundaries and testing