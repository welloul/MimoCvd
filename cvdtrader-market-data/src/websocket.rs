use anyhow::{Context, Result};
use cvdtrader_core::{Side, Trade};
use futures_util::{SinkExt, StreamExt};
use hyperliquid_rust_sdk::InfoClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// Trade data from WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeData {
    pub coin: String,
    pub side: String,
    pub px: String,
    pub sz: String,
    pub time: u64,
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

/// Metadata for a trading pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMetadata {
    pub name: String,
    pub sz_decimals: i32,
    pub px_decimals: i32,
    pub max_leverage: f64,
    pub only_isolated: bool,
}

/// Hyperliquid metadata response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataResponse {
    pub universe: Vec<SymbolMetadata>,
}

/// Hyperliquid WebSocket client
pub struct HyperliquidWs {
    url: String,
    api_url: String,
    symbols: Vec<String>,
    trade_tx: mpsc::Sender<Trade>,
    shutdown_tx: broadcast::Sender<()>,
    tick_sizes: Arc<RwLock<HashMap<String, f64>>>,
    subscribed_symbols: Arc<RwLock<HashMap<String, bool>>>,
}

impl HyperliquidWs {
    /// Create a new WebSocket client
    pub fn new(
        url: String,
        api_url: String,
        symbols: Vec<String>,
        trade_tx: mpsc::Sender<Trade>,
        shutdown_tx: broadcast::Sender<()>,
    ) -> Self {
        Self {
            url,
            api_url,
            symbols,
            trade_tx,
            shutdown_tx,
            tick_sizes: Arc::new(RwLock::new(HashMap::new())),
            subscribed_symbols: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Fetch metadata from exchange API using SDK
    pub async fn fetch_metadata(&self) -> Result<HashMap<String, f64>> {
        // Create InfoClient for metadata fetching
        let info_client = InfoClient::new(None, None)
            .await
            .context("Failed to create InfoClient")?;

        // Fetch metadata using SDK
        let metadata = info_client
            .meta()
            .await
            .context("Failed to fetch metadata from exchange")?;

        let mut tick_sizes = HashMap::new();

        for asset_meta in metadata.universe {
            // Calculate tick size from sz_decimals
            // sz_decimals = 0 -> tick_size = 1
            // sz_decimals = 1 -> tick_size = 0.1
            // sz_decimals = 2 -> tick_size = 0.01
            let tick_size = 10f64.powi(-(asset_meta.sz_decimals as i32));
            tick_sizes.insert(asset_meta.name.clone(), tick_size);

            info!(
                "Loaded metadata for {}: tick_size={}, sz_decimals={}",
                asset_meta.name, tick_size, asset_meta.sz_decimals
            );
        }

        // Update internal tick sizes
        *self.tick_sizes.write().await = tick_sizes.clone();

        Ok(tick_sizes)
    }

    /// Get tick size for a symbol
    pub async fn get_tick_size(&self, symbol: &str) -> Option<f64> {
        self.tick_sizes.read().await.get(symbol).copied()
    }

    /// Get all tick sizes
    pub async fn get_all_tick_sizes(&self) -> HashMap<String, f64> {
        self.tick_sizes.read().await.clone()
    }

    /// Start the WebSocket connection
    pub async fn start(&self) -> Result<()> {
        info!("Connecting to Hyperliquid WebSocket: {}", self.url);

        let (ws_stream, _) = connect_async(&self.url)
            .await
            .context("Failed to connect to WebSocket")?;

        info!("WebSocket connected successfully");

        let (mut write, mut read) = ws_stream.split();

        // Get symbols that need subscription (not already subscribed)
        let subscribed = self.subscribed_symbols.read().await;
        let symbols_to_subscribe: Vec<String> = self
            .symbols
            .iter()
            .filter(|symbol| !subscribed.contains_key(*symbol))
            .cloned()
            .collect();
        drop(subscribed);

        // Subscribe to trade data for symbols that need subscription
        for symbol in &symbols_to_subscribe {
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

            // Mark as subscribed
            self.subscribed_symbols
                .write()
                .await
                .insert(symbol.clone(), true);

            info!("Subscribed to trades for {}", symbol);
        }

        if symbols_to_subscribe.is_empty() {
            info!("All symbols already subscribed, skipping subscription");
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
        // Ignore pong messages (not JSON)
        if text.contains("pong") {
            return Ok(());
        }

        // Log all incoming messages for debugging
        if text.len() < 500 {
            debug!("Received WebSocket message: {}", text);
        } else {
            debug!(
                "Received WebSocket message (truncated): {}...",
                &text[..500]
            );
        }

        // Parse as generic JSON Value to handle Hyperliquid's actual message format
        let data: serde_json::Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(e) => {
                debug!("Failed to parse JSON: {}", e);
                return Ok(());
            }
        };

        // Extract channel and data from the message
        // Hyperliquid sends: { "channel": "trades", "data": [...] }
        if let (Some(channel), Some(trades_data)) = (
            data.get("channel").and_then(|c| c.as_str()),
            data.get("data").and_then(|d| d.as_array()),
        ) {
            debug!(
                "Received message on channel: {} with {} items",
                channel,
                trades_data.len()
            );

            if channel == "trades" {
                for trade_value in trades_data {
                    // Parse each trade from the JSON value
                    if let Ok(trade_data) = serde_json::from_value::<TradeData>(trade_value.clone())
                    {
                        match Self::parse_trade(trade_data) {
                            Ok(trade) => {
                                debug!(
                                    "Parsed trade: {} {} @ {} size {}",
                                    trade.symbol, trade.side, trade.price, trade.size
                                );
                                if let Err(e) = trade_tx.send(trade).await {
                                    error!("Failed to send trade: {}", e);
                                }
                            }
                            Err(e) => {
                                error!("Failed to parse trade: {}", e);
                            }
                        }
                    }
                }
            } else if channel == "error" {
                if let Some(error_msg) = data.get("data").and_then(|d| d.get("msg")) {
                    error!("WebSocket error: {}", error_msg);
                }
            }
        } else {
            debug!("Message does not have channel/data structure: {:?}", data);
        }
        // Ignore other message types (subscription confirmations, etc.)

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

                    // Calculate exponential backoff with jitter (0-50% of delay)
                    let base_delay_secs = base_delay.as_secs() * 2u64.pow(retry_count - 1);
                    // Simple jitter: use retry_count as a pseudo-random value
                    let jitter = (retry_count as u64 * 13) % (base_delay_secs / 2 + 1);
                    let delay = Duration::from_secs(base_delay_secs + jitter);

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
