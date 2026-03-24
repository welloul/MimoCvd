use anyhow::Result;
use cvdtrader_bot::Bot;
use cvdtrader_core::Config;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = Config::load("config.toml")?;

    // Create and start bot
    let bot = Bot::new(config)?;

    info!("Starting CVDTrader bot");
    match bot.start().await {
        Ok(()) => {
            info!("Bot stopped successfully");
            Ok(())
        }
        Err(e) => {
            error!("Bot failed: {}", e);
            Err(e)
        }
    }
}
