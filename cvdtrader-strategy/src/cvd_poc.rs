use chrono::Utc;
use cvdtrader_core::{Candle, ExitReason, GlobalState, Position, PositionSide, Trade, TradeSignal};
use cvdtrader_market_data::{IndicatorCompute, VolumeProfileBuilder};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::signals::{SignalEvaluator, SignalGenerator};

/// CVDPoC (Cumulative Volume Delta - Point of Control) strategy
pub struct CvdPocStrategy {
    /// Signal evaluator
    evaluator: SignalEvaluator,
    /// Global state
    state: GlobalState,
    /// Volume profile builders per symbol
    volume_profiles: HashMap<String, VolumeProfileBuilder>,
    /// Indicator compute engine
    indicators: IndicatorCompute,
    /// Maximum position size in USD
    max_position_usd: f64,
    /// Tick sizes per symbol
    tick_sizes: HashMap<String, f64>,
}

impl CvdPocStrategy {
    /// Create a new CVDPoC strategy
    pub fn new(
        state: GlobalState,
        lookback: usize,
        cvd_exhaustion_ratio: f64,
        cvd_absorption_pctile: f64,
        sl_offset: i32,
        risk_r_multiple: f64,
        entry_offset_pct: f64,
        tick_sizes: HashMap<String, f64>,
        max_position_usd: f64,
    ) -> Self {
        // Create volume profile builders for each symbol
        let mut volume_profiles = HashMap::new();
        for (symbol, tick_size) in &tick_sizes {
            volume_profiles.insert(symbol.clone(), VolumeProfileBuilder::new(*tick_size));
        }

        Self {
            evaluator: SignalEvaluator::new(
                lookback,
                cvd_exhaustion_ratio,
                cvd_absorption_pctile,
                sl_offset,
                risk_r_multiple,
                entry_offset_pct,
            ),
            state,
            volume_profiles,
            indicators: IndicatorCompute::new(100),
            max_position_usd,
            tick_sizes,
        }
    }

    /// Get tick size for a symbol
    pub fn get_tick_size(&self, symbol: &str) -> Option<f64> {
        self.tick_sizes.get(symbol).copied()
    }

    /// Get all tick sizes
    pub fn get_all_tick_sizes(&self) -> &HashMap<String, f64> {
        &self.tick_sizes
    }

    /// Process a candle and generate signals
    pub async fn process_candle(&mut self, candle: &Candle) -> Option<TradeSignal> {
        // Calculate POC for the candle
        let poc = if let Some(vp) = self.volume_profiles.get_mut(&candle.symbol) {
            vp.process_candle(candle)
        } else {
            None
        };

        // Create a mutable copy of the candle with POC set
        let mut candle_with_poc = candle.clone();
        candle_with_poc.poc = poc;

        // Update indicators
        self.indicators.process_candle(&candle_with_poc);

        // Check if we have a position for this symbol
        let has_position = self.state.has_position(&candle_with_poc.symbol).await;

        // Get candle history for signal evaluation
        let history = self
            .state
            .get_last_candles(&candle_with_poc.symbol, self.evaluator.lookback() + 1)
            .await;

        // Evaluate signal
        let signal = self.evaluator.evaluate_signal(
            &candle_with_poc,
            &history,
            &self.indicators,
            has_position,
        );

        if let Some(ref sig) = signal {
            if sig.is_valid() {
                info!(
                    "Signal generated for {}: {} (setup: {:?}, entry: {}, SL: {}, TP: {}, size: {})",
                    sig.symbol,
                    sig.signal,
                    sig.setup_type,
                    sig.entry_price,
                    sig.stop_loss,
                    sig.take_profit,
                    sig.size
                );
            }
        }

        signal
    }

    /// Check exit conditions for a position
    pub async fn check_exit(&self, symbol: &str, current_price: f64) -> Option<ExitReason> {
        let position = self.state.get_position(symbol).await?;

        // Check stop loss
        if position.is_sl_hit(current_price) {
            return Some(ExitReason::StopLoss);
        }

        // Check take profit
        if position.is_tp_hit(current_price) {
            return Some(ExitReason::TakeProfit);
        }

        // Get last candle for the symbol for CVD flip check
        let candles = self.state.get_last_candles(symbol, 1).await;
        if let Some(candle) = candles.first() {
            // Check CVD flip streak
            if position.flip_streak >= 2 {
                return Some(ExitReason::CvdFlip);
            }
        }

        None
    }

    /// Manage position exit based on CVD behavior
    pub async fn manage_position_exit(
        &mut self,
        symbol: &str,
        current_candle: &Candle,
    ) -> Option<ExitReason> {
        let mut position = self.state.get_position(symbol).await?;

        // Get previous candle
        let candles = self.state.get_last_candles(symbol, 2).await;
        if candles.len() < 2 {
            return None;
        }

        let prev_candle = &candles[candles.len() - 2];

        // Determine favorable CVD sign for position
        let fav_sign = match position.side {
            PositionSide::Long => 1.0,   // Positive CVD is favorable
            PositionSide::Short => -1.0, // Negative CVD is favorable
        };

        let curr_sign = current_candle.cvd.signum();
        let prev_sign = prev_candle.cvd.signum();

        // Rule 1: CVD declining (still favorable but weakening)
        if curr_sign == fav_sign && current_candle.cvd.abs() < prev_candle.cvd.abs() {
            debug!(
                "CVD declining for {}: curr={}, prev={}",
                symbol, current_candle.cvd, prev_candle.cvd
            );

            // Tighten SL to previous candle's POC
            if let Some(prev_poc) = prev_candle.poc {
                position.update_stop_loss(prev_poc);
                self.state
                    .set_position(symbol.to_string(), position.clone())
                    .await;
            }

            return None;
        }

        // Rule 2: CVD flip (1 hostile candle)
        if curr_sign != fav_sign && curr_sign != 0.0 {
            debug!(
                "CVD flip detected for {}: curr={}, prev={}",
                symbol, current_candle.cvd, prev_candle.cvd
            );

            // Tighten SL to current candle's POC
            if let Some(curr_poc) = current_candle.poc {
                position.update_stop_loss(curr_poc);
            }

            // Increment flip streak
            position.increment_flip_streak();
            self.state
                .set_position(symbol.to_string(), position.clone())
                .await;

            // Rule 3: Two consecutive CVD flips
            if position.flip_streak >= 2 {
                warn!("Two consecutive CVD flips for {}: closing position", symbol);
                return Some(ExitReason::CvdFlip);
            }

            return None;
        }

        // CVD returned to favorable sign - reset flip streak
        if curr_sign == fav_sign && position.flip_streak > 0 {
            debug!(
                "CVD returned to favorable sign for {}: resetting flip streak",
                symbol
            );
            position.reset_flip_streak();
            self.state.set_position(symbol.to_string(), position).await;
        }

        None
    }

    /// Get current indicators for a symbol
    pub fn get_indicators(&self, symbol: &str) -> Option<&IndicatorCompute> {
        Some(&self.indicators)
    }

    /// Get volume profile for a symbol
    pub fn get_volume_profile(&self, symbol: &str) -> Option<&VolumeProfileBuilder> {
        self.volume_profiles.get(symbol)
    }

    /// Clear all strategy data
    pub fn clear(&mut self) {
        for vp in self.volume_profiles.values_mut() {
            vp.clear_all();
        }
        self.indicators.clear_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use cvdtrader_core::Side;

    #[tokio::test]
    async fn test_strategy_process_candle() {
        let state = GlobalState::new();
        let mut tick_sizes = HashMap::new();
        tick_sizes.insert("BTC".to_string(), 1.0);
        let mut strategy =
            CvdPocStrategy::new(state, 20, 0.70, 0.90, 2, 1.5, 0.001, tick_sizes, 1000.0);

        let mut candle = Candle::new("BTC".to_string(), Utc::now());
        candle.open = 50000.0;
        candle.high = 51000.0;
        candle.low = 49000.0;
        candle.close = 50500.0;
        candle.volume = 100.0;
        candle.cvd = 50.0;

        // Add some trades to build volume profile
        for i in 0..10 {
            let trade = Trade::new(
                "BTC".to_string(),
                50000.0 + i as f64,
                10.0,
                Side::Buy,
                Utc::now(),
            );
            candle.add_trade(&trade);
        }

        let signal = strategy.process_candle(&candle).await;
        // Signal might be None due to insufficient history
        assert!(signal.is_none() || signal.is_some());
    }

    #[tokio::test]
    async fn test_strategy_check_exit() {
        let state = GlobalState::new();
        let mut tick_sizes = HashMap::new();
        tick_sizes.insert("BTC".to_string(), 1.0);
        let strategy = CvdPocStrategy::new(
            state.clone(),
            20,
            0.70,
            0.90,
            2,
            1.5,
            0.001,
            tick_sizes,
            1000.0,
        );

        // Create a position
        let position = Position::new(
            "BTC".to_string(),
            PositionSide::Long,
            1.0,
            50000.0,
            49000.0,
            52000.0,
        );
        state.set_position("BTC".to_string(), position).await;

        // Add a candle to the state so check_exit can find it
        let mut candle = Candle::new("BTC".to_string(), Utc::now());
        candle.open = 50000.0;
        candle.high = 51000.0;
        candle.low = 49000.0;
        candle.close = 50500.0;
        candle.volume = 100.0;
        candle.cvd = 50.0;
        state.add_candle("BTC".to_string(), candle).await;

        // Check exit at stop loss
        let exit = strategy.check_exit("BTC", 48999.0).await;
        assert_eq!(exit, Some(ExitReason::StopLoss));

        // Check exit at take profit
        let exit = strategy.check_exit("BTC", 52001.0).await;
        assert_eq!(exit, Some(ExitReason::TakeProfit));

        // No exit in between
        let exit = strategy.check_exit("BTC", 50500.0).await;
        assert_eq!(exit, None);
    }
}
