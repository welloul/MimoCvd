use cvdtrader_core::{ExecutionMode, GlobalState, TradeSignal};
use tracing::{debug, info, warn};

/// Risk manager for enforcing trading constraints
pub struct RiskManager {
    /// Global state
    state: GlobalState,
    /// Maximum position size in USD
    max_position_usd: f64,
    /// Maximum leverage
    max_leverage: f64,
    /// Maximum drawdown percentage
    max_drawdown_pct: f64,
    /// Account balance
    account_balance: f64,
    /// Execution mode
    execution_mode: ExecutionMode,
}

impl RiskManager {
    /// Create a new risk manager
    pub fn new(
        state: GlobalState,
        max_position_usd: f64,
        max_leverage: f64,
        max_drawdown_pct: f64,
        account_balance: f64,
        execution_mode: ExecutionMode,
    ) -> Self {
        Self {
            state,
            max_position_usd,
            max_leverage,
            max_drawdown_pct,
            account_balance,
            execution_mode,
        }
    }

    /// Fetch account balance from exchange API (for live/testnet modes)
    /// This is a placeholder - actual implementation would call exchange API
    pub async fn fetch_account_balance(&self, api_url: &str) -> Result<f64, String> {
        // In live/testnet mode, fetch from exchange API
        // For now, return error to indicate this needs implementation
        Err(format!(
            "Account balance fetch not implemented for {} mode. API URL: {}",
            self.execution_mode, api_url
        ))
    }

    /// Update account balance (for live mode)
    pub fn update_account_balance(&mut self, balance: f64) {
        info!("Updating account balance to {}", balance);
        self.account_balance = balance;
    }

    /// Get execution mode
    pub fn execution_mode(&self) -> ExecutionMode {
        self.execution_mode
    }

    /// Validate a trade signal against risk constraints
    pub async fn validate_signal(&self, signal: &TradeSignal) -> Result<(), String> {
        debug!(
            "Validating signal for {}: {} @ {} (size: {})",
            signal.symbol, signal.signal, signal.entry_price, signal.size
        );

        // Check position size limit
        let position_value = signal.entry_price * signal.size;
        if position_value > self.max_position_usd {
            return Err(format!(
                "Position size {} USD exceeds maximum {} USD",
                position_value, self.max_position_usd
            ));
        }

        // Check leverage
        let total_exposure = self.get_total_exposure().await;
        let new_exposure = total_exposure + position_value;
        let leverage = new_exposure / self.account_balance;

        if leverage > self.max_leverage {
            return Err(format!(
                "Leverage {} exceeds maximum {} (exposure: {}, balance: {})",
                leverage, self.max_leverage, new_exposure, self.account_balance
            ));
        }

        // Check drawdown
        let current_drawdown = self.calculate_drawdown().await;
        if current_drawdown > self.max_drawdown_pct {
            return Err(format!(
                "Drawdown {}% exceeds maximum {}%",
                current_drawdown * 100.0,
                self.max_drawdown_pct * 100.0
            ));
        }

        // Check if we already have a position for this symbol
        if self.state.has_position(&signal.symbol).await {
            return Err(format!("Already have a position for {}", signal.symbol));
        }

        info!(
            "Signal validated for {}: {} @ {} (size: {})",
            signal.symbol, signal.signal, signal.entry_price, signal.size
        );

        Ok(())
    }

    /// Get total exposure across all positions
    async fn get_total_exposure(&self) -> f64 {
        let positions = self.state.get_all_positions().await;
        positions.values().map(|p| p.entry_price * p.size).sum()
    }

    /// Calculate current drawdown
    async fn calculate_drawdown(&self) -> f64 {
        let positions = self.state.get_all_positions().await;
        let total_pnl: f64 = positions.values().map(|p| p.unrealized_pnl).sum();

        if total_pnl >= 0.0 {
            return 0.0;
        }

        let drawdown = -total_pnl / self.account_balance;
        drawdown
    }

    /// Get account balance
    pub fn account_balance(&self) -> f64 {
        self.account_balance
    }

    /// Set account balance
    pub fn set_account_balance(&mut self, balance: f64) {
        self.account_balance = balance;
    }

    /// Get maximum position size
    pub fn max_position_usd(&self) -> f64 {
        self.max_position_usd
    }

    /// Get maximum leverage
    pub fn max_leverage(&self) -> f64 {
        self.max_leverage
    }

    /// Get maximum drawdown percentage
    pub fn max_drawdown_pct(&self) -> f64 {
        self.max_drawdown_pct
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cvdtrader_core::{PositionSide, Signal};

    #[tokio::test]
    async fn test_risk_manager_validate_signal() {
        let state = GlobalState::new();
        let manager = RiskManager::new(state, 1000.0, 10.0, 0.05, 10000.0, ExecutionMode::DryRun);

        let signal = TradeSignal::new(
            Signal::Long,
            None,
            "BTC".to_string(),
            50000.0,
            49000.0,
            52000.0,
            0.02, // 1000 USD
        );

        assert!(manager.validate_signal(&signal).await.is_ok());
    }

    #[tokio::test]
    async fn test_risk_manager_position_size_limit() {
        let state = GlobalState::new();
        let manager = RiskManager::new(state, 1000.0, 10.0, 0.05, 10000.0, ExecutionMode::DryRun);

        let signal = TradeSignal::new(
            Signal::Long,
            None,
            "BTC".to_string(),
            50000.0,
            49000.0,
            52000.0,
            0.03, // 1500 USD - exceeds limit
        );

        assert!(manager.validate_signal(&signal).await.is_err());
    }

    #[tokio::test]
    async fn test_risk_manager_leverage_limit() {
        let state = GlobalState::new();
        let manager = RiskManager::new(state, 1000.0, 2.0, 0.05, 1000.0, ExecutionMode::DryRun);

        // Add existing position
        let position = cvdtrader_core::Position::new(
            "ETH".to_string(),
            PositionSide::Long,
            1.0,
            3000.0,
            2900.0,
            3200.0,
        );
        manager
            .state
            .set_position("ETH".to_string(), position)
            .await;

        let signal = TradeSignal::new(
            Signal::Long,
            None,
            "BTC".to_string(),
            50000.0,
            49000.0,
            52000.0,
            0.02, // 1000 USD - total exposure 4000 USD, leverage 4x
        );

        assert!(manager.validate_signal(&signal).await.is_err());
    }

    #[tokio::test]
    async fn test_risk_manager_existing_position() {
        let state = GlobalState::new();
        let manager = RiskManager::new(
            state.clone(),
            1000.0,
            10.0,
            0.05,
            10000.0,
            ExecutionMode::DryRun,
        );

        // Add existing position
        let position = cvdtrader_core::Position::new(
            "BTC".to_string(),
            PositionSide::Long,
            1.0,
            50000.0,
            49000.0,
            52000.0,
        );
        manager
            .state
            .set_position("BTC".to_string(), position)
            .await;

        let signal = TradeSignal::new(
            Signal::Long,
            None,
            "BTC".to_string(),
            50000.0,
            49000.0,
            52000.0,
            0.02,
        );

        assert!(manager.validate_signal(&signal).await.is_err());
    }
}
