# CVDTrader Trades Database Documentation

## Overview

The CVDTrader bot automatically records all completed trades to a SQLite database file (`trades.db`). This enables post-trade analysis, performance tracking, and historical record keeping.

## Database Location

- **Default Path**: `trades.db` in the working directory
- **Created**: Automatically on bot startup
- **Format**: SQLite 3

## Database Schema

```sql
CREATE TABLE trades (
    id TEXT PRIMARY KEY,           -- Unique trade ID (UUID)
    symbol TEXT NOT NULL,          -- Trading pair (e.g., "BTC", "ETH")
    side TEXT NOT NULL,            -- Position side: "LONG" or "SHORT"
    size REAL NOT NULL,            -- Position size
    entry_price REAL NOT NULL,     -- Entry price
    exit_price REAL NOT NULL,      -- Exit price
    entry_time TEXT NOT NULL,      -- Entry timestamp (RFC 3339 format)
    exit_time TEXT NOT NULL,       -- Exit timestamp (RFC 3339 format)
    pnl REAL NOT NULL,             -- Realized PnL in USD
    pnl_pct REAL NOT NULL,         -- PnL percentage
    exit_reason TEXT NOT NULL,     -- Exit reason (see below)
    setup_type TEXT                -- Setup type (optional)
);

-- Indexes for common queries
CREATE INDEX idx_trades_symbol ON trades(symbol);
CREATE INDEX idx_trades_exit_time ON trades(exit_time);
CREATE INDEX idx_trades_pnl ON trades(pnl);
```

### Exit Reasons

| Value | Description |
|-------|-------------|
| `STOP_LOSS` | Position closed at stop loss |
| `TAKE_PROFIT` | Position closed at take profit |
| `CVD_FLIP` | Position closed due to CVD flip streak |
| `MANUAL` | Position closed manually |

### Setup Types

| Value | Description |
|-------|-------------|
| `EXHAUSTION` | CVD exhaustion setup |
| `ABSORPTION` | CVD absorption setup |
| `NULL` | No setup type recorded |

## Querying the Database

### Quick Reference - Common Queries

```sql
-- Get all trades
SELECT * FROM trades;

-- Get winning trades
SELECT * FROM trades WHERE pnl > 0;

-- Get losing trades
SELECT * FROM trades WHERE pnl < 0;

-- Get statistics
SELECT 
    COUNT(*) as total,
    SUM(CASE WHEN pnl > 0 THEN 1 ELSE 0 END) as wins,
    AVG(pnl) as avg_pnl
FROM trades;

-- Get total PnL
SELECT SUM(pnl) as total_pnl FROM trades;

-- Get win rate
SELECT 
    ROUND(100.0 * SUM(CASE WHEN pnl > 0 THEN 1 ELSE 0 END) / COUNT(*), 2) as win_rate
FROM trades;

-- Get trades for a specific symbol
SELECT * FROM trades WHERE symbol = 'BTC';

-- Get trades from last 24 hours
SELECT * FROM trades 
WHERE exit_time >= datetime('now', '-1 day');

-- Get best trade
SELECT * FROM trades ORDER BY pnl DESC LIMIT 1;

-- Get worst trade
SELECT * FROM trades ORDER BY pnl ASC LIMIT 1;

-- Get stop loss trades
SELECT * FROM trades WHERE exit_reason = 'STOP_LOSS';

-- Get take profit trades
SELECT * FROM trades WHERE exit_reason = 'TAKE_PROFIT';

-- Get long trades only
SELECT * FROM trades WHERE side = 'LONG';

-- Get short trades only
SELECT * FROM trades WHERE side = 'SHORT';

-- Get average win
SELECT AVG(pnl) as avg_win FROM trades WHERE pnl > 0;

-- Get average loss
SELECT AVG(pnl) as avg_loss FROM trades WHERE pnl < 0;

-- Get profit factor
SELECT 
    ABS(SUM(CASE WHEN pnl > 0 THEN pnl ELSE 0 END)) /
    NULLIF(ABS(SUM(CASE WHEN pnl < 0 THEN pnl ELSE 0 END)), 0) as profit_factor
FROM trades;

-- Get trades by exit reason
SELECT exit_reason, COUNT(*) as count, AVG(pnl) as avg_pnl
FROM trades
GROUP BY exit_reason;

-- Get daily PnL
SELECT 
    DATE(exit_time) as date,
    SUM(pnl) as daily_pnl,
    COUNT(*) as trades
FROM trades
GROUP BY DATE(exit_time)
ORDER BY date DESC;
```

### Using SQLite Command Line

```bash
# Open the database
sqlite3 trades.db

# Set formatted output
.mode column
.headers on
```

### Common Queries

#### View All Trades
```sql
SELECT * FROM trades ORDER BY exit_time DESC;
```

#### Get Trade Count
```sql
SELECT COUNT(*) as total_trades FROM trades;
```

#### Get Winning vs Losing Trades
```sql
SELECT 
    COUNT(*) as total,
    SUM(CASE WHEN pnl > 0 THEN 1 ELSE 0 END) as wins,
    SUM(CASE WHEN pnl < 0 THEN 1 ELSE 0 END) as losses
FROM trades;
```

#### Calculate Win Rate
```sql
SELECT 
    ROUND(
        100.0 * SUM(CASE WHEN pnl > 0 THEN 1 ELSE 0 END) / COUNT(*),
        2
    ) as win_rate_pct
FROM trades;
```

#### Get Total PnL
```sql
SELECT 
    ROUND(SUM(pnl), 2) as total_pnl,
    ROUND(AVG(pnl), 2) as avg_pnl_per_trade
FROM trades;
```

#### Get Best and Worst Trades
```sql
-- Best trade
SELECT * FROM trades ORDER BY pnl DESC LIMIT 1;

-- Worst trade
SELECT * FROM trades ORDER BY pnl ASC LIMIT 1;
```

#### Get Profit Factor
```sql
SELECT 
    ROUND(
        ABS(SUM(CASE WHEN pnl > 0 THEN pnl ELSE 0 END)) /
        NULLIF(ABS(SUM(CASE WHEN pnl < 0 THEN pnl ELSE 0 END)), 0),
        2
    ) as profit_factor
FROM trades;
```

#### Filter by Symbol
```sql
-- All BTC trades
SELECT * FROM trades WHERE symbol = 'BTC' ORDER BY exit_time DESC;

-- PnL by symbol
SELECT 
    symbol,
    COUNT(*) as trades,
    ROUND(SUM(pnl), 2) as total_pnl,
    ROUND(AVG(pnl_pct), 2) as avg_pnl_pct
FROM trades 
GROUP BY symbol
ORDER BY total_pnl DESC;
```

#### Filter by Date Range
```sql
-- Trades from last 7 days
SELECT * FROM trades 
WHERE exit_time >= datetime('now', '-7 days')
ORDER BY exit_time DESC;

-- Trades from specific date
SELECT * FROM trades 
WHERE DATE(exit_time) = '2026-03-24'
ORDER BY exit_time DESC;
```

#### Filter by Exit Reason
```sql
-- Stop loss hits
SELECT * FROM trades WHERE exit_reason = 'STOP_LOSS';

-- Take profit hits
SELECT * FROM trades WHERE exit_reason = 'TAKE_PROFIT';

-- Count by exit reason
SELECT 
    exit_reason,
    COUNT(*) as count,
    ROUND(AVG(pnl), 2) as avg_pnl
FROM trades
GROUP BY exit_reason;
```

#### Filter by Setup Type
```sql
-- Exhaustion setups
SELECT * FROM trades WHERE setup_type = 'EXHAUSTION';

-- Absorption setups
SELECT * FROM trades WHERE setup_type = 'ABSORPTION';

-- Performance by setup type
SELECT 
    COALESCE(setup_type, 'NONE') as setup,
    COUNT(*) as trades,
    ROUND(SUM(pnl), 2) as total_pnl,
    ROUND(100.0 * SUM(CASE WHEN pnl > 0 THEN 1 ELSE 0 END) / COUNT(*), 2) as win_rate
FROM trades
GROUP BY setup_type;
```

#### Long vs Short Performance
```sql
SELECT 
    side,
    COUNT(*) as trades,
    ROUND(SUM(pnl), 2) as total_pnl,
    ROUND(AVG(pnl), 2) as avg_pnl,
    ROUND(100.0 * SUM(CASE WHEN pnl > 0 THEN 1 ELSE 0 END) / COUNT(*), 2) as win_rate
FROM trades
GROUP BY side;
```

#### Trade Duration Analysis
```sql
-- Average trade duration in minutes
SELECT 
    ROUND(AVG(
        (JULIANDAY(exit_time) - JULIANDAY(entry_time)) * 24 * 60
    ), 2) as avg_duration_minutes
FROM trades;

-- Duration by exit reason
SELECT 
    exit_reason,
    ROUND(AVG(
        (JULIANDAY(exit_time) - JULIANDAY(entry_time)) * 24 * 60
    ), 2) as avg_duration_minutes
FROM trades
GROUP BY exit_reason;
```

#### Daily Performance Summary
```sql
SELECT 
    DATE(exit_time) as date,
    COUNT(*) as trades,
    ROUND(SUM(pnl), 2) as daily_pnl,
    ROUND(100.0 * SUM(CASE WHEN pnl > 0 THEN 1 ELSE 0 END) / COUNT(*), 2) as win_rate
FROM trades
GROUP BY DATE(exit_time)
ORDER BY date DESC;
```

#### Cumulative PnL Over Time
```sql
SELECT 
    exit_time,
    symbol,
    pnl,
    ROUND(SUM(pnl) OVER (ORDER BY exit_time), 2) as cumulative_pnl
FROM trades
ORDER BY exit_time;
```

#### Drawdown Analysis
```sql
-- Find max drawdown (largest losing streak)
WITH cumulative AS (
    SELECT 
        exit_time,
        SUM(pnl) OVER (ORDER BY exit_time) as cum_pnl
    FROM trades
),
running_max AS (
    SELECT 
        exit_time,
        cum_pnl,
        MAX(cum_pnl) OVER (ORDER BY exit_time) as max_cum_pnl
    FROM cumulative
)
SELECT 
    MIN(cum_pnl - max_cum_pnl) as max_drawdown
FROM running_max;
```

## Exporting Data

### Export to CSV
```bash
sqlite3 -header -csv trades.db "SELECT * FROM trades;" > trades_export.csv
```

### Export to JSON
```bash
sqlite3 -json trades.db "SELECT * FROM trades;" > trades_export.json
```

### Export Specific Query
```bash
sqlite3 -header -csv trades.db "
    SELECT symbol, side, entry_price, exit_price, pnl, pnl_pct, exit_reason 
    FROM trades 
    WHERE pnl > 0 
    ORDER BY pnl DESC;
" > winning_trades.csv
```

## Programmatic Access (Rust)

The `TradeHistory` struct in `cvdtrader-core/src/history.rs` provides programmatic access:

```rust
use cvdtrader_core::{TradeHistory, TradeRecord};
use std::path::Path;

// Initialize database
let history = TradeHistory::new(Path::new("trades.db"))?;

// Record a trade
history.record_trade(&trade_record)?;

// Query trades for a symbol
let btc_trades = history.get_trades_for_symbol("BTC")?;

// Query trades in date range
let recent_trades = history.get_trades_in_range(start_time, end_time)?;

// Get statistics
let stats = history.get_statistics()?;
println!("{}", stats);

// Get trade count
let count = history.get_trade_count()?;
```

## Maintenance

### Database Backup
```bash
# Create backup
cp trades.db trades_backup_$(date +%Y%m%d).db

# Or use SQLite backup command
sqlite3 trades.db ".backup 'trades_backup.db'"
```

### Database Size
```bash
# Check database size
ls -lh trades.db

# Or from SQLite
sqlite3 trades.db "SELECT page_count * page_size as size_bytes FROM pragma_page_count(), pragma_page_size();"
```

### Vacuum Database
```bash
# Reclaim unused space
sqlite3 trades.db "VACUUM;"
```

### Check Integrity
```bash
sqlite3 trades.db "PRAGMA integrity_check;"
```

## Troubleshooting

### Database Locked
If you see "database is locked" errors:
- Ensure the bot is not running when querying
- Or use WAL mode: `sqlite3 trades.db "PRAGMA journal_mode=WAL;"`

### Corrupt Database
```bash
# Try to recover
sqlite3 trades.db ".recover" | sqlite3 trades_recovered.db
```

### Missing Trades
- Verify bot was running in dryrun, testnet, or live mode
- Check bot logs for "Trade recorded" messages
- Ensure positions were actually closed (not just opened)