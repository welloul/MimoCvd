//! Health check endpoint for monitoring bot status
//!
//! Provides an HTTP endpoint that exposes bot health information including
//! connection status, last message timestamp, and component status.

use axum::{routing::get, Json, Router};
use cvdtrader_core::GlobalState;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info};

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Overall bot status
    pub status: String,
    /// Whether the bot is running
    pub is_running: bool,
    /// Last update timestamp (Unix milliseconds)
    pub last_update_ms: i64,
    /// Number of active positions
    pub active_positions: usize,
    /// Number of pending orders
    pub pending_orders: usize,
    /// Number of subscribed symbols
    pub subscribed_symbols: usize,
    /// WebSocket connection status
    pub websocket_connected: bool,
}

/// Health check server
pub struct HealthServer {
    /// Port to listen on
    port: u16,
    /// Global state for reading bot status
    state: GlobalState,
    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
}

impl HealthServer {
    /// Create a new health server
    pub fn new(port: u16, state: GlobalState, shutdown_tx: broadcast::Sender<()>) -> Self {
        Self {
            port,
            state,
            shutdown_tx,
        }
    }

    /// Start the health check server
    pub async fn start(&self) -> anyhow::Result<()> {
        if self.port == 0 {
            info!("Health check endpoint disabled (port = 0)");
            return Ok(());
        }

        let state = self.state.clone();
        let shutdown_tx = self.shutdown_tx.clone();

        let app = Router::new().route("/health", get(move || Self::health_handler(state.clone())));

        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        info!("Starting health check server on {}", addr);

        let mut shutdown_rx = shutdown_tx.subscribe();

        tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(listener) => listener,
                Err(e) => {
                    error!("Failed to bind health check server: {}", e);
                    return;
                }
            };

            let server = axum::serve(listener, app);

            tokio::select! {
                result = server => {
                    if let Err(e) = result {
                        error!("Health check server error: {}", e);
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Health check server shutting down");
                }
            }
        });

        Ok(())
    }

    /// Health check handler
    async fn health_handler(state: GlobalState) -> Json<HealthResponse> {
        let is_running = state.is_running().await;
        let last_update = state.last_update().await;
        let positions = state.get_all_positions().await;
        let orders = state.get_all_orders().await;

        let pending_orders = orders
            .values()
            .filter(|o| o.status == cvdtrader_core::OrderStatus::Pending)
            .count();

        Json(HealthResponse {
            status: if is_running { "healthy" } else { "stopped" }.to_string(),
            is_running,
            last_update_ms: last_update.timestamp_millis(),
            active_positions: positions.len(),
            pending_orders,
            subscribed_symbols: 0,           // Will be updated by WebSocket
            websocket_connected: is_running, // Simplified - actual check would need WS state
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_response() {
        let state = GlobalState::new();
        state.set_running(true).await;

        let response = HealthServer::health_handler(state).await;
        assert_eq!(response.status, "healthy");
        assert!(response.is_running);
    }
}
