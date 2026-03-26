use cvdtrader_core::{Candle, ExitReason, Position, PositionSide, SetupType, Signal, TradeSignal};
use cvdtrader_market_data::IndicatorCompute;

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
    /// Entry offset percentage
    entry_offset_pct: f64,
}

impl SignalEvaluator {
    /// Create a new signal evaluator
    pub fn new(
        lookback: usize,
        cvd_exhaustion_ratio: f64,
        cvd_absorption_pctile: f64,
        sl_offset: i32,
        risk_r_multiple: f64,
        entry_offset_pct: f64,
    ) -> Self {
        Self {
            lookback,
            cvd_exhaustion_ratio,
            cvd_absorption_pctile,
            sl_offset,
            risk_r_multiple,
            entry_offset_pct,
        }
    }

    /// Get lookback period
    pub fn lookback(&self) -> usize {
        self.lookback
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

    /// Calculate entry price
    fn calculate_entry_price(&self, poc: f64, signal: Signal) -> f64 {
        match signal {
            Signal::Long => poc * (1.0 - self.entry_offset_pct),
            Signal::Short => poc * (1.0 + self.entry_offset_pct),
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
        let entry_price = self.calculate_entry_price(poc, signal);
        let tick_size = 1.0; // TODO: Get from config
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
        let evaluator = SignalEvaluator::new(20, 0.70, 0.90, 2, 1.5, 0.001);
        assert_eq!(evaluator.lookback, 20);
        assert_eq!(evaluator.cvd_exhaustion_ratio, 0.70);
    }

    #[test]
    fn test_calculate_entry_price() {
        let evaluator = SignalEvaluator::new(20, 0.70, 0.90, 2, 1.5, 0.001);

        let long_entry = evaluator.calculate_entry_price(50000.0, Signal::Long);
        assert!((long_entry - 49950.0).abs() < 0.001); // 50000 * (1 - 0.001)

        let short_entry = evaluator.calculate_entry_price(50000.0, Signal::Short);
        assert!((short_entry - 50050.0).abs() < 0.001); // 50000 * (1 + 0.001)
    }

    #[test]
    fn test_calculate_stop_loss() {
        let evaluator = SignalEvaluator::new(20, 0.70, 0.90, 2, 1.5, 0.001);
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
        let evaluator = SignalEvaluator::new(20, 0.70, 0.90, 2, 1.5, 0.001);

        let long_tp = evaluator.calculate_take_profit(50000.0, 49000.0, Signal::Long);
        assert_eq!(long_tp, 51500.0); // 50000 + (1000 * 1.5)

        let short_tp = evaluator.calculate_take_profit(50000.0, 51000.0, Signal::Short);
        assert_eq!(short_tp, 48500.0); // 50000 - (1000 * 1.5)
    }

    #[test]
    fn test_calculate_position_size() {
        let evaluator = SignalEvaluator::new(20, 0.70, 0.90, 2, 1.5, 0.001);

        let size = evaluator.calculate_position_size(50000.0, 1000.0);
        assert_eq!(size, 0.02); // 1000 / 50000
    }
}
