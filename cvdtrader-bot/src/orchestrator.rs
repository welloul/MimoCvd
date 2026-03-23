use anyhow::{Context, Result};
use cvdtrader_core::{Config, GlobalState, Trade};
use cvdtrader_execution::{ExecutionGateway, FillTracker, OrderTtlTracker};
use cvdtrader_market_data::{CandleBuilder, HyperliquidWs};
use cvdtrader_risk::{CircuitBreaker, RiskManager};
use cvdtrader_strategy::CvdPocStrategy;
use tokio::signal;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};

/// Main bot orchestrator
pub struct Bot {
    /// Configuration
    config: Config,
    /// Global state
    state: GlobalState,
    /// Shutdown signal
    shutdown_tx: broadcast::Sender<()>,
}

impl Bot {
    /// Create a new bot
    pub fn new(config: Config) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        let state = GlobalState::new();

        Self {
            config,
            state,
            shutdown_tx,
        }
    }

    /// Start the bot
    pub async fn start(&self) -> Result<()> {
        info!("Starting CVDTrader bot");
        info!("Configuration: {:?}", self.config);

        // Initialize logging
        self.init_logging()?;

        // Create channels
        let (trade_tx, trade_rx) = mpsc::channel(1000);
        let (candle_tx, candle_rx) = mpsc::channel(100);

        // Initialize components
        let ws = HyperliquidWs::new(
            self.config.exchange.ws_url.clone(),
            self.config.exchange.symbols.clone(),
            trade_tx,
            self.shutdown_tx.clone(),
        );

        let mut candle_builder = CandleBuilder::new(candle_tx, self.state.clone());

        let mut strategy = CvdPocStrategy::new(
            self.state.clone(),
            self.config.strategy.lookback,
            self.config.strategy.cvd_exhaustion_ratio,
            self.config.strategy.cvd_absorption_pctile,
            self.config.strategy.sl_offset,
            self.config.strategy.risk_r_multiple,
            self.config.strategy.entry_offset_pct,
            self.config.tick_size(&self.config.exchange.symbols[0]),
            self.config.risk.max_position_usd,
        );

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
            self.shutdown_tx.clone(),
        );

        let risk_manager = RiskManager::new(
            self.state.clone(),
            self.config.risk.max_position_usd,
            self.config.risk.max_leverage,
            self.config.risk.max_drawdown_pct,
            10000.0, // TODO: Get from config
        );

        let circuit_breaker = CircuitBreaker::new(
            self.config.risk.circuit_breaker_latency_ms,
            self.config.risk.circuit_breaker_failures,
            self.shutdown_tx.clone(),
        );

        // Start components
        info!("Starting WebSocket connection");
        ws.start_with_reconnect().await?;

        info!("Starting order TTL tracker");
        ttl_tracker.start().await;

        info!("Starting fill tracker");
        let fill_tracker_handle = tokio::spawn(async move {
            fill_tracker.start().await;
        });

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

        // Wait for fill tracker to finish
        fill_tracker_handle.await?;

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
                            if let Err(e) = gateway.close_position(&completed_candle.symbol).await {
                                error!("Failed to close position: {}", e);
                                circuit_breaker.record_failure().await;
                            } else {
                                circuit_breaker.record_success().await;
                            }
                        }
                    }

                    // Record latency
                    let latency = start.elapsed().as_millis() as u64;
                    circuit_breaker.record_latency(latency).await;
                }

                // Process completed candles
                Some(candle) = candle_rx.recv() => {
                    // Update indicators
                    // TODO: Update indicators with completed candle
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bot_new() {
        let config = Config::default();
        let bot = Bot::new(config);
        assert!(!bot.state().is_running().try_into().unwrap_or(false));
    }
}
