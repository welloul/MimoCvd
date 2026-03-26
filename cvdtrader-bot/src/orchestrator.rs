use crate::health::HealthServer;
use anyhow::{Context, Result};
use cvdtrader_core::{Config, ExitReason, GlobalState, Trade, TradeHistory, TradeRecord};
use cvdtrader_execution::{ExecutionGateway, FillTracker, OrderTtlTracker};
use cvdtrader_market_data::{CandleBuilder, HyperliquidWs};
use cvdtrader_risk::{CircuitBreaker, RiskManager};
use cvdtrader_strategy::CvdPocStrategy;
use serde_json::json;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use tokio::signal;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};

/// Main bot orchestrator
pub struct Bot {
    /// Configuration
    config: Config,
    /// Global state
    state: GlobalState,
    /// Trade history
    trade_history: TradeHistory,
    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
}

impl Bot {
    /// Create a new bot
    pub fn new(config: Config) -> Result<Self> {
        let (shutdown_tx, _) = broadcast::channel(1);
        let state = GlobalState::new();

        // Initialize trade history database
        let db_path = Path::new("trades.db");
        let trade_history =
            TradeHistory::new(db_path).context("Failed to initialize trade history database")?;

        Ok(Self {
            config,
            state,
            trade_history,
            shutdown_tx,
        })
    }

    /// Start the bot
    pub async fn start(&self) -> Result<()> {
        info!("Starting CVDTrader bot");
        info!("Configuration: {:?}", self.config);

        // Initialize logging
        self.init_logging()?;

        // Create channels
        let (trade_tx, trade_rx) = mpsc::channel(self.config.bot.trade_buffer_size);
        let (candle_tx, candle_rx) = mpsc::channel(self.config.bot.candle_buffer_size);

        // Initialize components
        let ws = HyperliquidWs::new(
            self.config.exchange.ws_url.clone(),
            self.config.exchange.api_url.clone(),
            self.config.exchange.symbols.clone(),
            trade_tx,
            self.shutdown_tx.clone(),
        );

        let mut candle_builder = CandleBuilder::new(candle_tx, self.state.clone());

        let gateway = ExecutionGateway::new(
            self.config.exchange.api_url.clone(),
            self.config.execution.mode,
            self.state.clone(),
            self.config.execution.post_only,
        );

        let mut fill_tracker = FillTracker::new(self.state.clone());

        let ttl_tracker = OrderTtlTracker::new(
            self.state.clone(),
            self.config.execution.ttl_seconds,
            self.config.execution.ttl_check_interval_secs,
            self.shutdown_tx.clone(),
        );

        let risk_manager = RiskManager::new(
            self.state.clone(),
            self.config.risk.max_position_usd,
            self.config.risk.max_leverage,
            self.config.risk.max_drawdown_pct,
            self.config.risk.account_balance,
            self.config.execution.mode,
        );

        let circuit_breaker = CircuitBreaker::new(
            self.config.risk.circuit_breaker_latency_ms,
            self.config.risk.circuit_breaker_failures,
            self.shutdown_tx.clone(),
        );

        // Fetch metadata from exchange
        info!("Fetching metadata from exchange");
        let tick_sizes = ws.fetch_metadata().await?;
        info!("Loaded tick sizes for {} symbols", tick_sizes.len());

        // Set tick sizes for POC calculation
        candle_builder.set_tick_sizes(&tick_sizes);

        // Create strategy with tick sizes from exchange
        let mut strategy = CvdPocStrategy::new(
            self.state.clone(),
            self.config.strategy.lookback,
            self.config.strategy.cvd_exhaustion_ratio,
            self.config.strategy.cvd_absorption_pctile,
            self.config.strategy.sl_offset,
            self.config.strategy.risk_r_multiple,
            self.config.strategy.entry_offset_pct,
            tick_sizes,
            self.config.risk.max_position_usd,
        );

        // Start components
        info!("Starting WebSocket connection");
        // Spawn WebSocket reconnection as background task (non-blocking)
        let ws_handle = tokio::spawn(async move {
            if let Err(e) = ws.start_with_reconnect().await {
                error!("WebSocket reconnection failed: {}", e);
            }
        });

        info!("Starting order TTL tracker");
        ttl_tracker.start().await;

        info!("Starting fill tracker");
        let fill_tracker_handle = tokio::spawn(async move {
            fill_tracker.start().await;
        });

        // Start health check server
        let health_server = HealthServer::new(
            self.config.bot.health_check_port,
            self.state.clone(),
            self.shutdown_tx.clone(),
        );
        health_server.start().await?;

        // Set bot as running
        self.state.set_running(true).await;
        info!("Bot is now running");

        // Main event loop
        self.run_event_loop(
            trade_rx,
            candle_rx,
            &mut candle_builder,
            &mut strategy,
            &gateway,
            &risk_manager,
            &circuit_breaker,
        )
        .await?;

        // Shutdown
        info!("Shutting down bot");
        self.state.set_running(false).await;

        // Wait for background tasks to finish
        let _ = fill_tracker_handle.await;
        let _ = ws_handle.await;

        info!("Bot stopped");
        Ok(())
    }

    /// Main event loop
    async fn run_event_loop(
        &self,
        mut trade_rx: mpsc::Receiver<Trade>,
        mut candle_rx: mpsc::Receiver<cvdtrader_core::Candle>,
        candle_builder: &mut CandleBuilder,
        strategy: &mut CvdPocStrategy,
        gateway: &ExecutionGateway,
        risk_manager: &RiskManager,
        circuit_breaker: &CircuitBreaker,
    ) -> Result<()> {
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                // Process trades
                Some(trade) = trade_rx.recv() => {
                    let start = std::time::Instant::now();

                    // Process trade through candle builder
                    if let Some(completed_candle) = candle_builder.process_trade(&trade).await {
                        // Add completed candle to state BEFORE strategy processing
                        self.state.add_candle(completed_candle.symbol.clone(), completed_candle.clone()).await;

                        // Process completed candle through strategy
                        if let Some(signal) = strategy.process_candle(&completed_candle).await {
                            if signal.is_valid() {
                                // Validate signal with risk manager
                                match risk_manager.validate_signal(&signal).await {
                                    Ok(()) => {
                                        // Place order
                                        if let Err(e) = gateway.place_order(&signal).await {
                                            error!("Failed to place order: {}", e);
                                            circuit_breaker.record_failure().await;
                                        } else {
                                            circuit_breaker.record_success().await;
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Signal rejected by risk manager: {}", e);
                                    }
                                }
                            }
                        }

                        // Check exit conditions for existing positions
                        if let Some(exit_reason) = strategy.check_exit(&completed_candle.symbol, completed_candle.close).await {
                            info!("Exit signal for {}: {:?}", completed_candle.symbol, exit_reason);

                            // Get position before closing for trade recording
                            if let Some(position) = self.state.get_position(&completed_candle.symbol).await {
                                let exit_price = completed_candle.close;

                                // Close the position
                                if let Err(e) = gateway.close_position(&completed_candle.symbol).await {
                                    error!("Failed to close position: {}", e);
                                    circuit_breaker.record_failure().await;
                                } else {
                                    // Record the completed trade
                                    let trade_record = TradeRecord::new(&position, exit_price, exit_reason);
                                    if let Err(e) = self.trade_history.record_trade(&trade_record) {
                                        error!("Failed to record trade: {}", e);
                                    } else {
                                        info!(
                                            "Trade recorded: {} {} @ {} -> {} (PnL: {:.2}, {:.2}%)",
                                            trade_record.symbol,
                                            trade_record.side,
                                            trade_record.entry_price,
                                            trade_record.exit_price,
                                            trade_record.pnl,
                                            trade_record.pnl_pct
                                        );
                                    }
                                    circuit_breaker.record_success().await;
                                }
                            }
                        }
                    }

                    // Record latency
                    let latency = start.elapsed().as_millis() as u64;
                    circuit_breaker.record_latency(latency).await;
                }

                // Process completed candles
                Some(candle) = candle_rx.recv() => {
                    // Export candle to JSON file
                    self.export_candle_to_json(&candle);

                    // Log candle completion with CVD and POC
                    info!(
                        "Candle completed: {} O={} H={} L={} C={} V={} CVD={} POC={:?}",
                        candle.symbol,
                        candle.open,
                        candle.high,
                        candle.low,
                        candle.close,
                        candle.volume,
                        candle.cvd,
                        candle.poc
                    );

                    // Note: Candle already added to state in trade processing branch
                }

                // Check for shutdown
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received");
                    break;
                }

                // Check for Ctrl+C
                _ = signal::ctrl_c() => {
                    info!("Ctrl+C received, shutting down");
                    break;
                }
            }

            // Check if circuit breaker is tripped
            if circuit_breaker.is_tripped().await {
                error!("Circuit breaker tripped, shutting down");
                break;
            }
        }

        Ok(())
    }

    /// Initialize logging
    fn init_logging(&self) -> Result<()> {
        use tracing_subscriber::{fmt, EnvFilter};

        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&self.config.logging.level));

        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_target(true)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true);

        match self.config.logging.format.as_str() {
            "json" => {
                subscriber.json().init();
            }
            "pretty" => {
                subscriber.pretty().init();
            }
            _ => {
                subscriber.compact().init();
            }
        }

        Ok(())
    }

    /// Get global state
    pub fn state(&self) -> &GlobalState {
        &self.state
    }

    /// Get configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Export candle data to JSON file
    fn export_candle_to_json(&self, candle: &cvdtrader_core::Candle) {
        let candle_data = json!({
            "symbol": candle.symbol,
            "timestamp": candle.timestamp.to_rfc3339(),
            "open": candle.open,
            "high": candle.high,
            "low": candle.low,
            "close": candle.close,
            "volume": candle.volume,
            "cvd": candle.cvd,
            "poc": candle.poc,
            "trade_count": candle.trades.len()
        });

        let json_line = serde_json::to_string(&candle_data).unwrap_or_default();

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open("candles.json")
        {
            let _ = writeln!(file, "{}", json_line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bot_new() {
        let config = Config::default();
        let bot = Bot::new(config).unwrap();
        assert!(!bot.state().is_running().await);
    }
}
