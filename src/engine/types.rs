pub enum Side {
    BUY,
    SELL
}
// Order as submitted by client/strategy
pub struct Order {
    pub id: u64,
    pub price: Option<i64>,
    pub quantity: i64,
    pub side: Side
}

// Resting order in the book (mutable remaining)
pub struct Resting {
    pub id: u64,
    pub price: Option<i64>,
    pub quantity: i64,
    pub ts: u64,
    pub remaining: i64,
    pub active: bool
}

 // Fill (execution) event
pub struct Fill {
    pub taker_id: u64,
    pub maker_id: u64,
    pub price: Option<i64>,
    pub quantity: i64,
    pub ts: u64
}

pub struct Ack {
    pub id: u64,
    pub ts: u64
}

pub enum DoneReason {
    FILLED,
    RESTED,
    CANCELLED,
    REJECTED
}

pub enum Event {
    Ack,
    Fill,
    DoneReason
}

