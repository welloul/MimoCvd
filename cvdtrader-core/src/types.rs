use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Trade side (aggressor)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Side::Buy => write!(f, "BUY"),
            Side::Sell => write!(f, "SELL"),
        }
    }
}

/// Order side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl fmt::Display for OrderSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderSide::Buy => write!(f, "BUY"),
            OrderSide::Sell => write!(f, "SELL"),
        }
    }
}

/// Position side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    Long,
    Short,
}

impl fmt::Display for PositionSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PositionSide::Long => write!(f, "LONG"),
            PositionSide::Short => write!(f, "SHORT"),
        }
    }
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Filled,
    PartiallyFilled,
    Cancelled,
    Rejected,
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderStatus::Pending => write!(f, "PENDING"),
            OrderStatus::Filled => write!(f, "FILLED"),
            OrderStatus::PartiallyFilled => write!(f, "PARTIALLY_FILLED"),
            OrderStatus::Cancelled => write!(f, "CANCELLED"),
            OrderStatus::Rejected => write!(f, "REJECTED"),
        }
    }
}

/// Execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    DryRun,
    TestNet,
    Live,
}

impl fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionMode::DryRun => write!(f, "DRYRUN"),
            ExecutionMode::TestNet => write!(f, "TESTNET"),
            ExecutionMode::Live => write!(f, "LIVE"),
        }
    }
}

/// Trade data from exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub symbol: String,
    pub price: f64,
    pub size: f64,
    pub side: Side,
    pub timestamp: DateTime<Utc>,
}

impl Trade {
    pub fn new(
        symbol: String,
        price: f64,
        size: f64,
        side: Side,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            symbol,
            price,
            size,
            side,
            timestamp,
        }
    }

    /// Calculate delta (positive for buy, negative for sell)
    pub fn delta(&self) -> f64 {
        match self.side {
            Side::Buy => self.size,
            Side::Sell => -self.size,
        }
    }
}

/// Candle data (1-minute OHLCV with CVD and POC)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub symbol: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub cvd: f64,
    pub poc: Option<f64>,
    pub timestamp: DateTime<Utc>,
    pub trades: Vec<Trade>,
}

impl Candle {
    pub fn new(symbol: String, timestamp: DateTime<Utc>) -> Self {
        Self {
            symbol,
            open: 0.0,
            high: 0.0,
            low: f64::MAX,
            close: 0.0,
            volume: 0.0,
            cvd: 0.0,
            poc: None,
            timestamp,
            trades: Vec::new(),
        }
    }

    /// Add a trade to the candle
    pub fn add_trade(&mut self, trade: &Trade) {
        if self.trades.is_empty() {
            self.open = trade.price;
        }

        self.high = self.high.max(trade.price);
        self.low = self.low.min(trade.price);
        self.close = trade.price;
        self.volume += trade.size;
        self.cvd += trade.delta();
        self.trades.push(trade.clone());
    }

    /// Finalize the candle (calculate POC)
    pub fn finalize(&mut self, poc: Option<f64>) {
        self.poc = poc;
    }

    /// Get candle range (high - low)
    pub fn range(&self) -> f64 {
        self.high - self.low
    }

    /// Get candle midpoint
    pub fn midpoint(&self) -> f64 {
        (self.high + self.low) / 2.0
    }

    /// Check if POC is in upper half of range
    pub fn poc_in_upper_half(&self) -> bool {
        if let Some(poc) = self.poc {
            poc > self.midpoint()
        } else {
            false
        }
    }

    /// Check if POC is in lower half of range
    pub fn poc_in_lower_half(&self) -> bool {
        if let Some(poc) = self.poc {
            poc < self.midpoint()
        } else {
            false
        }
    }
}

/// Position data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub symbol: String,
    pub side: PositionSide,
    pub size: f64,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub unrealized_pnl: f64,
    pub flip_streak: u32,
    pub entry_time: DateTime<Utc>,
}

impl Position {
    pub fn new(
        symbol: String,
        side: PositionSide,
        size: f64,
        entry_price: f64,
        stop_loss: f64,
        take_profit: f64,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            symbol,
            side,
            size,
            entry_price,
            stop_loss,
            take_profit,
            unrealized_pnl: 0.0,
            flip_streak: 0,
            entry_time: Utc::now(),
        }
    }

    /// Update unrealized PnL based on current price
    pub fn update_pnl(&mut self, current_price: f64) {
        self.unrealized_pnl = match self.side {
            PositionSide::Long => (current_price - self.entry_price) * self.size,
            PositionSide::Short => (self.entry_price - current_price) * self.size,
        };
    }

    /// Check if stop loss is hit
    pub fn is_sl_hit(&self, current_price: f64) -> bool {
        match self.side {
            PositionSide::Long => current_price <= self.stop_loss,
            PositionSide::Short => current_price >= self.stop_loss,
        }
    }

    /// Check if take profit is hit
    pub fn is_tp_hit(&self, current_price: f64) -> bool {
        match self.side {
            PositionSide::Long => current_price >= self.take_profit,
            PositionSide::Short => current_price <= self.take_profit,
        }
    }

    /// Update stop loss (only move in profitable direction)
    pub fn update_stop_loss(&mut self, new_sl: f64) {
        match self.side {
            PositionSide::Long => {
                if new_sl > self.stop_loss {
                    self.stop_loss = new_sl;
                }
            }
            PositionSide::Short => {
                if new_sl < self.stop_loss {
                    self.stop_loss = new_sl;
                }
            }
        }
    }

    /// Increment flip streak
    pub fn increment_flip_streak(&mut self) {
        self.flip_streak += 1;
    }

    /// Reset flip streak
    pub fn reset_flip_streak(&mut self) {
        self.flip_streak = 0;
    }
}

/// Order data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub price: f64,
    pub size: f64,
    pub status: OrderStatus,
    pub filled_size: f64,
    pub filled_price: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Order {
    pub fn new(symbol: String, side: OrderSide, price: f64, size: f64) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            symbol,
            side,
            price,
            size,
            status: OrderStatus::Pending,
            filled_size: 0.0,
            filled_price: 0.0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Check if order is expired based on TTL
    pub fn is_expired(&self, ttl_seconds: i64) -> bool {
        let now = Utc::now();
        let elapsed = now.signed_duration_since(self.created_at);
        elapsed.num_seconds() > ttl_seconds
    }

    /// Update order status
    pub fn update_status(&mut self, status: OrderStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }

    /// Update fill information
    pub fn update_fill(&mut self, filled_size: f64, filled_price: f64) {
        self.filled_size = filled_size;
        self.filled_price = filled_price;
        self.updated_at = Utc::now();

        if self.filled_size >= self.size {
            self.status = OrderStatus::Filled;
        } else if self.filled_size > 0.0 {
            self.status = OrderStatus::PartiallyFilled;
        }
    }
}

/// Signal types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Signal {
    Long,
    Short,
    None,
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Signal::Long => write!(f, "LONG"),
            Signal::Short => write!(f, "SHORT"),
            Signal::None => write!(f, "NONE"),
        }
    }
}

/// Setup types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SetupType {
    Exhaustion,
    Absorption,
}

impl fmt::Display for SetupType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupType::Exhaustion => write!(f, "EXHAUSTION"),
            SetupType::Absorption => write!(f, "ABSORPTION"),
        }
    }
}

/// Exit reasons
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExitReason {
    StopLoss,
    TakeProfit,
    CvdFlip,
    Manual,
}

impl fmt::Display for ExitReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExitReason::StopLoss => write!(f, "STOP_LOSS"),
            ExitReason::TakeProfit => write!(f, "TAKE_PROFIT"),
            ExitReason::CvdFlip => write!(f, "CVD_FLIP"),
            ExitReason::Manual => write!(f, "MANUAL"),
        }
    }
}

/// Trade signal with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSignal {
    pub signal: Signal,
    pub setup_type: Option<SetupType>,
    pub symbol: String,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub size: f64,
    pub timestamp: DateTime<Utc>,
}

impl TradeSignal {
    pub fn new(
        signal: Signal,
        setup_type: Option<SetupType>,
        symbol: String,
        entry_price: f64,
        stop_loss: f64,
        take_profit: f64,
        size: f64,
    ) -> Self {
        Self {
            signal,
            setup_type,
            symbol,
            entry_price,
            stop_loss,
            take_profit,
            size,
            timestamp: Utc::now(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.signal != Signal::None && self.size > 0.0
    }
}

/// Trade record for completed trades (entry + exit)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub id: String,
    pub symbol: String,
    pub side: PositionSide,
    pub size: f64,
    pub entry_price: f64,
    pub exit_price: f64,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub exit_reason: ExitReason,
    pub setup_type: Option<SetupType>,
}

impl TradeRecord {
    pub fn new(position: &Position, exit_price: f64, exit_reason: ExitReason) -> Self {
        let exit_time = Utc::now();
        let pnl = match position.side {
            PositionSide::Long => (exit_price - position.entry_price) * position.size,
            PositionSide::Short => (position.entry_price - exit_price) * position.size,
        };
        let pnl_pct = (pnl / (position.entry_price * position.size)) * 100.0;

        Self {
            id: Uuid::new_v4().to_string(),
            symbol: position.symbol.clone(),
            side: position.side,
            size: position.size,
            entry_price: position.entry_price,
            exit_price,
            entry_time: position.entry_time,
            exit_time,
            pnl,
            pnl_pct,
            exit_reason,
            setup_type: None,
        }
    }

    /// Get trade duration in seconds
    pub fn duration_secs(&self) -> i64 {
        self.exit_time
            .signed_duration_since(self.entry_time)
            .num_seconds()
    }

    /// Check if trade was profitable
    pub fn is_profitable(&self) -> bool {
        self.pnl > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trade_delta() {
        let trade_buy = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, Utc::now());
        let trade_sell = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Sell, Utc::now());

        assert_eq!(trade_buy.delta(), 1.0);
        assert_eq!(trade_sell.delta(), -1.0);
    }

    #[test]
    fn test_candle_add_trade() {
        let mut candle = Candle::new("BTC".to_string(), Utc::now());
        let trade1 = Trade::new("BTC".to_string(), 50000.0, 1.0, Side::Buy, Utc::now());
        let trade2 = Trade::new("BTC".to_string(), 50100.0, 0.5, Side::Sell, Utc::now());

        candle.add_trade(&trade1);
        candle.add_trade(&trade2);

        assert_eq!(candle.open, 50000.0);
        assert_eq!(candle.high, 50100.0);
        assert_eq!(candle.low, 50000.0);
        assert_eq!(candle.close, 50100.0);
        assert_eq!(candle.volume, 1.5);
        assert_eq!(candle.cvd, 0.5); // 1.0 - 0.5
    }

    #[test]
    fn test_position_pnl() {
        let mut pos = Position::new(
            "BTC".to_string(),
            PositionSide::Long,
            1.0,
            50000.0,
            49000.0,
            52000.0,
        );

        pos.update_pnl(51000.0);
        assert_eq!(pos.unrealized_pnl, 1000.0);

        pos.update_pnl(49000.0);
        assert_eq!(pos.unrealized_pnl, -1000.0);
    }

    #[test]
    fn test_position_sl_tp() {
        let pos = Position::new(
            "BTC".to_string(),
            PositionSide::Long,
            1.0,
            50000.0,
            49000.0,
            52000.0,
        );

        assert!(pos.is_sl_hit(48999.0));
        assert!(!pos.is_sl_hit(49001.0));
        assert!(pos.is_tp_hit(52001.0));
        assert!(!pos.is_tp_hit(51999.0));
    }

    #[test]
    fn test_order_expiry() {
        let order = Order::new("BTC".to_string(), OrderSide::Buy, 50000.0, 1.0);
        assert!(!order.is_expired(120));
    }

    #[test]
    fn test_candle_poc_placement() {
        let mut candle = Candle::new("BTC".to_string(), Utc::now());
        candle.high = 51000.0;
        candle.low = 49000.0;
        candle.poc = Some(50500.0); // Above midpoint (50000.0)

        assert!(candle.poc_in_upper_half());
        assert!(!candle.poc_in_lower_half());
    }
}
