//! Trade history persistence using SQLite
//!
//! Records all completed trades with entry/exit times, PnL, and exit reasons.

use crate::types::{ExitReason, PositionSide, SetupType, TradeRecord};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::Path;
use tracing::{debug, info};

/// Trade history manager for persisting completed trades
pub struct TradeHistory {
    conn: Connection,
}

impl TradeHistory {
    /// Create a new trade history manager with SQLite database
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path).context("Failed to open trade history database")?;

        // Create tables if they don't exist
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS trades (
                id TEXT PRIMARY KEY,
                symbol TEXT NOT NULL,
                side TEXT NOT NULL,
                size REAL NOT NULL,
                entry_price REAL NOT NULL,
                exit_price REAL NOT NULL,
                entry_time TEXT NOT NULL,
                exit_time TEXT NOT NULL,
                pnl REAL NOT NULL,
                pnl_pct REAL NOT NULL,
                exit_reason TEXT NOT NULL,
                setup_type TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_trades_symbol ON trades(symbol);
            CREATE INDEX IF NOT EXISTS idx_trades_exit_time ON trades(exit_time);
            CREATE INDEX IF NOT EXISTS idx_trades_pnl ON trades(pnl);",
        )
        .context("Failed to create trade history tables")?;

        info!("Trade history database initialized at {:?}", db_path);

        Ok(Self { conn })
    }

    /// Record a completed trade
    pub fn record_trade(&self, trade: &TradeRecord) -> Result<()> {
        let side_str = match trade.side {
            PositionSide::Long => "LONG",
            PositionSide::Short => "SHORT",
        };

        let exit_reason_str = match trade.exit_reason {
            ExitReason::StopLoss => "STOP_LOSS",
            ExitReason::TakeProfit => "TAKE_PROFIT",
            ExitReason::CvdFlip => "CVD_FLIP",
            ExitReason::Manual => "MANUAL",
        };

        let setup_type_str = trade.setup_type.map(|s| match s {
            SetupType::Exhaustion => "EXHAUSTION",
            SetupType::Absorption => "ABSORPTION",
        });

        self.conn.execute(
            "INSERT INTO trades (id, symbol, side, size, entry_price, exit_price, entry_time, exit_time, pnl, pnl_pct, exit_reason, setup_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                trade.id,
                trade.symbol,
                side_str,
                trade.size,
                trade.entry_price,
                trade.exit_price,
                trade.entry_time.to_rfc3339(),
                trade.exit_time.to_rfc3339(),
                trade.pnl,
                trade.pnl_pct,
                exit_reason_str,
                setup_type_str,
            ],
        )
        .context("Failed to insert trade record")?;

        debug!(
            "Recorded trade: {} {} {} @ {} -> {} (PnL: {:.2}%)",
            trade.symbol, side_str, trade.size, trade.entry_price, trade.exit_price, trade.pnl_pct
        );

        Ok(())
    }

    /// Get all trades for a symbol
    pub fn get_trades_for_symbol(&self, symbol: &str) -> Result<Vec<TradeRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, symbol, side, size, entry_price, exit_price, entry_time, exit_time, pnl, pnl_pct, exit_reason, setup_type
             FROM trades WHERE symbol = ?1 ORDER BY exit_time DESC",
        )?;

        let trades = stmt
            .query_map([symbol], |row| {
                let side_str: String = row.get(2)?;
                let exit_reason_str: String = row.get(10)?;
                let setup_type_str: Option<String> = row.get(11)?;

                let side = match side_str.as_str() {
                    "LONG" => PositionSide::Long,
                    "SHORT" => PositionSide::Short,
                    _ => PositionSide::Long,
                };

                let exit_reason = match exit_reason_str.as_str() {
                    "STOP_LOSS" => ExitReason::StopLoss,
                    "TAKE_PROFIT" => ExitReason::TakeProfit,
                    "CVD_FLIP" => ExitReason::CvdFlip,
                    _ => ExitReason::Manual,
                };

                let setup_type = setup_type_str.and_then(|s| match s.as_str() {
                    "EXHAUSTION" => Some(SetupType::Exhaustion),
                    "ABSORPTION" => Some(SetupType::Absorption),
                    _ => None,
                });

                let entry_time_str: String = row.get(6)?;
                let exit_time_str: String = row.get(7)?;

                Ok(TradeRecord {
                    id: row.get(0)?,
                    symbol: row.get(1)?,
                    side,
                    size: row.get(3)?,
                    entry_price: row.get(4)?,
                    exit_price: row.get(5)?,
                    entry_time: DateTime::parse_from_rfc3339(&entry_time_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    exit_time: DateTime::parse_from_rfc3339(&exit_time_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    pnl: row.get(8)?,
                    pnl_pct: row.get(9)?,
                    exit_reason,
                    setup_type,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(trades)
    }

    /// Get all trades within a time range
    pub fn get_trades_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TradeRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, symbol, side, size, entry_price, exit_price, entry_time, exit_time, pnl, pnl_pct, exit_reason, setup_type
             FROM trades WHERE exit_time BETWEEN ?1 AND ?2 ORDER BY exit_time DESC",
        )?;

        let trades = stmt
            .query_map(params![start.to_rfc3339(), end.to_rfc3339()], |row| {
                let side_str: String = row.get(2)?;
                let exit_reason_str: String = row.get(10)?;
                let setup_type_str: Option<String> = row.get(11)?;

                let side = match side_str.as_str() {
                    "LONG" => PositionSide::Long,
                    "SHORT" => PositionSide::Short,
                    _ => PositionSide::Long,
                };

                let exit_reason = match exit_reason_str.as_str() {
                    "STOP_LOSS" => ExitReason::StopLoss,
                    "TAKE_PROFIT" => ExitReason::TakeProfit,
                    "CVD_FLIP" => ExitReason::CvdFlip,
                    _ => ExitReason::Manual,
                };

                let setup_type = setup_type_str.and_then(|s| match s.as_str() {
                    "EXHAUSTION" => Some(SetupType::Exhaustion),
                    "ABSORPTION" => Some(SetupType::Absorption),
                    _ => None,
                });

                let entry_time_str: String = row.get(6)?;
                let exit_time_str: String = row.get(7)?;

                Ok(TradeRecord {
                    id: row.get(0)?,
                    symbol: row.get(1)?,
                    side,
                    size: row.get(3)?,
                    entry_price: row.get(4)?,
                    exit_price: row.get(5)?,
                    entry_time: DateTime::parse_from_rfc3339(&entry_time_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    exit_time: DateTime::parse_from_rfc3339(&exit_time_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    pnl: row.get(8)?,
                    pnl_pct: row.get(9)?,
                    exit_reason,
                    setup_type,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(trades)
    }

    /// Get trade statistics
    pub fn get_statistics(&self) -> Result<TradeStatistics> {
        let total_trades: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM trades", [], |row| row.get(0))?;

        let winning_trades: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM trades WHERE pnl > 0", [], |row| {
                    row.get(0)
                })?;

        let losing_trades: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM trades WHERE pnl < 0", [], |row| {
                    row.get(0)
                })?;

        let total_pnl: f64 =
            self.conn
                .query_row("SELECT COALESCE(SUM(pnl), 0) FROM trades", [], |row| {
                    row.get(0)
                })?;

        let avg_win: f64 = self.conn.query_row(
            "SELECT COALESCE(AVG(pnl), 0) FROM trades WHERE pnl > 0",
            [],
            |row| row.get(0),
        )?;

        let avg_loss: f64 = self.conn.query_row(
            "SELECT COALESCE(AVG(pnl), 0) FROM trades WHERE pnl < 0",
            [],
            |row| row.get(0),
        )?;

        let max_win: f64 =
            self.conn
                .query_row("SELECT COALESCE(MAX(pnl), 0) FROM trades", [], |row| {
                    row.get(0)
                })?;

        let max_loss: f64 =
            self.conn
                .query_row("SELECT COALESCE(MIN(pnl), 0) FROM trades", [], |row| {
                    row.get(0)
                })?;

        Ok(TradeStatistics {
            total_trades,
            winning_trades,
            losing_trades,
            win_rate: if total_trades > 0 {
                (winning_trades as f64 / total_trades as f64) * 100.0
            } else {
                0.0
            },
            total_pnl,
            avg_win,
            avg_loss,
            max_win,
            max_loss,
            profit_factor: if avg_loss != 0.0 {
                (avg_win * winning_trades as f64).abs() / (avg_loss * losing_trades as f64).abs()
            } else {
                0.0
            },
        })
    }

    /// Get total number of trades
    pub fn get_trade_count(&self) -> Result<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM trades", [], |row| row.get(0))?;
        Ok(count)
    }
}

/// Trade statistics summary
#[derive(Debug, Clone)]
pub struct TradeStatistics {
    pub total_trades: i64,
    pub winning_trades: i64,
    pub losing_trades: i64,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub max_win: f64,
    pub max_loss: f64,
    pub profit_factor: f64,
}

impl std::fmt::Display for TradeStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Trade Statistics ===")?;
        writeln!(f, "Total Trades: {}", self.total_trades)?;
        writeln!(f, "Winning Trades: {}", self.winning_trades)?;
        writeln!(f, "Losing Trades: {}", self.losing_trades)?;
        writeln!(f, "Win Rate: {:.2}%", self.win_rate)?;
        writeln!(f, "Total PnL: {:.2}", self.total_pnl)?;
        writeln!(f, "Average Win: {:.2}", self.avg_win)?;
        writeln!(f, "Average Loss: {:.2}", self.avg_loss)?;
        writeln!(f, "Max Win: {:.2}", self.max_win)?;
        writeln!(f, "Max Loss: {:.2}", self.max_loss)?;
        writeln!(f, "Profit Factor: {:.2}", self.profit_factor)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Position;
    use tempfile::tempdir;

    #[test]
    fn test_trade_history_record() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_trades.db");
        let history = TradeHistory::new(&db_path).unwrap();

        let position = Position::new(
            "BTC".to_string(),
            PositionSide::Long,
            1.0,
            50000.0,
            49000.0,
            52000.0,
        );

        let trade = TradeRecord::new(&position, 51000.0, ExitReason::TakeProfit);
        history.record_trade(&trade).unwrap();

        let count = history.get_trade_count().unwrap();
        assert_eq!(count, 1);

        let trades = history.get_trades_for_symbol("BTC").unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].pnl, 1000.0);
    }

    #[test]
    fn test_trade_statistics() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_stats.db");
        let history = TradeHistory::new(&db_path).unwrap();

        // Record winning trade
        let pos1 = Position::new(
            "BTC".to_string(),
            PositionSide::Long,
            1.0,
            50000.0,
            49000.0,
            52000.0,
        );
        let trade1 = TradeRecord::new(&pos1, 51000.0, ExitReason::TakeProfit);
        history.record_trade(&trade1).unwrap();

        // Record losing trade
        let pos2 = Position::new(
            "ETH".to_string(),
            PositionSide::Long,
            10.0,
            3000.0,
            2900.0,
            3200.0,
        );
        let trade2 = TradeRecord::new(&pos2, 2900.0, ExitReason::StopLoss);
        history.record_trade(&trade2).unwrap();

        let stats = history.get_statistics().unwrap();
        assert_eq!(stats.total_trades, 2);
        assert_eq!(stats.winning_trades, 1);
        assert_eq!(stats.losing_trades, 1);
        assert_eq!(stats.win_rate, 50.0);
    }
}
