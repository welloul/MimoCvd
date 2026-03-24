use chrono::{DateTime, DurationRound, Utc};
use cvdtrader_core::{Candle, GlobalState, Trade};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::VolumeProfileBuilder;

/// Candle builder that aggregates trades into 1-minute candles
pub struct CandleBuilder {
    /// Current candles being built (symbol -> candle)
    current_candles: HashMap<String, Candle>,
    /// Channel to send completed candles
    candle_tx: mpsc::Sender<Candle>,
    /// Global state for CVD tracking
    state: GlobalState,
    /// Volume profile builders per symbol for POC calculation
    volume_profiles: HashMap<String, VolumeProfileBuilder>,
}

impl CandleBuilder {
    /// Create a new candle builder
    pub fn new(candle_tx: mpsc::Sender<Candle>, state: GlobalState) -> Self {
        Self {
            current_candles: HashMap::new(),
            candle_tx,
            state,
            volume_profiles: HashMap::new(),
        }
    }

    /// Set tick sizes for symbols (enables POC calculation)
    pub fn set_tick_sizes(&mut self, tick_sizes: &HashMap<String, f64>) {
        for (symbol, tick_size) in tick_sizes {
            self.volume_profiles
                .insert(symbol.clone(), VolumeProfileBuilder::new(*tick_size));
        }
    }

    /// Process a trade and potentially emit a completed candle
    pub async fn process_trade(&mut self, trade: &Trade) -> Option<Candle> {
        let symbol = trade.symbol.clone();
        let candle_timestamp = trade
            .timestamp
            .duration_trunc(chrono::Duration::minutes(1))
            .expect("Failed to truncate timestamp");

        // Check if we need to start a new candle
        let needs_new_candle = if let Some(current) = self.current_candles.get(&symbol) {
            current.timestamp != candle_timestamp
        } else {
            true
        };

        // If we need a new candle, finalize the old one and emit it
        if needs_new_candle {
            let completed_candle = if let Some(old_candle) = self.current_candles.remove(&symbol) {
                let completed = self.finalize_candle(old_candle).await;
                debug!(
                    "Completed candle for {}: O={} H={} L={} C={} V={} CVD={}",
                    symbol,
                    completed.open,
                    completed.high,
                    completed.low,
                    completed.close,
                    completed.volume,
                    completed.cvd
                );

                // Send completed candle
                if let Err(e) = self.candle_tx.send(completed.clone()).await {
                    tracing::error!("Failed to send completed candle: {}", e);
                }

                // Store in global state
                self.state
                    .add_candle(symbol.clone(), completed.clone())
                    .await;

                Some(completed)
            } else {
                None
            };

            // Create new candle
            let new_candle = Candle::new(symbol.clone(), candle_timestamp);
            self.current_candles.insert(symbol.clone(), new_candle);
            info!("Started new candle for {} at {}", symbol, candle_timestamp);

            // If we completed a candle, return it (but continue processing the current trade)
            if let Some(completed) = completed_candle {
                // Add current trade to the new candle before returning
                if let Some(candle) = self.current_candles.get_mut(&symbol) {
                    candle.add_trade(trade);
                    self.state
                        .update_global_cvd(symbol.clone(), trade.delta())
                        .await;
                }
                return Some(completed);
            }
        }

        // Add trade to current candle
        if let Some(candle) = self.current_candles.get_mut(&symbol) {
            candle.add_trade(trade);

            // Update global CVD
            self.state
                .update_global_cvd(symbol.clone(), trade.delta())
                .await;
        }

        None
    }

    /// Finalize a candle (calculate POC)
    async fn finalize_candle(&self, mut candle: Candle) -> Candle {
        // Calculate POC using VolumeProfileBuilder if available
        let poc = if let Some(vp) = self.volume_profiles.get(&candle.symbol) {
            // Create a temporary VP to calculate POC for this candle
            let tick_size = vp.get_tick_size();
            let mut temp_vp = VolumeProfileBuilder::new(tick_size);
            temp_vp.process_candle(&candle)
        } else {
            None
        };

        candle.finalize(poc);
        candle
    }

    /// Get current candle for symbol
    pub fn get_current_candle(&self, symbol: &str) -> Option<&Candle> {
        self.current_candles.get(symbol)
    }

    /// Get all current candles
    pub fn get_all_current_candles(&self) -> &HashMap<String, Candle> {
        &self.current_candles
    }

    /// Force finalize all current candles (for shutdown)
    pub async fn finalize_all(&mut self) -> Vec<Candle> {
        let mut completed = Vec::new();

        let candles: Vec<Candle> = self.current_candles.drain().map(|(_, c)| c).collect();

        for candle in candles {
            let finalized = self.finalize_candle(candle).await;
            completed.push(finalized);
        }

        completed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[tokio::test]
    async fn test_candle_builder_single_candle() {
        let (candle_tx, mut candle_rx) = mpsc::channel(100);
        let state = GlobalState::new();
        let mut builder = CandleBuilder::new(candle_tx, state.clone());

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

        // Add trades to same candle
        let trade1 = Trade::new(
            "BTC".to_string(),
            50000.0,
            1.0,
            cvdtrader_core::Side::Buy,
            timestamp,
        );
        let trade2 = Trade::new(
            "BTC".to_string(),
            50100.0,
            0.5,
            cvdtrader_core::Side::Sell,
            timestamp,
        );

        assert!(builder.process_trade(&trade1).await.is_none());
        assert!(builder.process_trade(&trade2).await.is_none());

        // Check current candle
        let current = builder.get_current_candle("BTC").unwrap();
        assert_eq!(current.open, 50000.0);
        assert_eq!(current.high, 50100.0);
        assert_eq!(current.low, 50000.0);
        assert_eq!(current.close, 50100.0);
        assert_eq!(current.volume, 1.5);
        assert_eq!(current.cvd, 0.5); // 1.0 - 0.5

        // Check global CVD
        assert_eq!(state.get_global_cvd("BTC").await, 0.5);
    }

    #[tokio::test]
    async fn test_candle_builder_multiple_candles() {
        let (candle_tx, mut candle_rx) = mpsc::channel(100);
        let state = GlobalState::new();
        let mut builder = CandleBuilder::new(candle_tx, state);

        let timestamp1 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let timestamp2 = Utc.with_ymd_and_hms(2024, 1, 1, 0, 1, 0).unwrap();

        // First candle
        let trade1 = Trade::new(
            "BTC".to_string(),
            50000.0,
            1.0,
            cvdtrader_core::Side::Buy,
            timestamp1,
        );
        assert!(builder.process_trade(&trade1).await.is_none());

        // Second candle (should emit first candle)
        let trade2 = Trade::new(
            "BTC".to_string(),
            50100.0,
            0.5,
            cvdtrader_core::Side::Sell,
            timestamp2,
        );
        let completed = builder.process_trade(&trade2).await;
        assert!(completed.is_some());

        let completed_candle = completed.unwrap();
        assert_eq!(completed_candle.open, 50000.0);
        assert_eq!(completed_candle.close, 50000.0);
        assert_eq!(completed_candle.volume, 1.0);
        assert_eq!(completed_candle.cvd, 1.0);

        // Check new candle
        let current = builder.get_current_candle("BTC").unwrap();
        assert_eq!(current.open, 50100.0);
        assert_eq!(current.volume, 0.5);
        assert_eq!(current.cvd, -0.5);
    }

    #[tokio::test]
    async fn test_candle_builder_finalize_all() {
        let (candle_tx, _candle_rx) = mpsc::channel(100);
        let state = GlobalState::new();
        let mut builder = CandleBuilder::new(candle_tx, state);

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

        // Add trades
        let trade1 = Trade::new(
            "BTC".to_string(),
            50000.0,
            1.0,
            cvdtrader_core::Side::Buy,
            timestamp,
        );
        let trade2 = Trade::new(
            "ETH".to_string(),
            3000.0,
            10.0,
            cvdtrader_core::Side::Sell,
            timestamp,
        );

        builder.process_trade(&trade1).await;
        builder.process_trade(&trade2).await;

        // Finalize all
        let completed = builder.finalize_all().await;
        assert_eq!(completed.len(), 2);
        assert!(builder.get_current_candle("BTC").is_none());
        assert!(builder.get_current_candle("ETH").is_none());
    }
}
