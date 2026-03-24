# CVDTrader Strategy Module Documentation

## Responsibility
The cvdtrader-strategy module implements the CVDPoC (Cumulative Volume Delta - Point of Control) trading strategy. It processes market data to generate trading signals, manages position exits, and implements the core trading logic based on volume delta analysis and price action patterns.

## Key Logic & Functions

### Signal Evaluator (signals.rs)
The core of the strategy logic resides in the SignalEvaluator struct, which implements the SignalGenerator trait.

#### Swing Detection
- **is_new_swing_high/low**: Identifies when price breaks to new highs/lows over a lookback period
- **Lookback Period**: Configurable number of previous candles to compare against (default: 20)
- **Price Action Focus**: Uses candle highs/lows rather than closing prices for swing detection

#### Setup Identification
- **Exhaustion Setup**: 
  - Detects when CVD conviction weakens significantly
  - Current candle's absolute CVD < previous candle's absolute CVD * exhaustion_ratio (default: 0.70)
  - Indicates fading momentum despite price breakout

- **Absorption Setup**:
  - Detects when price breaks to new extreme but range contracts
  - Current candle range < previous candle range AND
  - Current candle's CVD is in top percentile of historical CVD (default: 90th percentile)
  - Indicates liquidity absorption at price extremes

#### Signal Generation
- **determine_direction**: Combines swing detection with VWAP and POC analysis
  - LONG: New swing low + close above midpoint + POC in lower half + close below VWAP
  - SHORT: New swing high + close below midpoint + POC in upper half + close above VWAP
  - Requires VWAP confirmation to avoid counter-trend entries

#### Order Parameter Calculation
- **calculate_entry_price**: POC ± offset percentage (default: 0.1% from POC)
- **calculate_stop_loss**: Candle wick ± offset in ticks (default: 2 ticks)
- **calculate_take_profit**: Entry ± (stop loss distance * risk_r_multiple) (default: 1.5R)
- **calculate_position_size**: max_position_usd / entry_price

### CVDPoC Strategy (cvd_poc.rs)
Main strategy orchestrator that combines all components:

#### CvdPocStrategy Struct
- **Components**:
  - SignalEvaluator: Core signal generation logic
  - GlobalState: Access to position/order/candle data
  - VolumeProfileBuilders: Per-symbol POC calculation for candles (HashMap<String, VolumeProfileBuilder>)
  - IndicatorCompute: CVD/RVOL/historical indicators
  - max_position_usd: Position sizing limit
  - tick_sizes: Per-symbol tick sizes from exchange metadata (HashMap<String, f64>)

#### Main Processing Flow
1. **process_candle**:
   - Calculate POC for incoming candle using VolumeProfileBuilder
   - Update indicators with candle data
   - Check current position status for symbol
   - Evaluate signal using SignalEvaluator (if no existing position)
   - Validate signal with risk management (not shown in this module - handled in bot)
   - Return validated TradeSignal

2. **check_exit**:
   - Retrieve current position for symbol
   - Get most recent candle
   - Evaluate exit conditions using SignalEvaluator.check_exit()
   - Stop loss: Price hits position's stop loss
   - Take profit: Price hits position's take profit  
   - CVD flip streak: Position.flip_streak >= 2 triggers exit

3. **manage_position_exit**:
   - Implements CVD-based trailing stop logic
   - **Rule 1**: CVD declining but still favorable - tighten SL to previous candle's POC
   - **Rule 2**: Single CVD flip - tighten SL to current candle's POC, increment flip streak
   - **Rule 3**: Two consecutive CVD flips - exit position (CvdFlip reason)
   - **Rule 4**: CVD returns to favorable - reset flip streak

#### Helper Methods
- get_vwap: Placeholder for VWAP integration (currently returns None)
- get_indicators/get_volume_profile: Read-only access to internal components
- clear: Resets volume profile and indicators

## The "Hurdles"
### Known Limitations
1. **History Simplification**: SignalEvaluator uses empty history vector (TODO comments) - relies on external state management
2. **VWAP Integration**: Marked as TODO - currently returns None, reducing signal accuracy
3. **Tick Size Assumption**: Stop loss calculation uses hardcoded tick size of 1.0 rather than config-derived value
4. **Position Sizing**: Uses hardcoded max_position_usd of 1000.0 rather than config value
5. **Lookback Dependency**: Swing detection requires sufficient history - signals delayed until lookback period satisfied

### Technical Debt
1. **Incomplete TODOs**: Multiple TODO comments indicating incomplete integrations
2. **Magic Numbers**: Some hardcoded values that should be configurable (tick size now fetched from exchange, position sizing from config)
3. **Error Handling**: Limited error propagation - assumes operations succeed
4. **Code Duplication**: Some logic duplicated between evaluate_signal and manage_position_exit
5. **State Coupling**: Direct access to GlobalState creates tight coupling between strategy and state layers

### Performance Considerations
1. **Volume Profile Recreation**: VolumeProfileBuilder clears and rebuilds profile for each candle - O(n) operation where n = trades per candle
2. **Indicator Updates**: IndicatorCompute updates history vectors - could cause allocations if not pre-sized
3. **Clone Operations**: TradeSignal creation involves String cloning for symbol
4. **Allocation Hotspots**: Frequent Vec allocations in signal evaluation paths

## Future Roadmap
### Immediate Improvements
1. Implement proper history passing from GlobalState to SignalEvaluator
2. Integrate actual VWAP tracker for signal validation
3. Hardcoded tick size replaced with exchange metadata; position sizing from config
4. Remove TODO comments and complete incomplete integrations
5. Add more specific error types for strategy validation failures

### Medium-term Enhancements
1. Optimize VolumeProfileBuilder to maintain rolling window rather than rebuild per candle
2. Add configurable lookback periods for different timeframes/market conditions
3. Implement signal strength scoring rather than binary pass/fail
4. Add multi-timeframe analysis capabilities
5. Optimize memory allocations with object pooling where beneficial

### Long-term Goals
1. Pluggable strategy architecture (easy to add/test alternative strategies)
2. Machine learning signal enhancement layer
3. Real-time strategy performance analytics
4. Genetic algorithm parameter optimization
5. Strategy combination/ensemble capabilities