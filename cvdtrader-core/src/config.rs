use crate::types::ExecutionMode;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Exchange configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub name: String,
    pub ws_url: String,
    pub api_url: String,
    pub symbols: Vec<String>,
}

impl Default for ExchangeConfig {
    fn default() -> Self {
        Self {
            name: "hyperliquid".to_string(),
            ws_url: "wss://api.hyperliquid.xyz/ws".to_string(),
            api_url: "https://api.hyperliquid.xyz".to_string(),
            symbols: vec!["BTC".to_string()],
        }
    }
}

/// Strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub name: String,
    pub lookback: usize,
    pub cvd_exhaustion_ratio: f64,
    pub cvd_absorption_pctile: f64,
    pub sl_offset: i32,
    pub risk_r_multiple: f64,
    pub entry_offset_pct: f64,
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            name: "cvd_poc".to_string(),
            lookback: 20,
            cvd_exhaustion_ratio: 0.70,
            cvd_absorption_pctile: 0.90,
            sl_offset: 2,
            risk_r_multiple: 1.5,
            entry_offset_pct: 0.001, // 0.1%
        }
    }
}

/// Risk configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub max_position_usd: f64,
    pub max_leverage: f64,
    pub max_drawdown_pct: f64,
    pub circuit_breaker_latency_ms: u64,
    pub circuit_breaker_failures: u32,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_usd: 1000.0,
            max_leverage: 10.0,
            max_drawdown_pct: 0.05, // 5%
            circuit_breaker_latency_ms: 500,
            circuit_breaker_failures: 3,
        }
    }
}

/// Execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub mode: ExecutionMode,
    pub ttl_seconds: i64,
    pub post_only: bool,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            mode: ExecutionMode::DryRun,
            ttl_seconds: 120,
            post_only: true,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "json".to_string(),
        }
    }
}

/// Main configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub exchange: ExchangeConfig,
    pub strategy: StrategyConfig,
    pub risk: RiskConfig,
    pub execution: ExecutionConfig,
    pub logging: LoggingConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            exchange: ExchangeConfig::default(),
            strategy: StrategyConfig::default(),
            risk: RiskConfig::default(),
            execution: ExecutionConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl Config {
    /// Load configuration from TOML file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from TOML string
    pub fn from_str(s: &str) -> Result<Self> {
        let config: Config = toml::from_str(s).context("Failed to parse config string")?;
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate exchange config
        if self.exchange.symbols.is_empty() {
            anyhow::bail!("At least one symbol must be configured");
        }

        if self.exchange.ws_url.is_empty() {
            anyhow::bail!("WebSocket URL must be configured");
        }

        // Validate strategy config
        if self.strategy.lookback == 0 {
            anyhow::bail!("Lookback must be greater than 0");
        }

        if self.strategy.cvd_exhaustion_ratio <= 0.0 || self.strategy.cvd_exhaustion_ratio > 1.0 {
            anyhow::bail!("CVD exhaustion ratio must be between 0 and 1");
        }

        if self.strategy.cvd_absorption_pctile <= 0.0 || self.strategy.cvd_absorption_pctile > 1.0 {
            anyhow::bail!("CVD absorption percentile must be between 0 and 1");
        }

        if self.strategy.risk_r_multiple <= 0.0 {
            anyhow::bail!("Risk R-multiple must be greater than 0");
        }

        if self.strategy.entry_offset_pct <= 0.0 || self.strategy.entry_offset_pct >= 1.0 {
            anyhow::bail!("Entry offset percentage must be between 0 and 1");
        }

        // Validate risk config
        if self.risk.max_position_usd <= 0.0 {
            anyhow::bail!("Max position USD must be greater than 0");
        }

        if self.risk.max_leverage <= 0.0 {
            anyhow::bail!("Max leverage must be greater than 0");
        }

        if self.risk.max_drawdown_pct <= 0.0 || self.risk.max_drawdown_pct > 1.0 {
            anyhow::bail!("Max drawdown percentage must be between 0 and 1");
        }

        // Validate execution config
        if self.execution.ttl_seconds <= 0 {
            anyhow::bail!("TTL seconds must be greater than 0");
        }

        // Validate logging config
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.logging.level.as_str()) {
            anyhow::bail!("Invalid log level: {}", self.logging.level);
        }

        let valid_formats = ["json", "pretty"];
        if !valid_formats.contains(&self.logging.format.as_str()) {
            anyhow::bail!("Invalid log format: {}", self.logging.format);
        }

        Ok(())
    }

    /// Save configuration to TOML file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let content = toml::to_string_pretty(self).context("Failed to serialize config to TOML")?;

        std::fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }

    /// Get tick size for symbol (simplified - in production, fetch from exchange)
    pub fn tick_size(&self, symbol: &str) -> f64 {
        match symbol {
            "BTC" => 1.0,
            "ETH" => 0.1,
            "SOL" => 0.01,
            _ => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.exchange.symbols, vec!["BTC"]);
        assert_eq!(config.strategy.lookback, 20);
        assert_eq!(config.risk.max_position_usd, 1000.0);
    }

    #[test]
    fn test_config_from_str() {
        let toml_str = r#"
[exchange]
name = "hyperliquid"
ws_url = "wss://api.hyperliquid.xyz/ws"
api_url = "https://api.hyperliquid.xyz"
symbols = ["BTC", "ETH"]

[strategy]
name = "cvd_poc"
lookback = 20
cvd_exhaustion_ratio = 0.70
cvd_absorption_pctile = 0.90
sl_offset = 2
risk_r_multiple = 1.5
entry_offset_pct = 0.001

[risk]
max_position_usd = 1000.0
max_leverage = 10.0
max_drawdown_pct = 0.05
circuit_breaker_latency_ms = 500
circuit_breaker_failures = 3

[execution]
mode = "dryrun"
ttl_seconds = 120
post_only = true

[logging]
level = "info"
format = "json"
"#;

        let config = Config::from_str(toml_str).unwrap();
        assert!(config.validate().is_ok());
        assert_eq!(config.exchange.symbols.len(), 2);
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();

        // Test invalid lookback
        config.strategy.lookback = 0;
        assert!(config.validate().is_err());

        // Test invalid CVD ratio
        config.strategy.lookback = 20;
        config.strategy.cvd_exhaustion_ratio = 1.5;
        assert!(config.validate().is_err());

        // Test invalid max position
        config.strategy.cvd_exhaustion_ratio = 0.70;
        config.risk.max_position_usd = -100.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_tick_size() {
        let config = Config::default();
        assert_eq!(config.tick_size("BTC"), 1.0);
        assert_eq!(config.tick_size("ETH"), 0.1);
        assert_eq!(config.tick_size("SOL"), 0.01);
        assert_eq!(config.tick_size("UNKNOWN"), 1.0);
    }
}
