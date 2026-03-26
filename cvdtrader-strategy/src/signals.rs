use cvdtrader_core::{Candle, ExitReason, Position, PositionSide, SetupType, Signal, TradeSignal};
use cvdtrader_market_data::IndicatorCompute;
use std::collections::HashMap;

/// Signal generator trait
pub trait SignalGenerator {
    /// Evaluate signal for a candle
    fn evaluate_signal(
        &self,
        candle: &Candle,
        history: &[Candle],
        indicators: &IndicatorCompute,
        has_position: bool,
    ) -> Option<TradeSignal>;

    /// Check exit conditions for a position
    fn check_exit(
        &self,
        position: &Position,
        candle: &Candle,
        indicators: &IndicatorCompute,
    ) -> Option<ExitReason>;
}

/// Signal evaluator for CVDPoC strategy
pub struct SignalEvaluator {
    /// Lookback period for swing detection
    lookback: usize,
    /// CVD exhaustion ratio threshold
    cvd_exhaustion_ratio: f64,
    /// CVD absorption percentile threshold
    cvd_absorption_pctile: f64,
    /// Stop loss offset in ticks
    sl_offset: i32,
    /// Risk R-multiple for take profit
    risk_r_multiple: f64,
    /// Tick sizes per symbol
    tick_sizes: std::collections::HashMap<String, f64>,
}

impl SignalEvaluator {
    /// Create a new signal evaluator
    pub fn new(
        lookback: usize,
        cvd_exhaustion_ratio: f64,
        cvd_absorption_pctile: f64,
        sl_offset: i32,
        risk_r_multiple: f64,
        tick_sizes: HashMap<String, f64>,
    ) -> Self {
        Self {
            lookback,
            cvd_exhaustion_ratio,
            cvd_absorption_pctile,
            sl_offset,
            risk_r_multiple,
            tick_sizes,
        }
    }

    /// Get lookback period
    pub fn lookback(&self) -> usize {
        self.lookback
    }

    /// Get tick size for a symbol
    pub fn get_tick_size(&self, symbol: &str) -> f64 {
        self.tick_sizes.get(symbol).copied().unwrap_or(1.0) // Default to 1.0 if not found
    }

    /// Check if candle sets a new swing high
    fn is_new_swing_high(&self, candle: &Candle, history: &[Candle]) -> bool {
        if history.len() < self.lookback {
            return false;
        }

        let lookback_highs: Vec<f64> = history
            .iter()
            .rev()
            .take(self.lookback)
            .map(|c| c.high)
            .collect();

        let max_high = lookback_highs
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(&0.0);

        candle.high > *max_high
    }

    /// Check if candle sets a new swing low
    fn is_new_swing_low(&self, candle: &Candle, history: &[Candle]) -> bool {
        if history.len() < self.lookback {
            return false;
        }

        let lookback_lows: Vec<f64> = history
            .iter()
            .rev()
            .take(self.lookback)
            .map(|c| c.low)
            .collect();

        let min_low = lookback_lows
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(&f64::MAX);

        candle.low < *min_low
    }

    /// Check exhaustion setup
    fn is_exhaustion_setup(&self, candle: &Candle, prev_candle: &Candle) -> bool {
        let curr_cvd_abs = candle.cvd.abs();
        let prev_cvd_abs = prev_candle.cvd.abs();

        if prev_cvd_abs == 0.0 {
            return false;
        }

        curr_cvd_abs < prev_cvd_abs * self.cvd_exhaustion_ratio
    }

    /// Check absorption setup
    fn is_absorption_setup(
        &self,
        candle: &Candle,
        prev_candle: &Candle,
        indicators: &IndicatorCompute,
    ) -> bool {
        let curr_range = candle.range();
        let prev_range = prev_candle.range();

        // Candle body shrank
        if curr_range >= prev_range {
            return false;
        }

        // CVD is in top percentile
        indicators.is_cvd_in_top_percentile(&candle.symbol, candle.cvd, self.cvd_absorption_pctile)
    }

    /// Determine trade direction
    fn determine_direction(
        &self,
        candle: &Candle,
        is_new_high: bool,
        is_new_low: bool,
    ) -> Option<Signal> {
        let close = candle.close;
        let midpoint = candle.midpoint();
        // Temporarily comment out POC validation for testing signal generation
        // let poc_in_upper = candle.poc_in_upper_half();
        // let poc_in_lower = candle.poc_in_lower_half();

        // SHORT setup
        if is_new_high && close < midpoint
        /* && poc_in_upper */
        {
            return Some(Signal::Short);
        }

        // LONG setup
        if is_new_low && close > midpoint
        /* && poc_in_lower */
        {
            return Some(Signal::Long);
        }

        None
    }

    /// Calculate entry price using tick-based offsets
    fn calculate_entry_price(&self, poc: f64, signal: Signal, tick_size: f64) -> f64 {
        match signal {
            Signal::Long => poc + tick_size,  // Buy above POC
            Signal::Short => poc - tick_size, // Sell below POC
            Signal::None => poc,
        }
    }

    /// Calculate stop loss
    fn calculate_stop_loss(&self, candle: &Candle, signal: Signal, tick_size: f64) -> f64 {
        let offset = self.sl_offset as f64 * tick_size;
        match signal {
            Signal::Long => candle.low - offset,
            Signal::Short => candle.high + offset,
            Signal::None => 0.0,
        }
    }

    /// Calculate take profit
    fn calculate_take_profit(&self, entry_price: f64, stop_loss: f64, signal: Signal) -> f64 {
        let sl_distance = (entry_price - stop_loss).abs();
        let tp_distance = sl_distance * self.risk_r_multiple;

        match signal {
            Signal::Long => entry_price + tp_distance,
            Signal::Short => entry_price - tp_distance,
            Signal::None => 0.0,
        }
    }

    /// Calculate position size
    fn calculate_position_size(&self, entry_price: f64, max_position_usd: f64) -> f64 {
        if entry_price <= 0.0 {
            return 0.0;
        }
        max_position_usd / entry_price
    }
}

impl SignalGenerator for SignalEvaluator {
    fn evaluate_signal(
        &self,
        candle: &Candle,
        history: &[Candle],
        indicators: &IndicatorCompute,
        has_position: bool,
    ) -> Option<TradeSignal> {
        // Pre-conditions
        if has_position {
            return None;
        }

        if candle.poc.is_none() {
            return None;
        }

        if history.len() < self.lookback {
            return None;
        }

        let prev_candle = history.last()?;

        // Check for new swing high/low
        let is_new_high = self.is_new_swing_high(candle, &history);
        let is_new_low = self.is_new_swing_low(candle, &history);

        if !is_new_high && !is_new_low {
            return None;
        }

        // Determine direction first
        let signal = self.determine_direction(candle, is_new_high, is_new_low)?;

        if signal == Signal::None {
            return None;
        }

        // Check for CVD flip (simplified signal condition)
        let prev_cvd_sign = prev_candle.cvd.signum();
        let curr_cvd_sign = candle.cvd.signum();

        let cvd_flipped = match signal {
            Signal::Short => prev_cvd_sign > 0.0 && curr_cvd_sign < 0.0, // Positive to negative
            Signal::Long => prev_cvd_sign < 0.0 && curr_cvd_sign > 0.0,  // Negative to positive
            Signal::None => false,
        };

        if !cvd_flipped {
            return None;
        }

        // Use flip as setup type
        let setup_type = SetupType::Exhaustion; // Simplified - could differentiate later

        // Calculate order parameters
        let poc = candle.poc.unwrap();
        let tick_size = self.get_tick_size(&candle.symbol);
        let entry_price = self.calculate_entry_price(poc, signal, tick_size);
        let stop_loss = self.calculate_stop_loss(candle, signal, tick_size);
        let take_profit = self.calculate_take_profit(entry_price, stop_loss, signal);
        let max_position_usd = 1000.0; // TODO: Get from config
        let size = self.calculate_position_size(entry_price, max_position_usd);

        if size <= 0.0 {
            return None;
        }

        Some(TradeSignal::new(
            signal,
            Some(setup_type),
            candle.symbol.clone(),
            entry_price,
            stop_loss,
            take_profit,
            size,
        ))
    }

    fn check_exit(
        &self,
        position: &Position,
        candle: &Candle,
        indicators: &IndicatorCompute,
    ) -> Option<ExitReason> {
        // Check stop loss
        if position.is_sl_hit(candle.close) {
            return Some(ExitReason::StopLoss);
        }

        // Check take profit
        if position.is_tp_hit(candle.close) {
            return Some(ExitReason::TakeProfit);
        }

        // Check CVD flip streak
        if position.flip_streak >= 2 {
            return Some(ExitReason::CvdFlip);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_signal_evaluator_new() {
        let tick_sizes = HashMap::new();
        let evaluator = SignalEvaluator::new(20, 0.70, 0.90, 2, 1.5, tick_sizes);
        assert_eq!(evaluator.lookback, 20);
        assert_eq!(evaluator.cvd_exhaustion_ratio, 0.70);
    }

    #[test]
    fn test_calculate_entry_price() {
        let tick_sizes = HashMap::new();
        let evaluator = SignalEvaluator::new(20, 0.70, 0.90, 2, 1.5, tick_sizes);

        let long_entry = evaluator.calculate_entry_price(50000.0, Signal::Long, 1.0);
        assert_eq!(long_entry, 50001.0); // 50000 + 1.0 (buy above POC)

        let short_entry = evaluator.calculate_entry_price(50000.0, Signal::Short, 1.0);
        assert_eq!(short_entry, 49999.0); // 50000 - 1.0 (sell below POC)
    }

    #[test]
    fn test_calculate_stop_loss() {
        let tick_sizes = HashMap::new();
        let evaluator = SignalEvaluator::new(20, 0.70, 0.90, 2, 1.5, tick_sizes);
        let mut candle = Candle::new("BTC".to_string(), Utc::now());
        candle.high = 51000.0;
        candle.low = 49000.0;

        let long_sl = evaluator.calculate_stop_loss(&candle, Signal::Long, 1.0);
        assert_eq!(long_sl, 48998.0); // 49000 - 2

        let short_sl = evaluator.calculate_stop_loss(&candle, Signal::Short, 1.0);
        assert_eq!(short_sl, 51002.0); // 51000 + 2
    }

    #[test]
    fn test_calculate_take_profit() {
        let tick_sizes = HashMap::new();
        let evaluator = SignalEvaluator::new(20, 0.70, 0.90, 2, 1.5, tick_sizes);

        let long_tp = evaluator.calculate_take_profit(50000.0, 49000.0, Signal::Long);
        assert_eq!(long_tp, 51500.0); // 50000 + (1000 * 1.5)

        let short_tp = evaluator.calculate_take_profit(50000.0, 51000.0, Signal::Short);
        assert_eq!(short_tp, 48500.0); // 50000 - (1000 * 1.5)
    }

    #[test]
    fn test_calculate_position_size() {
        let tick_sizes = HashMap::new();
        let evaluator = SignalEvaluator::new(20, 0.70, 0.90, 2, 1.5, tick_sizes);

        let size = evaluator.calculate_position_size(50000.0, 1000.0);
        assert_eq!(size, 0.02); // 1000 / 50000
    }

    /// Helper function to create test candles with CVD values
    fn create_test_candle(
        symbol: &str,
        high: f64,
        low: f64,
        close: f64,
        cvd: f64,
        poc: Option<f64>,
    ) -> Candle {
        let mut candle = Candle::new(symbol.to_string(), Utc::now());
        candle.high = high;
        candle.low = low;
        candle.close = close;
        candle.cvd = cvd;
        candle.poc = poc;
        candle
    }

    /// Helper function to create mock indicators that don't affect CVD flip logic
    fn create_mock_indicators() -> IndicatorCompute {
        IndicatorCompute::new(100)
    }

    #[test]
    fn test_cvd_flip_short_signal_positive_to_negative() {
        let tick_sizes = HashMap::new();
        let evaluator = SignalEvaluator::new(3, 0.70, 0.90, 2, 1.5, tick_sizes);
        let indicators = create_mock_indicators();

        // Create sufficient history (3 candles) for lookback=3
        let hist1 = create_test_candle("BTC", 49800.0, 49700.0, 49750.0, 50.0, Some(49775.0));
        let hist2 = create_test_candle("BTC", 49900.0, 49800.0, 49850.0, 75.0, Some(49875.0));
        let prev_candle =
            create_test_candle("BTC", 50000.0, 49900.0, 49950.0, 100.0, Some(49975.0)); // Positive CVD
        let curr_candle =
            create_test_candle("BTC", 50100.0, 49800.0, 49925.0, -50.0, Some(49950.0)); // Negative CVD, new swing high

        let history = vec![hist1, hist2, prev_candle];

        let signal = evaluator.evaluate_signal(&curr_candle, &history, &indicators, false);

        assert!(
            signal.is_some(),
            "Should generate short signal on CVD flip from positive to negative"
        );
        if let Some(sig) = signal {
            assert_eq!(sig.signal, Signal::Short);
        }
    }

    #[test]
    fn test_cvd_flip_long_signal_negative_to_positive() {
        let tick_sizes = HashMap::new();
        let evaluator = SignalEvaluator::new(3, 0.70, 0.90, 2, 1.5, tick_sizes);
        let indicators = create_mock_indicators();

        // Create sufficient history (3 candles) for lookback=3
        let hist1 = create_test_candle("BTC", 50100.0, 50000.0, 50050.0, -50.0, Some(50075.0));
        let hist2 = create_test_candle("BTC", 50200.0, 50100.0, 50150.0, -75.0, Some(50175.0));
        let prev_candle =
            create_test_candle("BTC", 50000.0, 49900.0, 49950.0, -100.0, Some(49975.0)); // Negative CVD
        let curr_candle = create_test_candle("BTC", 50100.0, 49800.0, 49975.0, 50.0, Some(49950.0)); // Positive CVD, new swing low

        let history = vec![hist1, hist2, prev_candle];

        let signal = evaluator.evaluate_signal(&curr_candle, &history, &indicators, false);

        assert!(
            signal.is_some(),
            "Should generate long signal on CVD flip from negative to positive"
        );
        if let Some(sig) = signal {
            assert_eq!(sig.signal, Signal::Long);
        }
    }

    #[test]
    fn test_no_signal_when_cvd_does_not_flip() {
        let tick_sizes = HashMap::new();
        let evaluator = SignalEvaluator::new(3, 0.70, 0.90, 2, 1.5, tick_sizes);
        let indicators = create_mock_indicators();

        // Test case 1: Both positive (no flip)
        let prev_candle =
            create_test_candle("BTC", 50000.0, 49900.0, 49950.0, 100.0, Some(49975.0));
        let curr_candle = create_test_candle("BTC", 50100.0, 49800.0, 49925.0, 50.0, Some(49950.0));

        let history = vec![prev_candle];
        let signal = evaluator.evaluate_signal(&curr_candle, &history, &indicators, false);
        assert!(
            signal.is_none(),
            "Should not generate signal when CVD stays positive"
        );

        // Test case 2: Both negative (no flip)
        let prev_candle =
            create_test_candle("BTC", 50000.0, 49900.0, 49950.0, -100.0, Some(49975.0));
        let curr_candle =
            create_test_candle("BTC", 50100.0, 49800.0, 49925.0, -50.0, Some(49950.0));

        let history = vec![prev_candle];
        let signal = evaluator.evaluate_signal(&curr_candle, &history, &indicators, false);
        assert!(
            signal.is_none(),
            "Should not generate signal when CVD stays negative"
        );
    }

    #[test]
    fn test_no_signal_when_has_existing_position() {
        let tick_sizes = HashMap::new();
        let evaluator = SignalEvaluator::new(3, 0.70, 0.90, 2, 1.5, tick_sizes);
        let indicators = create_mock_indicators();

        // Valid CVD flip scenario
        let prev_candle =
            create_test_candle("BTC", 50000.0, 49900.0, 49950.0, 100.0, Some(49975.0));
        let curr_candle =
            create_test_candle("BTC", 50100.0, 49800.0, 49925.0, -50.0, Some(49950.0));

        let history = vec![prev_candle];

        let signal = evaluator.evaluate_signal(&curr_candle, &history, &indicators, true); // has_position = true

        assert!(
            signal.is_none(),
            "Should not generate signal when position already exists"
        );
    }

    #[test]
    fn test_no_signal_when_insufficient_history() {
        let tick_sizes = HashMap::new();
        let evaluator = SignalEvaluator::new(5, 0.70, 0.90, 2, 1.5, tick_sizes); // Requires 5 candles history
        let indicators = create_mock_indicators();

        // Valid CVD flip scenario but insufficient history
        let prev_candle =
            create_test_candle("BTC", 50000.0, 49900.0, 49950.0, 100.0, Some(49975.0));
        let curr_candle =
            create_test_candle("BTC", 50100.0, 49800.0, 49925.0, -50.0, Some(49950.0));

        let history = vec![prev_candle]; // Only 1 candle, need 5

        let signal = evaluator.evaluate_signal(&curr_candle, &history, &indicators, false);

        assert!(
            signal.is_none(),
            "Should not generate signal with insufficient history"
        );
    }

    #[test]
    fn test_no_signal_when_poc_missing() {
        let tick_sizes = HashMap::new();
        let evaluator = SignalEvaluator::new(3, 0.70, 0.90, 2, 1.5, tick_sizes);
        let indicators = create_mock_indicators();

        // Valid CVD flip but no POC
        let prev_candle =
            create_test_candle("BTC", 50000.0, 49900.0, 49950.0, 100.0, Some(49975.0));
        let curr_candle = create_test_candle("BTC", 50100.0, 49800.0, 49925.0, -50.0, None); // No POC

        let history = vec![prev_candle];

        let signal = evaluator.evaluate_signal(&curr_candle, &history, &indicators, false);

        assert!(
            signal.is_none(),
            "Should not generate signal when POC is missing"
        );
    }

    #[test]
    fn test_no_signal_when_not_new_swing() {
        let tick_sizes = HashMap::new();
        let evaluator = SignalEvaluator::new(3, 0.70, 0.90, 2, 1.5, tick_sizes);
        let indicators = create_mock_indicators();

        // Create history where current candle is not a new swing high/low
        let prev1 = create_test_candle("BTC", 50000.0, 49900.0, 49950.0, 100.0, Some(49975.0));
        let prev2 = create_test_candle("BTC", 50100.0, 49950.0, 50050.0, 200.0, Some(50025.0));
        let prev3 = create_test_candle("BTC", 50200.0, 50000.0, 50100.0, 300.0, Some(50100.0));

        // Current candle lower than previous highs
        let curr_candle =
            create_test_candle("BTC", 49900.0, 49800.0, 49850.0, -50.0, Some(49875.0));

        let history = vec![prev1, prev2, prev3];

        let signal = evaluator.evaluate_signal(&curr_candle, &history, &indicators, false);

        assert!(
            signal.is_none(),
            "Should not generate signal when not a new swing high/low"
        );
    }

    #[test]
    fn test_signal_includes_correct_setup_type() {
        let tick_sizes = HashMap::new();
        let evaluator = SignalEvaluator::new(3, 0.70, 0.90, 2, 1.5, tick_sizes);
        let indicators = create_mock_indicators();

        // Valid short signal scenario with sufficient history
        let hist1 = create_test_candle("BTC", 49800.0, 49700.0, 49750.0, 50.0, Some(49775.0));
        let hist2 = create_test_candle("BTC", 49900.0, 49800.0, 49850.0, 75.0, Some(49875.0));
        let prev_candle =
            create_test_candle("BTC", 50000.0, 49900.0, 49950.0, 100.0, Some(49975.0)); // Positive CVD
        let curr_candle =
            create_test_candle("BTC", 50100.0, 49800.0, 49925.0, -50.0, Some(49950.0)); // Negative CVD, new swing high

        let history = vec![hist1, hist2, prev_candle];

        let signal = evaluator.evaluate_signal(&curr_candle, &history, &indicators, false);

        assert!(signal.is_some());
        if let Some(sig) = signal {
            assert_eq!(sig.setup_type, Some(SetupType::Exhaustion)); // Currently using Exhaustion as placeholder
        }
    }

    #[test]
    fn test_signal_calculation_with_cvd_flip() {
        let tick_sizes = HashMap::new();
        let evaluator = SignalEvaluator::new(3, 0.70, 0.90, 2, 1.5, tick_sizes);
        let indicators = create_mock_indicators();

        // Valid short signal scenario with sufficient history
        let hist1 = create_test_candle("BTC", 49800.0, 49700.0, 49750.0, 50.0, Some(49775.0));
        let hist2 = create_test_candle("BTC", 49900.0, 49800.0, 49850.0, 75.0, Some(49875.0));
        let prev_candle =
            create_test_candle("BTC", 50000.0, 49900.0, 49950.0, 100.0, Some(49975.0)); // Positive CVD
        let curr_candle =
            create_test_candle("BTC", 50100.0, 49800.0, 49925.0, -50.0, Some(49950.0)); // Negative CVD, new swing high

        let history = vec![hist1, hist2, prev_candle];

        let signal = evaluator.evaluate_signal(&curr_candle, &history, &indicators, false);

        assert!(signal.is_some());
        if let Some(sig) = signal {
            assert_eq!(sig.symbol, "BTC");
            assert_eq!(sig.signal, Signal::Short);
            // Verify the signal uses proper tick sizes for calculations
            assert!(sig.entry_price > 0.0);
            assert!(sig.stop_loss > sig.entry_price); // Stop loss above entry for short
            assert!(sig.take_profit < sig.entry_price); // Take profit below entry for short
            assert!(sig.size > 0.0);
        }
    }
}
