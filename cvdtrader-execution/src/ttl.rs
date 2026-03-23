use cvdtrader_core::{GlobalState, OrderStatus};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};
use tracing::{debug, info, warn};

/// Order TTL tracker for automatic order cancellation
pub struct OrderTtlTracker {
    /// Global state
    state: GlobalState,
    /// TTL in seconds
    ttl_seconds: i64,
    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
}

impl OrderTtlTracker {
    /// Create a new order TTL tracker
    pub fn new(state: GlobalState, ttl_seconds: i64, shutdown_tx: broadcast::Sender<()>) -> Self {
        Self {
            state,
            ttl_seconds,
            shutdown_tx,
        }
    }

    /// Start the TTL tracker background task
    pub async fn start(&self) {
        let state = self.state.clone();
        let ttl_seconds = self.ttl_seconds;
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10)); // Check every 10 seconds

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::check_expired_orders(&state, ttl_seconds).await;
                    }
                    _ = shutdown_rx.recv() => {
                        info!("TTL tracker shutting down");
                        break;
                    }
                }
            }
        });

        info!("Order TTL tracker started (TTL: {}s)", ttl_seconds);
    }

    /// Check for expired orders and cancel them
    async fn check_expired_orders(state: &GlobalState, ttl_seconds: i64) {
        let orders = state.get_all_orders().await;

        for (order_id, order) in orders {
            if order.status == OrderStatus::Pending && order.is_expired(ttl_seconds) {
                warn!(
                    "Order {} expired ({}s), cancelling: {} {} @ {}",
                    order_id,
                    ttl_seconds,
                    order.side,
                    order.symbol,
                    order.price
                );

                // Update order status to cancelled
                let mut order = order;
                order.update_status(OrderStatus::Cancelled);
                state.update_order(order).await;

                debug!("Order {} cancelled due to TTL expiry", order_id);
            }
        }
    }

    /// Get TTL in seconds
    pub fn ttl_seconds(&self) -> i64 {
        self.ttl_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use cvdtrader_core::{Order, OrderSide};

    #[tokio::test]
    async fn test_ttl_tracker_no_expired_orders() {
        let state = GlobalState::new();
        let (shutdown_tx, _) = broadcast::channel(1);
        let tracker = OrderTtlTracker::new(state.clone(), 120, shutdown_tx);

        // Add a recent order
        let order = Order::new("BTC".to_string(), OrderSide::Buy, 50000.0, 1.0);
        state.add_order(order).await;

        // Check expired orders
        OrderTtlTracker::check_expired_orders(&state, 120).await;

        // Order should still be pending
        let orders = state.get_all_orders().await;
        assert_eq!(orders.len(), 1);
        assert_eq!(orders.values().next().unwrap().status, OrderStatus::Pending);
    }

    #[tokio::test]
    async fn test_ttl_tracker_expired_order() {
        let state = GlobalState::new();
        let (shutdown_tx, _) = broadcast::channel(1);
        let tracker = OrderTtlTracker::new(state.clone(), 1, shutdown_tx);

        // Add an old order
        let mut order = Order::new("BTC".to_string(), OrderSide::Buy, 50000.0, 1.0);
        // Manually set created_at to 2 seconds ago
        order.created_at = Utc::now() - chrono::Duration::seconds(2);
        state.add_order(order).await;

        // Check expired orders
        OrderTtlTracker::check_expired_orders(&state, 1).await;

        // Order should be cancelled
        let orders = state.get_all_orders().await;
        assert_eq!(orders.len(), 1);
        assert_eq!(orders.values().next().unwrap().status, OrderStatus::Cancelled);
    }
}
