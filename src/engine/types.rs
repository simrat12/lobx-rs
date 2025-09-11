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

pub enum DoneReason { Filled, Rested, Cancelled, Rejected }

pub enum Event {
    Ack  { id: u64, ts: u64 },
    Fill { taker_id: u64, maker_id: u64, price: i64, qty: i64, ts: u64 },
    Done { id: u64, reason: DoneReason, ts: u64 },
}

pub struct SubmitResult {
    pub events: Vec<Event>
}

