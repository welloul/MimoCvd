use anyhow::{Context, Result};
use cvdtrader_core::{ExecutionMode, GlobalState, Order, OrderSide, OrderStatus, TradeSignal};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

/// Order request for Hyperliquid API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderRequest {
    coin: String,
    is_buy: bool,
    limit_px: String,
    sz: String,
    order_type: OrderType,
    reduce_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderType {
    limit: LimitOrderType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LimitOrderType {
    tif: String,
}

/// Order response from Hyperliquid API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderResponse {
    status: String,
    response: OrderResponseData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderResponseData {
    #[serde(rename = "type")]
    response_type: String,
    data: Option<OrderResponseDataInner>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderResponseDataInner {
    statuses: Vec<String>,
}

/// Execution gateway for order placement
pub struct ExecutionGateway {
    /// HTTP client
    client: Client,
    /// API base URL
    api_url: String,
    /// Execution mode
    mode: ExecutionMode,
    /// Global state
    state: GlobalState,
    /// Post-only flag
    post_only: bool,
}

impl ExecutionGateway {
    /// Create a new execution gateway
    pub fn new(
        api_url: String,
        mode: ExecutionMode,
        state: GlobalState,
        post_only: bool,
    ) -> Self {
        Self {
            client: Client::new(),
            api_url,
            mode,
            state,
            post_only,
        }
    }

    /// Place an order based on a trade signal
    pub async fn place_order(&self, signal: &TradeSignal) -> Result<Order> {
        info!(
            "Placing {} order for {}: {} @ {} (size: {})",
            signal.signal, signal.symbol, signal.signal, signal.entry_price, signal.size
        );

        // Create order
        let mut order = Order::new(
            signal.symbol.clone(),
            match signal.signal {
                cvdtrader_core::Signal::Long => OrderSide::Buy,
                cvdtrader_core::Signal::Short => OrderSide::Sell,
                cvdtrader_core::Signal::None => anyhow::bail!("Invalid signal: None"),
            },
            signal.entry_price,
            signal.size,
        );

        match self.mode {
            ExecutionMode::DryRun => {
                // Simulate immediate fill in dry run mode
                order.update_status(OrderStatus::Filled);
                order.update_fill(signal.size, signal.entry_price);
                info!("Dry run: Order filled immediately @ {}", signal.entry_price);
            }
            ExecutionMode::TestNet | ExecutionMode::Live => {
                // Place real order via API
                self.place_order_api(&mut order).await?;
            }
        }

        // Store order in state
        self.state.add_order(order.clone()).await;

        Ok(order)
    }

    /// Place order via Hyperliquid API
    async fn place_order_api(&self, order: &mut Order) -> Result<()> {
        let order_request = OrderRequest {
            coin: order.symbol.clone(),
            is_buy: order.side == OrderSide::Buy,
            limit_px: format!("{:.2}", order.price),
            sz: format!("{:.6}", order.size),
            order_type: OrderType {
                limit: LimitOrderType {
                    tif: "Alo".to_string(), // Post-Only
                },
            },
            reduce_only: false,
        };

        let url = format!("{}/exchange", self.api_url);
        let response = self
            .client
            .post(&url)
            .json(&order_request)
            .send()
            .await
            .context("Failed to send order request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("Order placement failed: {}", error_text);
            order.update_status(OrderStatus::Rejected);
            anyhow::bail!("Order placement failed: {}", error_text);
        }

        let order_response: OrderResponse = response
            .json()
            .await
            .context("Failed to parse order response")?;

        if order_response.status != "ok" {
            error!("Order placement failed: {:?}", order_response);
            order.update_status(OrderStatus::Rejected);
            anyhow::bail!("Order placement failed: {:?}", order_response);
        }

        order.update_status(OrderStatus::Pending);
        info!("Order placed successfully: {}", order.id);

        Ok(())
    }

    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let order = self
            .state
            .get_order(order_id)
            .await
            .context("Order not found")?;

        info!("Cancelling order: {} ({})", order_id, order.symbol);

        match self.mode {
            ExecutionMode::DryRun => {
                // Simulate cancellation in dry run mode
                let mut order = order;
                order.update_status(OrderStatus::Cancelled);
                self.state.update_order(order).await;
                info!("Dry run: Order cancelled");
            }
            ExecutionMode::TestNet | ExecutionMode::Live => {
                // Cancel via API
                self.cancel_order_api(order_id).await?;
            }
        }

        Ok(())
    }

    /// Cancel order via Hyperliquid API
    async fn cancel_order_api(&self, order_id: &str) -> Result<()> {
        let url = format!("{}/exchange", self.api_url);
        let cancel_request = serde_json::json!({
            "action": {
                "type": "cancel",
                "cancels": [{
                    "oid": order_id,
                }]
            }
        });

        let response = self
            .client
            .post(&url)
            .json(&cancel_request)
            .send()
            .await
            .context("Failed to send cancel request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("Order cancellation failed: {}", error_text);
            anyhow::bail!("Order cancellation failed: {}", error_text);
        }

        let mut order = self.state.get_order(order_id).await.context("Order not found")?;
        order.update_status(OrderStatus::Cancelled);
        self.state.update_order(order).await;

        info!("Order cancelled successfully: {}", order_id);
        Ok(())
    }

    /// Close a position (market order)
    pub async fn close_position(&self, symbol: &str) -> Result<()> {
        let position = self
            .state
            .get_position(symbol)
            .await
            .context("Position not found")?;

        info!(
            "Closing position for {}: {} {} @ {}",
            symbol, position.side, position.size, position.entry_price
        );

        match self.mode {
            ExecutionMode::DryRun => {
                // Simulate position close in dry run mode
                self.state.remove_position(symbol).await;
                info!("Dry run: Position closed");
            }
            ExecutionMode::TestNet | ExecutionMode::Live => {
                // Close via API
                self.close_position_api(symbol).await?;
            }
        }

        Ok(())
    }

    /// Close position via Hyperliquid API
    async fn close_position_api(&self, symbol: &str) -> Result<()> {
        let position = self
            .state
            .get_position(symbol)
            .await
            .context("Position not found")?;

        let url = format!("{}/exchange", self.api_url);
        let close_request = serde_json::json!({
            "action": {
                "type": "order",
                "orders": [{
                    "coin": symbol,
                    "is_buy": position.side == cvdtrader_core::PositionSide::Short,
                    "limit_px": "0", // Market order
                    "sz": format!("{:.6}", position.size),
                    "order_type": {
                        "limit": {
                            "tif": "Ioc" // Immediate or cancel
                        }
                    },
                    "reduce_only": true,
                }]
            }
        });

        let response = self
            .client
            .post(&url)
            .json(&close_request)
            .send()
            .await
            .context("Failed to send close request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("Position close failed: {}", error_text);
            anyhow::bail!("Position close failed: {}", error_text);
        }

        self.state.remove_position(symbol).await;
        info!("Position closed successfully: {}", symbol);

        Ok(())
    }

    /// Get execution mode
    pub fn mode(&self) -> ExecutionMode {
        self.mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cvdtrader_core::Signal;

    #[tokio::test]
    async fn test_gateway_dry_run() {
        let state = GlobalState::new();
        let gateway = ExecutionGateway::new(
            "https://api.hyperliquid.xyz".to_string(),
            ExecutionMode::DryRun,
            state.clone(),
            true,
        );

        let signal = TradeSignal::new(
            Signal::Long,
            None,
            "BTC".to_string(),
            50000.0,
            49000.0,
            52000.0,
            0.02,
        );

        let order = gateway.place_order(&signal).await.unwrap();
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.filled_size, 0.02);
        assert_eq!(order.filled_price, 50000.0);

        // Check order is in state
        let stored_order = state.get_order(&order.id).await.unwrap();
        assert_eq!(stored_order.status, OrderStatus::Filled);
    }

    #[tokio::test]
    async fn test_gateway_cancel_order() {
        let state = GlobalState::new();
        let gateway = ExecutionGateway::new(
            "https://api.hyperliquid.xyz".to_string(),
            ExecutionMode::DryRun,
            state.clone(),
            true,
        );

        let signal = TradeSignal::new(
            Signal::Long,
            None,
            "BTC".to_string(),
            50000.0,
            49000.0,
            52000.0,
            0.02,
        );

        let order = gateway.place_order(&signal).await.unwrap();
        gateway.cancel_order(&order.id).await.unwrap();

        let stored_order = state.get_order(&order.id).await.unwrap();
        assert_eq!(stored_order.status, OrderStatus::Cancelled);
    }
}
