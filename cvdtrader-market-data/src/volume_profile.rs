use cvdtrader_core::{Candle, Trade};
use std::collections::HashMap;

/// Volume profile builder for calculating Point of Control (POC)
pub struct VolumeProfileBuilder {
    /// Tick size for price binning
    tick_size: f64,
    /// Volume at each price level (symbol -> price -> volume)
    profiles: HashMap<String, HashMap<i64, f64>>,
}

impl VolumeProfileBuilder {
    /// Create a new volume profile builder
    pub fn new(tick_size: f64) -> Self {
        Self {
            tick_size,
            profiles: HashMap::new(),
        }
    }

    /// Get tick size
    pub fn get_tick_size(&self) -> f64 {
        self.tick_size
    }

    /// Add a trade to the volume profile
    pub fn add_trade(&mut self, trade: &Trade) {
        let binned_price = self.bin_price(trade.price);
        let profile = self
            .profiles
            .entry(trade.symbol.clone())
            .or_insert_with(HashMap::new);
        let entry = profile.entry(binned_price).or_insert(0.0);
        *entry += trade.size;
    }

    /// Calculate POC for a symbol
    pub fn calculate_poc(&self, symbol: &str) -> Option<f64> {
        let profile = self.profiles.get(symbol)?;

        if profile.is_empty() {
            return None;
        }

        // Find price level with maximum volume
        let (poc_bin, _) = profile
            .iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))?;

        // Convert bin back to price
        Some(*poc_bin as f64 * self.tick_size)
    }

    /// Get volume profile for a symbol
    pub fn get_profile(&self, symbol: &str) -> Option<&HashMap<i64, f64>> {
        self.profiles.get(symbol)
    }

    /// Get total volume for a symbol
    pub fn get_total_volume(&self, symbol: &str) -> f64 {
        self.profiles
            .get(symbol)
            .map(|profile| profile.values().sum())
            .unwrap_or(0.0)
    }

    /// Clear profile for a symbol
    pub fn clear_symbol(&mut self, symbol: &str) {
        self.profiles.remove(symbol);
    }

    /// Clear all profiles
    pub fn clear_all(&mut self) {
        self.profiles.clear();
    }

    /// Bin a price to the nearest tick
    fn bin_price(&self, price: f64) -> i64 {
        (price / self.tick_size).round() as i64
    }

    /// Process a candle and calculate its POC
    pub fn process_candle(&mut self, candle: &Candle) -> Option<f64> {
        // Clear previous profile for this symbol
        self.clear_symbol(&candle.symbol);

        // Add all trades from the candle
        for trade in &candle.trades {
            self.add_trade(trade);
        }

        // Calculate POC
        self.calculate_poc(&candle.symbol)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use cvdtrader_core::Side;

    #[test]
    fn test_volume_profile_single_trade() {
        let mut builder = VolumeProfileBuilder::new(1.0);
        let trade = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, Utc::now());

        builder.add_trade(&trade);
        let poc = builder.calculate_poc("BTC").unwrap();

        assert_eq!(poc, 50000.0);
    }

    #[test]
    fn test_volume_profile_multiple_trades() {
        let mut builder = VolumeProfileBuilder::new(1.0);

        // Add trades at different prices
        let trade1 = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, Utc::now());
        let trade2 = Trade::new("BTC".to_string(), 50001.0, 2.0, Side::Buy, Utc::now());
        let trade3 = Trade::new("BTC".to_string(), 50002.0, 0.5, Side::Sell, Utc::now());

        builder.add_trade(&trade1);
        builder.add_trade(&trade2);
        builder.add_trade(&trade3);

        let poc = builder.calculate_poc("BTC").unwrap();

        // POC should be at 50001.0 (highest volume: 2.0)
        assert_eq!(poc, 50001.0);
    }

    #[test]
    fn test_volume_profile_tick_size() {
        let mut builder = VolumeProfileBuilder::new(10.0);

        // Trades at 50000, 50005, 50010 should bin to 50000, 50010, 50010
        let trade1 = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, Utc::now());
        let trade2 = Trade::new("BTC".to_string(), 50005.0, 1.0, Side::Buy, Utc::now());
        let trade3 = Trade::new("BTC".to_string(), 50010.0, 1.0, Side::Buy, Utc::now());

        builder.add_trade(&trade1);
        builder.add_trade(&trade2);
        builder.add_trade(&trade3);

        let poc = builder.calculate_poc("BTC").unwrap();

        // POC should be at 50010.0 (2 trades binned there)
        assert_eq!(poc, 50010.0);
    }

    #[test]
    fn test_volume_profile_empty() {
        let builder = VolumeProfileBuilder::new(1.0);
        assert!(builder.calculate_poc("BTC").is_none());
    }

    #[test]
    fn test_volume_profile_total_volume() {
        let mut builder = VolumeProfileBuilder::new(1.0);

        let trade1 = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, Utc::now());
        let trade2 = Trade::new("BTC".to_string(), 50001.0, 2.0, Side::Buy, Utc::now());

        builder.add_trade(&trade1);
        builder.add_trade(&trade2);

        assert_eq!(builder.get_total_volume("BTC"), 3.0);
    }

    #[test]
    fn test_volume_profile_clear() {
        let mut builder = VolumeProfileBuilder::new(1.0);

        let trade = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, Utc::now());
        builder.add_trade(&trade);

        assert!(builder.calculate_poc("BTC").is_some());

        builder.clear_symbol("BTC");
        assert!(builder.calculate_poc("BTC").is_none());
    }

    #[test]
    fn test_process_candle() {
        let mut builder = VolumeProfileBuilder::new(1.0);
        let mut candle = Candle::new("BTC".to_string(), Utc::now());

        let trade1 = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, Utc::now());
        let trade2 = Trade::new("BTC".to_string(), 50001.0, 2.0, Side::Buy, Utc::now());

        candle.add_trade(&trade1);
        candle.add_trade(&trade2);

        let poc = builder.process_candle(&candle).unwrap();
        assert_eq!(poc, 50001.0);
    }
}
