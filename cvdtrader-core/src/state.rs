use crate::types::{Candle, Order, Position};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Global state for the trading bot
#[derive(Debug, Clone)]
pub struct GlobalState {
    /// Active positions (symbol -> position)
    pub positions: Arc<RwLock<HashMap<String, Position>>>,
    /// Active orders (order_id -> order)
    pub orders: Arc<RwLock<HashMap<String, Order>>>,
    /// Recent candles (symbol -> candles)
    pub candles: Arc<RwLock<HashMap<String, Vec<Candle>>>>,
    /// Global CVD (symbol -> cumulative CVD)
    pub global_cvd: Arc<RwLock<HashMap<String, f64>>>,
    /// Bot running state
    pub is_running: Arc<RwLock<bool>>,
    /// Last update timestamp
    pub last_update: Arc<RwLock<DateTime<Utc>>>,
}

impl GlobalState {
    /// Create a new global state
    pub fn new() -> Self {
        Self {
            positions: Arc::new(RwLock::new(HashMap::new())),
            orders: Arc::new(RwLock::new(HashMap::new())),
            candles: Arc::new(RwLock::new(HashMap::new())),
            global_cvd: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
            last_update: Arc::new(RwLock::new(Utc::now())),
        }
    }

    /// Check if bot is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Set bot running state
    pub async fn set_running(&self, running: bool) {
        *self.is_running.write().await = running;
        *self.last_update.write().await = Utc::now();
    }

    /// Get position for symbol
    pub async fn get_position(&self, symbol: &str) -> Option<Position> {
        self.positions.read().await.get(symbol).cloned()
    }

    /// Check if has position for symbol
    pub async fn has_position(&self, symbol: &str) -> bool {
        self.positions.read().await.contains_key(symbol)
    }

    /// Add or update position
    pub async fn set_position(&self, symbol: String, position: Position) {
        self.positions.write().await.insert(symbol, position);
        *self.last_update.write().await = Utc::now();
    }

    /// Remove position
    pub async fn remove_position(&self, symbol: &str) -> Option<Position> {
        let position = self.positions.write().await.remove(symbol);
        *self.last_update.write().await = Utc::now();
        position
    }

    /// Get all positions
    pub async fn get_all_positions(&self) -> HashMap<String, Position> {
        self.positions.read().await.clone()
    }

    /// Get order by ID
    pub async fn get_order(&self, order_id: &str) -> Option<Order> {
        self.orders.read().await.get(order_id).cloned()
    }

    /// Add order
    pub async fn add_order(&self, order: Order) {
        self.orders.write().await.insert(order.id.clone(), order);
        *self.last_update.write().await = Utc::now();
    }

    /// Update order
    pub async fn update_order(&self, order: Order) {
        self.orders.write().await.insert(order.id.clone(), order);
        *self.last_update.write().await = Utc::now();
    }

    /// Remove order
    pub async fn remove_order(&self, order_id: &str) -> Option<Order> {
        let order = self.orders.write().await.remove(order_id);
        *self.last_update.write().await = Utc::now();
        order
    }

    /// Get all orders
    pub async fn get_all_orders(&self) -> HashMap<String, Order> {
        self.orders.read().await.clone()
    }

    /// Get pending orders for symbol
    pub async fn get_pending_orders(&self, symbol: &str) -> Vec<Order> {
        self.orders
            .read()
            .await
            .values()
            .filter(|o| o.symbol == symbol && o.status == crate::types::OrderStatus::Pending)
            .cloned()
            .collect()
    }

    /// Get candles for symbol
    pub async fn get_candles(&self, symbol: &str) -> Vec<Candle> {
        self.candles
            .read()
            .await
            .get(symbol)
            .cloned()
            .unwrap_or_default()
    }

    /// Add candle for symbol
    pub async fn add_candle(&self, symbol: String, candle: Candle) {
        let mut candles = self.candles.write().await;
        let symbol_candles = candles.entry(symbol).or_insert_with(Vec::new);
        symbol_candles.push(candle);

        // Keep only last 100 candles per symbol
        if symbol_candles.len() > 100 {
            symbol_candles.remove(0);
        }

        *self.last_update.write().await = Utc::now();
    }

    /// Get last N candles for symbol
    pub async fn get_last_candles(&self, symbol: &str, n: usize) -> Vec<Candle> {
        let candles = self.candles.read().await;
        if let Some(symbol_candles) = candles.get(symbol) {
            let start = if symbol_candles.len() > n {
                symbol_candles.len() - n
            } else {
                0
            };
            symbol_candles[start..].to_vec()
        } else {
            Vec::new()
        }
    }

    /// Get global CVD for symbol
    pub async fn get_global_cvd(&self, symbol: &str) -> f64 {
        self.global_cvd
            .read()
            .await
            .get(symbol)
            .copied()
            .unwrap_or(0.0)
    }

    /// Update global CVD for symbol
    pub async fn update_global_cvd(&self, symbol: String, delta: f64) {
        let mut cvd = self.global_cvd.write().await;
        let entry = cvd.entry(symbol).or_insert(0.0);
        *entry += delta;
        *self.last_update.write().await = Utc::now();
    }

    /// Set global CVD for symbol
    pub async fn set_global_cvd(&self, symbol: String, cvd: f64) {
        self.global_cvd.write().await.insert(symbol, cvd);
        *self.last_update.write().await = Utc::now();
    }

    /// Get last update timestamp
    pub async fn last_update(&self) -> DateTime<Utc> {
        *self.last_update.read().await
    }

    /// Clear all state (for testing)
    pub async fn clear(&self) {
        self.positions.write().await.clear();
        self.orders.write().await.clear();
        self.candles.write().await.clear();
        self.global_cvd.write().await.clear();
        *self.is_running.write().await = false;
        *self.last_update.write().await = Utc::now();
    }
}

impl Default for GlobalState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{OrderSide, PositionSide};

    #[tokio::test]
    async fn test_state_position() {
        let state = GlobalState::new();
        let position = Position::new(
            "BTC".to_string(),
            PositionSide::Long,
            1.0,
            50000.0,
            49000.0,
            52000.0,
        );

        assert!(!state.has_position("BTC").await);
        state.set_position("BTC".to_string(), position.clone()).await;
        assert!(state.has_position("BTC").await);

        let retrieved = state.get_position("BTC").await.unwrap();
        assert_eq!(retrieved.symbol, "BTC");
        assert_eq!(retrieved.size, 1.0);

        state.remove_position("BTC").await;
        assert!(!state.has_position("BTC").await);
    }

    #[tokio::test]
    async fn test_state_order() {
        let state = GlobalState::new();
        let order = Order::new("BTC".to_string(), OrderSide::Buy, 50000.0, 1.0);

        assert!(state.get_order(&order.id).await.is_none());
        state.add_order(order.clone()).await;
        assert!(state.get_order(&order.id).await.is_some());

        let retrieved = state.get_order(&order.id).await.unwrap();
        assert_eq!(retrieved.symbol, "BTC");
        assert_eq!(retrieved.price, 50000.0);

        state.remove_order(&order.id).await;
        assert!(state.get_order(&order.id).await.is_none());
    }

    #[tokio::test]
    async fn test_state_candles() {
        let state = GlobalState::new();
        let candle = Candle::new("BTC".to_string(), Utc::now());

        assert!(state.get_candles("BTC").await.is_empty());
        state.add_candle("BTC".to_string(), candle).await;
        assert_eq!(state.get_candles("BTC").await.len(), 1);

        let last = state.get_last_candles("BTC", 1).await;
        assert_eq!(last.len(), 1);
    }

    #[tokio::test]
    async fn test_state_cvd() {
        let state = GlobalState::new();

        assert_eq!(state.get_global_cvd("BTC").await, 0.0);
        state.update_global_cvd("BTC".to_string(), 10.0).await;
        assert_eq!(state.get_global_cvd("BTC").await, 10.0);

        state.update_global_cvd("BTC".to_string(), -5.0).await;
        assert_eq!(state.get_global_cvd("BTC").await, 5.0);

        state.set_global_cvd("BTC".to_string(), 100.0).await;
        assert_eq!(state.get_global_cvd("BTC").await, 100.0);
    }

    #[tokio::test]
    async fn test_state_running() {
        let state = GlobalState::new();

        assert!(!state.is_running().await);
        state.set_running(true).await;
        assert!(state.is_running().await);
        state.set_running(false).await;
        assert!(!state.is_running().await);
    }
}
