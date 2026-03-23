use anyhow::{Context, Result};
use cvdtrader_core::{Side, Trade};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

/// WebSocket message types from Hyperliquid
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "channel")]
pub enum WsMessage {
    #[serde(rename = "trades")]
    Trades { data: Vec<TradeData> },
    #[serde(rename = "error")]
    Error { data: ErrorData },
}

/// Trade data from WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeData {
    pub coin: String,
    pub side: String,
    pub px: String,
    pub sz: String,
    pub time: u64,
}

/// Error data from WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorData {
    pub msg: String,
}

/// Subscription message
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubscriptionMessage {
    method: String,
    subscription: Subscription,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Subscription {
    #[serde(rename = "type")]
    sub_type: String,
    coin: String,
}

/// Hyperliquid WebSocket client
pub struct HyperliquidWs {
    url: String,
    symbols: Vec<String>,
    trade_tx: mpsc::Sender<Trade>,
    shutdown_tx: broadcast::Sender<()>,
}

impl HyperliquidWs {
    /// Create a new WebSocket client
    pub fn new(
        url: String,
        symbols: Vec<String>,
        trade_tx: mpsc::Sender<Trade>,
        shutdown_tx: broadcast::Sender<()>,
    ) -> Self {
        Self {
            url,
            symbols,
            trade_tx,
            shutdown_tx,
        }
    }

    /// Start the WebSocket connection
    pub async fn start(&self) -> Result<()> {
        info!("Connecting to Hyperliquid WebSocket: {}", self.url);

        let (ws_stream, _) = connect_async(&self.url)
            .await
            .context("Failed to connect to WebSocket")?;

        info!("WebSocket connected successfully");

        let (mut write, mut read) = ws_stream.split();

        // Subscribe to trade data for all symbols
        for symbol in &self.symbols {
            let subscription = SubscriptionMessage {
                method: "subscribe".to_string(),
                subscription: Subscription {
                    sub_type: "trades".to_string(),
                    coin: symbol.clone(),
                },
            };

            let msg = serde_json::to_string(&subscription)
                .context("Failed to serialize subscription message")?;

            write
                .send(Message::Text(msg))
                .await
                .context("Failed to send subscription message")?;

            info!("Subscribed to trades for {}", symbol);
        }

        // Spawn task to handle incoming messages
        let trade_tx = self.trade_tx.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                if let Err(e) = Self::handle_message(&text, &trade_tx).await {
                                    error!("Error handling message: {}", e);
                                }
                            }
                            Some(Ok(Message::Close(_))) => {
                                info!("WebSocket connection closed");
                                break;
                            }
                            Some(Err(e)) => {
                                error!("WebSocket error: {}", e);
                                break;
                            }
                            None => {
                                warn!("WebSocket stream ended");
                                break;
                            }
                            _ => {}
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Shutdown signal received, closing WebSocket");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Handle incoming WebSocket message
    async fn handle_message(text: &str, trade_tx: &mpsc::Sender<Trade>) -> Result<()> {
        let msg: WsMessage = serde_json::from_str(text).context("Failed to parse WebSocket message")?;

        match msg {
            WsMessage::Trades { data } => {
                for trade_data in data {
                    let trade = Self::parse_trade(trade_data)?;
                    if let Err(e) = trade_tx.send(trade).await {
                        error!("Failed to send trade: {}", e);
                    }
                }
            }
            WsMessage::Error { data } => {
                error!("WebSocket error: {}", data.msg);
            }
        }

        Ok(())
    }

    /// Parse trade data into Trade struct
    fn parse_trade(data: TradeData) -> Result<Trade> {
        let price: f64 = data.px.parse().context("Failed to parse price")?;
        let size: f64 = data.sz.parse().context("Failed to parse size")?;
        let side = match data.side.as_str() {
            "B" => Side::Buy,
            "A" => Side::Sell,
            _ => anyhow::bail!("Unknown trade side: {}", data.side),
        };

        let timestamp = chrono::DateTime::from_timestamp_millis(data.time as i64)
            .context("Failed to parse timestamp")?;

        Ok(Trade::new(data.coin, price, size, side, timestamp))
    }

    /// Start with automatic reconnection
    pub async fn start_with_reconnect(&self) -> Result<()> {
        let mut retry_count = 0;
        let max_retries = 10;
        let base_delay = Duration::from_secs(1);

        loop {
            match self.start().await {
                Ok(_) => {
                    info!("WebSocket connection established");
                    retry_count = 0;
                    // Wait for shutdown signal
                    let mut shutdown_rx = self.shutdown_tx.subscribe();
                    let _ = shutdown_rx.recv().await;
                    info!("WebSocket shutting down");
                    break;
                }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= max_retries {
                        error!("Max retries reached, giving up: {}", e);
                        return Err(e);
                    }

                    let delay = base_delay * 2u32.pow(retry_count - 1);
                    warn!(
                        "WebSocket connection failed (retry {}/{}): {}. Retrying in {:?}",
                        retry_count, max_retries, e, delay
                    );
                    sleep(delay).await;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_trade_buy() {
        let data = TradeData {
            coin: "BTC".to_string(),
            side: "B".to_string(),
            px: "50000.0".to_string(),
            sz: "1.5".to_string(),
            time: 1700000000000,
        };

        let trade = HyperliquidWs::parse_trade(data).unwrap();
        assert_eq!(trade.symbol, "BTC");
        assert_eq!(trade.price, 50000.0);
        assert_eq!(trade.size, 1.5);
        assert_eq!(trade.side, Side::Buy);
    }

    #[test]
    fn test_parse_trade_sell() {
        let data = TradeData {
            coin: "ETH".to_string(),
            side: "A".to_string(),
            px: "3000.0".to_string(),
            sz: "10.0".to_string(),
            time: 1700000000000,
        };

        let trade = HyperliquidWs::parse_trade(data).unwrap();
        assert_eq!(trade.symbol, "ETH");
        assert_eq!(trade.price, 3000.0);
        assert_eq!(trade.size, 10.0);
        assert_eq!(trade.side, Side::Sell);
    }

    #[test]
    fn test_parse_trade_invalid_price() {
        let data = TradeData {
            coin: "BTC".to_string(),
            side: "B".to_string(),
            px: "invalid".to_string(),
            sz: "1.0".to_string(),
            time: 1700000000000,
        };

        assert!(HyperliquidWs::parse_trade(data).is_err());
    }
}
