use cvdtrader_core::{GlobalState, Order, OrderStatus, Position, PositionSide};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Fill event from exchange
#[derive(Debug, Clone)]
pub struct FillEvent {
    pub order_id: String,
    pub symbol: String,
    pub side: PositionSide,
    pub filled_size: f64,
    pub filled_price: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Fill tracker for order fill confirmation
pub struct FillTracker {
    /// Global state
    state: GlobalState,
    /// Channel to receive fill events
    fill_rx: mpsc::Receiver<FillEvent>,
    /// Channel to send fill events (for testing)
    fill_tx: mpsc::Sender<FillEvent>,
}

impl FillTracker {
    /// Create a new fill tracker
    pub fn new(state: GlobalState) -> Self {
        let (fill_tx, fill_rx) = mpsc::channel(100);
        Self {
            state,
            fill_rx,
            fill_tx,
        }
    }

    /// Get fill sender for external use
    pub fn fill_sender(&self) -> mpsc::Sender<FillEvent> {
        self.fill_tx.clone()
    }

    /// Start the fill tracker background task
    pub async fn start(&mut self) {
        info!("Fill tracker started");

        while let Some(fill_event) = self.fill_rx.recv().await {
            self.process_fill(fill_event).await;
        }

        info!("Fill tracker stopped");
    }

    /// Process a fill event
    async fn process_fill(&self, fill_event: FillEvent) {
        debug!(
            "Processing fill: {} {} {} @ {}",
            fill_event.order_id,
            fill_event.side,
            fill_event.filled_size,
            fill_event.filled_price
        );

        // Update order status
        if let Some(mut order) = self.state.get_order(&fill_event.order_id).await {
            order.update_fill(fill_event.filled_size, fill_event.filled_price);
            self.state.update_order(order).await;

            info!(
                "Order {} filled: {} {} @ {}",
                fill_event.order_id,
                fill_event.side,
                fill_event.filled_size,
                fill_event.filled_price
            );
        } else {
            warn!("Order not found for fill: {}", fill_event.order_id);
        }

        // Create or update position
        let position = Position::new(
            fill_event.symbol.clone(),
            fill_event.side,
            fill_event.filled_size,
            fill_event.filled_price,
            0.0, // SL will be set by strategy
            0.0, // TP will be set by strategy
        );

        self.state
            .set_position(fill_event.symbol.clone(), position)
            .await;

        info!(
            "Position created for {}: {} {} @ {}",
            fill_event.symbol, fill_event.side, fill_event.filled_size, fill_event.filled_price
        );
    }

    /// Simulate a fill (for testing)
    pub async fn simulate_fill(&self, fill_event: FillEvent) {
        if let Err(e) = self.fill_tx.send(fill_event).await {
            warn!("Failed to send fill event: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use cvdtrader_core::{Order, OrderSide};

    #[tokio::test]
    async fn test_fill_tracker_process_fill() {
        let state = GlobalState::new();
        let mut tracker = FillTracker::new(state.clone());

        // Add an order
        let order = Order::new("BTC".to_string(), OrderSide::Buy, 50000.0, 1.0);
        let order_id = order.id.clone();
        state.add_order(order).await;

        // Create fill event
        let fill_event = FillEvent {
            order_id: order_id.clone(),
            symbol: "BTC".to_string(),
            side: PositionSide::Long,
            filled_size: 1.0,
            filled_price: 50000.0,
            timestamp: Utc::now(),
        };

        // Process fill
        tracker.process_fill(fill_event).await;

        // Check order status
        let order = state.get_order(&order_id).await.unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.filled_size, 1.0);
        assert_eq!(order.filled_price, 50000.0);

        // Check position
        let position = state.get_position("BTC").await.unwrap();
        assert_eq!(position.side, PositionSide::Long);
        assert_eq!(position.size, 1.0);
        assert_eq!(position.entry_price, 50000.0);
    }

    #[tokio::test]
    async fn test_fill_tracker_simulate_fill() {
        let state = GlobalState::new();
        let tracker = FillTracker::new(state.clone());

        // Add an order
        let order = Order::new("BTC".to_string(), OrderSide::Buy, 50000.0, 1.0);
        let order_id = order.id.clone();
        state.add_order(order).await;

        // Simulate fill
        let fill_event = FillEvent {
            order_id: order_id.clone(),
            symbol: "BTC".to_string(),
            side: PositionSide::Long,
            filled_size: 1.0,
            filled_price: 50000.0,
            timestamp: Utc::now(),
        };

        tracker.simulate_fill(fill_event).await;

        // Start tracker to process the fill
        let mut tracker = tracker;
        tokio::spawn(async move {
            tracker.start().await;
        });

        // Give it time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check order status
        let order = state.get_order(&order_id).await.unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
    }
}
