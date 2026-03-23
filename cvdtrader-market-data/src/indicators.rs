use cvdtrader_core::{Candle, Trade};
use std::collections::HashMap;

/// Indicator compute engine for CVD, RVOL, and other metrics
pub struct IndicatorCompute {
    /// Global CVD per symbol
    global_cvd: HashMap<String, f64>,
    /// Historical CVD values for percentile calculations
    cvd_history: HashMap<String, Vec<f64>>,
    /// Historical volume for RVOL calculations
    volume_history: HashMap<String, Vec<f64>>,
    /// Maximum history size
    max_history: usize,
}

impl IndicatorCompute {
    /// Create a new indicator compute engine
    pub fn new(max_history: usize) -> Self {
        Self {
            global_cvd: HashMap::new(),
            cvd_history: HashMap::new(),
            volume_history: HashMap::new(),
            max_history,
        }
    }

    /// Process a trade and update indicators
    pub fn process_trade(&mut self, trade: &Trade) {
        // Update global CVD
        let entry = self.global_cvd.entry(trade.symbol.clone()).or_insert(0.0);
        *entry += trade.delta();
    }

    /// Process a candle and update historical data
    pub fn process_candle(&mut self, candle: &Candle) {
        // Update CVD history
        let cvd_history = self
            .cvd_history
            .entry(candle.symbol.clone())
            .or_insert_with(Vec::new);
        cvd_history.push(candle.cvd);
        if cvd_history.len() > self.max_history {
            cvd_history.remove(0);
        }

        // Update volume history
        let volume_history = self
            .volume_history
            .entry(candle.symbol.clone())
            .or_insert_with(Vec::new);
        volume_history.push(candle.volume);
        if volume_history.len() > self.max_history {
            volume_history.remove(0);
        }
    }

    /// Get global CVD for a symbol
    pub fn get_global_cvd(&self, symbol: &str) -> f64 {
        self.global_cvd.get(symbol).copied().unwrap_or(0.0)
    }

    /// Get CVD percentile for a symbol (0.0 to 1.0)
    pub fn get_cvd_percentile(&self, symbol: &str, value: f64) -> f64 {
        let history = match self.cvd_history.get(symbol) {
            Some(h) => h,
            None => return 0.0,
        };

        if history.is_empty() {
            return 0.0;
        }

        let abs_value = value.abs();
        let count = history.iter().filter(|&&v| v.abs() <= abs_value).count();
        count as f64 / history.len() as f64
    }

    /// Get CVD in top N percentile for a symbol
    pub fn is_cvd_in_top_percentile(&self, symbol: &str, value: f64, percentile: f64) -> bool {
        let actual_percentile = self.get_cvd_percentile(symbol, value);
        actual_percentile >= percentile
    }

    /// Get RVOL (Relative Volume) for a symbol
    pub fn get_rvol(&self, symbol: &str, current_volume: f64) -> f64 {
        let history = match self.volume_history.get(symbol) {
            Some(h) => h,
            None => return 1.0,
        };

        if history.is_empty() {
            return 1.0;
        }

        let avg_volume: f64 = history.iter().sum::<f64>() / history.len() as f64;
        if avg_volume == 0.0 {
            return 1.0;
        }

        current_volume / avg_volume
    }

    /// Get average CVD magnitude for a symbol
    pub fn get_avg_cvd_magnitude(&self, symbol: &str) -> f64 {
        let history = match self.cvd_history.get(symbol) {
            Some(h) => h,
            None => return 0.0,
        };

        if history.is_empty() {
            return 0.0;
        }

        let sum: f64 = history.iter().map(|v| v.abs()).sum();
        sum / history.len() as f64
    }

    /// Get CVD history for a symbol
    pub fn get_cvd_history(&self, symbol: &str) -> Vec<f64> {
        self.cvd_history.get(symbol).cloned().unwrap_or_default()
    }

    /// Get volume history for a symbol
    pub fn get_volume_history(&self, symbol: &str) -> Vec<f64> {
        self.volume_history.get(symbol).cloned().unwrap_or_default()
    }

    /// Clear all data for a symbol
    pub fn clear_symbol(&mut self, symbol: &str) {
        self.global_cvd.remove(symbol);
        self.cvd_history.remove(symbol);
        self.volume_history.remove(symbol);
    }

    /// Clear all data
    pub fn clear_all(&mut self) {
        self.global_cvd.clear();
        self.cvd_history.clear();
        self.volume_history.clear();
    }
}

impl Default for IndicatorCompute {
    fn default() -> Self {
        Self::new(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use cvdtrader_core::Side;

    #[test]
    fn test_global_cvd() {
        let mut indicators = IndicatorCompute::new(100);

        let trade1 = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, Utc::now());
        let trade2 = Trade::new("BTC".to_string(), 50000.0, 0.5, Side::Sell, Utc::now());

        indicators.process_trade(&trade1);
        indicators.process_trade(&trade2);

        assert_eq!(indicators.get_global_cvd("BTC"), 0.5); // 1.0 - 0.5
    }

    #[test]
    fn test_cvd_percentile() {
        let mut indicators = IndicatorCompute::new(100);

        // Add some CVD history
        let mut candle1 = Candle::new("BTC".to_string(), Utc::now());
        candle1.cvd = 10.0;
        let mut candle2 = Candle::new("BTC".to_string(), Utc::now());
        candle2.cvd = 20.0;
        let mut candle3 = Candle::new("BTC".to_string(), Utc::now());
        candle3.cvd = 30.0;

        indicators.process_candle(&candle1);
        indicators.process_candle(&candle2);
        indicators.process_candle(&candle3);

        // Test percentile calculation
        let percentile = indicators.get_cvd_percentile("BTC", 25.0);
        assert!(percentile >= 0.66 && percentile <= 0.67); // 2/3
    }

    #[test]
    fn test_cvd_top_percentile() {
        let mut indicators = IndicatorCompute::new(100);

        // Add some CVD history
        for i in 1..=10 {
            let mut candle = Candle::new("BTC".to_string(), Utc::now());
            candle.cvd = i as f64 * 10.0;
            indicators.process_candle(&candle);
        }

        // 90th percentile should be around 90.0
        assert!(indicators.is_cvd_in_top_percentile("BTC", 95.0, 0.90));
        assert!(!indicators.is_cvd_in_top_percentile("BTC", 50.0, 0.90));
    }

    #[test]
    fn test_rvol() {
        let mut indicators = IndicatorCompute::new(100);

        // Add some volume history
        for i in 1..=5 {
            let mut candle = Candle::new("BTC".to_string(), Utc::now());
            candle.volume = i as f64 * 10.0;
            indicators.process_candle(&candle);
        }

        // Average volume = (10+20+30+40+50)/5 = 30
        // Current volume = 60
        // RVOL = 60/30 = 2.0
        let rvol = indicators.get_rvol("BTC", 60.0);
        assert_eq!(rvol, 2.0);
    }

    #[test]
    fn test_avg_cvd_magnitude() {
        let mut indicators = IndicatorCompute::new(100);

        let mut candle1 = Candle::new("BTC".to_string(), Utc::now());
        candle1.cvd = 10.0;
        let mut candle2 = Candle::new("BTC".to_string(), Utc::now());
        candle2.cvd = -20.0;
        let mut candle3 = Candle::new("BTC".to_string(), Utc::now());
        candle3.cvd = 30.0;

        indicators.process_candle(&candle1);
        indicators.process_candle(&candle2);
        indicators.process_candle(&candle3);

        let avg = indicators.get_avg_cvd_magnitude("BTC");
        assert_eq!(avg, 20.0); // (10+20+30)/3
    }

    #[test]
    fn test_clear_symbol() {
        let mut indicators = IndicatorCompute::new(100);

        let trade = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, Utc::now());
        indicators.process_trade(&trade);

        assert_eq!(indicators.get_global_cvd("BTC"), 1.0);

        indicators.clear_symbol("BTC");
        assert_eq!(indicators.get_global_cvd("BTC"), 0.0);
    }
}
