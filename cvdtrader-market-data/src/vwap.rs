use chrono::{DateTime, Datelike, TimeZone, Utc};
use cvdtrader_core::Trade;
use std::collections::HashMap;

/// Daily VWAP tracker
pub struct DailyVWAPTracker {
    /// VWAP data per symbol
    data: HashMap<String, VWAPData>,
}

/// VWAP calculation data for a symbol
#[derive(Debug, Clone)]
struct VWAPData {
    /// Cumulative price * volume
    cumulative_pv: f64,
    /// Cumulative volume
    cumulative_volume: f64,
    /// Current VWAP value
    vwap: f64,
    /// Last update date (for reset detection)
    last_date: DateTime<Utc>,
}

impl DailyVWAPTracker {
    /// Create a new VWAP tracker
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Process a trade and update VWAP
    pub fn process_trade(&mut self, trade: &Trade) {
        let trade_date = trade.timestamp.date_naive();
        let symbol_data = self
            .data
            .entry(trade.symbol.clone())
            .or_insert_with(|| VWAPData {
                cumulative_pv: 0.0,
                cumulative_volume: 0.0,
                vwap: 0.0,
                last_date: trade.timestamp,
            });

        // Check if we need to reset (new day)
        let last_date = symbol_data.last_date.date_naive();
        if trade_date != last_date {
            tracing::info!(
                "Resetting VWAP for {} (new day: {} -> {})",
                trade.symbol,
                last_date,
                trade_date
            );
            symbol_data.cumulative_pv = 0.0;
            symbol_data.cumulative_volume = 0.0;
            symbol_data.vwap = 0.0;
        }

        // Update cumulative values
        let pv = trade.price * trade.size;
        symbol_data.cumulative_pv += pv;
        symbol_data.cumulative_volume += trade.size;
        symbol_data.last_date = trade.timestamp;

        // Calculate VWAP
        if symbol_data.cumulative_volume > 0.0 {
            symbol_data.vwap = symbol_data.cumulative_pv / symbol_data.cumulative_volume;
        }
    }

    /// Get current VWAP for a symbol
    pub fn get_vwap(&self, symbol: &str) -> Option<f64> {
        self.data.get(symbol).map(|d| d.vwap)
    }

    /// Get cumulative volume for a symbol
    pub fn get_cumulative_volume(&self, symbol: &str) -> Option<f64> {
        self.data.get(symbol).map(|d| d.cumulative_volume)
    }

    /// Get all VWAP values
    pub fn get_all_vwap(&self) -> HashMap<String, f64> {
        self.data
            .iter()
            .map(|(symbol, data)| (symbol.clone(), data.vwap))
            .collect()
    }

    /// Reset VWAP for a symbol
    pub fn reset_symbol(&mut self, symbol: &str) {
        if let Some(data) = self.data.get_mut(symbol) {
            data.cumulative_pv = 0.0;
            data.cumulative_volume = 0.0;
            data.vwap = 0.0;
        }
    }

    /// Reset all VWAP data
    pub fn reset_all(&mut self) {
        for data in self.data.values_mut() {
            data.cumulative_pv = 0.0;
            data.cumulative_volume = 0.0;
            data.vwap = 0.0;
        }
    }
}

impl Default for DailyVWAPTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use cvdtrader_core::Side;

    #[test]
    fn test_vwap_single_trade() {
        let mut tracker = DailyVWAPTracker::new();
        let trade = Trade::new(
            "BTC".to_string(),
            50000.0,
            1.0,
            Side::Buy,
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
        );

        tracker.process_trade(&trade);
        let vwap = tracker.get_vwap("BTC").unwrap();

        assert_eq!(vwap, 50000.0);
    }

    #[test]
    fn test_vwap_multiple_trades() {
        let mut tracker = DailyVWAPTracker::new();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

        let trade1 = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, timestamp);
        let trade2 = Trade::new("BTC".to_string(), 50100.0, 2.0, Side::Buy, timestamp);

        tracker.process_trade(&trade1);
        tracker.process_trade(&trade2);

        let vwap = tracker.get_vwap("BTC").unwrap();

        // VWAP = (50000*1 + 50100*2) / (1+2) = 150200 / 3 = 50066.67
        assert!((vwap - 50066.666666666664).abs() < 0.01);
    }

    #[test]
    fn test_vwap_daily_reset() {
        let mut tracker = DailyVWAPTracker::new();

        let trade1 = Trade::new(
            "BTC".to_string(),
            50000.0,
            1.0,
            Side::Buy,
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
        );
        let trade2 = Trade::new(
            "BTC".to_string(),
            51000.0,
            1.0,
            Side::Buy,
            Utc.with_ymd_and_hms(2024, 1, 2, 0, 0, 0).unwrap(),
        );

        tracker.process_trade(&trade1);
        assert_eq!(tracker.get_vwap("BTC").unwrap(), 50000.0);

        tracker.process_trade(&trade2);
        // Should reset and use only trade2
        assert_eq!(tracker.get_vwap("BTC").unwrap(), 51000.0);
    }

    #[test]
    fn test_vwap_cumulative_volume() {
        let mut tracker = DailyVWAPTracker::new();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

        let trade1 = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, timestamp);
        let trade2 = Trade::new("BTC".to_string(), 50100.0, 2.0, Side::Buy, timestamp);

        tracker.process_trade(&trade1);
        tracker.process_trade(&trade2);

        assert_eq!(tracker.get_cumulative_volume("BTC").unwrap(), 3.0);
    }

    #[test]
    fn test_vwap_reset_symbol() {
        let mut tracker = DailyVWAPTracker::new();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

        let trade = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, timestamp);
        tracker.process_trade(&trade);

        assert_eq!(tracker.get_vwap("BTC").unwrap(), 50000.0);

        tracker.reset_symbol("BTC");
        assert_eq!(tracker.get_vwap("BTC").unwrap(), 0.0);
    }

    #[test]
    fn test_vwap_multiple_symbols() {
        let mut tracker = DailyVWAPTracker::new();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

        let trade_btc = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, timestamp);
        let trade_eth = Trade::new("ETH".to_string(), 3000.0, 10.0, Side::Buy, timestamp);

        tracker.process_trade(&trade_btc);
        tracker.process_trade(&trade_eth);

        assert_eq!(tracker.get_vwap("BTC").unwrap(), 50000.0);
        assert_eq!(tracker.get_vwap("ETH").unwrap(), 3000.0);
    }
}
