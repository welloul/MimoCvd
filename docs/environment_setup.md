# CVDTrader Environment Setup Guide

## Prerequisites

### Software Requirements
- **Rust**: 1.75 or newer
- **Cargo**: Comes with Rust installation
- **Git**: For version control
- **OpenSSL**: Required for TLS connections (usually pre-installed on macOS/Linux)

### System Requirements
- **Memory**: Minimum 512MB RAM (1GB+ recommended)
- **Storage**: Minimum 2GB free space
- **Network**: Stable internet connection for exchange connectivity
- **CPU**: Modern CPU with good single-thread performance

## Installation

### 1. Install Rust
If you don't have Rust installed, use rustup:

```bash
# macOS/Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows
# Download and run rustup-init.exe from https://www.rust-lang.org/tools/install
```

Verify installation:
```bash
rustc --version
cargo --version
```

### 2. Clone the Repository
```bash
git clone https://github.com/yourusername/cvdtrader.git
cd cvdtrader
```

### 3. Build the Project
```bash
# Debug build (faster iteration)
cargo build

# Release build (optimized for performance)
cargo build --release
```

## Configuration

### 1. Create Configuration File
Copy the example configuration (if provided) or create `config.toml`:

```toml
[exchange]
# WebSocket and API URLs for Hyperliquid
ws_url = "wss://api.hyperliquid.xyz/ws"
api_url = "https://api.hyperliquid.xyz"
# Trading pairs to monitor
symbols = ["BTC", "ETH", "SOL"]

[strategy]
# Strategy parameters
lookback = 20
cvd_exhaustion_ratio = 0.70
cvd_absorption_pctile = 0.90
sl_offset = 2
risk_r_multiple = 1.5
entry_offset_pct = 0.001  # 0.1% offset from POC

[risk]
# Risk management parameters
max_position_usd = 1000.0
max_leverage = 10.0
max_drawdown_pct = 0.05  # 5%
circuit_breaker_latency_ms = 500
circuit_breaker_failures = 3

[execution]
# Order execution settings
mode = "dryrun"  # Options: dryrun, testnet, live
ttl_seconds = 120
post_only = true

[logging]
# Logging configuration
level = "info"   # Options: trace, debug, info, warn, error
format = "json"  # Options: json, pretty
```

### 2. Configuration Options Explained

#### Exchange Section
- `ws_url`: WebSocket URL for real-time trade data
- `api_url`: REST API URL for order placement
- `symbols`: Array of trading pairs to monitor (e.g., ["BTC", "ETH"])

#### Strategy Section
- `lookback`: Number of previous candles to consider for swing detection
- `cvd_exhaustion_ratio`: Threshold for exhaustion setup (0.0-1.0)
- `cvd_absorption_pctile`: Percentile threshold for absorption setup (0.0-1.0)
- `sl_offset`: Stop loss offset in ticks from candle wick
- `risk_r_multiple`: Reward-to-risk ratio for take profit calculation
- `entry_offset_pct`: Percentage offset from POC for order entry (0.0-1.0)

#### Risk Section
- `max_position_usd`: Maximum position value in USD
- `max_leverage`: Maximum leverage allowed (total exposure / account balance)
- `max_drawdown_pct`: Maximum drawdown percentage before trading halt
- `circuit_breaker_latency_ms`: Maximum latency before circuit breaker trips
- `circuit_breaker_failures`: Maximum consecutive failures before circuit breaker trips

#### Execution Section
- `mode`: Execution mode - "dryrun" (simulation), "testnet", or "live"
- `ttl_seconds`: Time-to-live for orders before automatic cancellation
- `post_only`: Whether to use post-only (maker-only) orders

#### Logging Section
- `level`: Log verbosity level
- `format`: Output format - "json" for structured logging, "pretty" for human-readable

## Running the Bot

### 1. Dry Run Mode (Recommended for Testing)
```bash
# Default mode is dryrun
cargo run --release

# Or explicitly specify
EXECUTION_MODE=dryrun cargo run --release
```

### 2. Testnet Mode
```bash
EXECUTION_MODE=testnet cargo run --release
```
*Note: You'll need testnet API credentials configured in your environment or config*

### 3. Live Mode
```bash
EXECUTION_MODE=live cargo run --release
```
*⚠️ WARNING: Live mode uses real funds. Ensure you understand the risks and have tested thoroughly in dryrun/testnet modes first.*

## Environment Variables

The bot supports environment variable overrides for configuration:

```bash
# Exchange settings
CVDTRADER_EXCHANGE_WS_URL=wss://api.hyperliquid.xyz/ws
CVDTRADER_EXCHANGE_API_URL=https://api.hyperliquid.xyz
CVDTRADER_EXCHANGE_SYMBOLS='["BTC", "ETH"]'

# Strategy settings
CVDTRADER_STRATEGY_LOOKBACK=20
CVDTRADER_STRATEGY_CVD_EXHAUSTION_RATIO=0.70
CVDTRADER_STRATEGY_CVD_ABSORPTION_PCTILE=0.90

# Risk settings
CVDTRADER_RISK_MAX_POSITION_USD=1000.0
CVDTRADER_RISK_MAX_LEVERAGE=10.0
CVDTRADER_RISK_MAX_DRAWDOWN_PCT=0.05

# Execution settings
CVDTRADER_EXECUTION_MODE=dryrun
CVDTRADER_EXECUTION_TTL_SECONDS=120
CVDTRADER_EXECUTION_POST_ONLY=true

# Logging settings
CVDTRADER_LOGGING_LEVEL=info
CVDTRADER_LOGGING_FORMAT=json

# Standard logging override
RUST_LOG=info
```

## Testing

### Run All Tests
```bash
cargo test
```

### Run Tests for Specific Module
```bash
cargo test -p cvdtrader-core
cargo test -p cvdtrader-market-data
cargo test -p cvdtrader-strategy
cargo test -p cvdtrader-execution
cargo test -p cvdtrader-risk
cargo test -p cvdtrader-bot
```

### Run Tests in Release Mode (More Realistic)
```bash
cargo test --release
```

## Deployment

### Production Deployment Considerations

#### 1. System Preparation
- Ensure system clock is synchronized (NTP recommended)
- Install security updates
- Create dedicated user for running the bot
- Set up monitoring and logging

#### 2. Service Management (systemd Example)
Create `/etc/systemd/system/cvdtrader.service`:

```ini
[Unit]
Description=CVDTrader Bot
After=network.target

[Service]
Type=simple
User=cvdtrader
WorkingDirectory=/opt/cvdtrader
ExecStart=/opt/cvdtrader/target/release/cvdtrader
Environment=EXECUTION_MODE=live
Environment=RUST_LOG=info
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

Then enable and start:
```bash
sudo systemctl daemon-reload
sudo systemctl enable cvdtrader
sudo systemctl start cvdtrader
sudo systemctl status cvdtrader
```

#### 3. Logging in Production
For JSON logging in production:
```bash
# View logs with journalctl
journalctl -u cvdtrader -f

# Or redirect to file
ExecStart=/opt/cvdtrader/target/release/cvdtrader > /var/log/cvdtrader.log 2>&1
```

## Troubleshooting

### Common Issues

#### 1. Compilation Errors
- **Symptom**: `cargo build` fails with dependency errors
- **Solution**: 
  - Update Rust toolchain: `rustup update`
  - Clean build artifacts: `cargo clean`
  - Retry build

#### 2. Connection Issues
- **Symptom**: Bot fails to connect to exchange
- **Solution**:
  - Check network connectivity
  - Verify WebSocket and API URLs in config
  - Check firewall settings
  - Test with `wscat` or `curl` for basic connectivity

#### 3. Performance Issues
- **Symptom**: High latency or missed trades
- **Solution**:
  - Check system resources (top/htop)
  - Verify CPU frequency scaling is disabled
  - Consider running on dedicated core
  - Check for background processes consuming resources

#### 4. Configuration Errors
- **Symptom**: Bot fails to start with configuration errors
- **Solution**:
  - Verify TOML syntax with online validator
  - Check that all required sections are present
  - Ensure values are within valid ranges
  - Check data types (strings vs numbers vs booleans)

### Debugging

#### Enable Verbose Logging
```bash
RUST_LOG=debug cargo run
```

#### Log to File
```bash
RUST_LOG=info LOG_FORMAT=pretty cargo run > bot.log 2>&1
```

#### Monitor Metrics
The bot outputs periodic metrics through logging when in debug mode.

## Maintenance

### Regular Tasks
1. **Log Rotation**: Implement log rotation strategy for production deployments
2. **Dependency Updates**: Periodically run `cargo update` to get latest secure dependencies
3. **Security Audits**: Use `cargo audit` to check for known vulnerabilities
4. **Performance Monitoring**: Monitor CPU, memory, and latency metrics
5. **Backup Configuration**: Keep secure backups of config.toml (excluding secrets)

### Updating the Bot
```bash
# Pull latest changes
git pull origin main

# Rebuild
cargo build --release

# Restart service (if using systemd)
sudo systemctl restart cvdtrader
```

## Support

For issues and questions:
1. Check the [issues.md](issues.md) file for known problems
2. Review the [README.md](README.md) for general information
3. Consult the module-specific documentation in this `/docs` directory
4. Check the test cases in each module for usage examples