#[derive(Debug, PartialEq, Eq)]
pub enum Side {
    BUY,
    SELL
}
// Order as submitted by client/strategy
#[derive(Debug, PartialEq, Eq)]
pub struct Order {
    pub id: u64,
    pub price: Option<i64>,
    pub quantity: u64,
    pub side: Side
}

// Resting order in the book (mutable remaining)
#[derive(Debug, PartialEq, Eq)]
pub struct Resting {
    pub id: u64,
    pub price: Option<i64>,
    pub quantity: u64,
    pub ts: u64,
    pub remaining: u64,
    pub active: bool
}

 // Fill (execution) event
 #[derive(Debug, PartialEq, Eq)]
pub struct Fill {
    pub taker_id: u64,
    pub maker_id: u64,
    pub price: Option<i64>,
    pub quantity: u64,
    pub ts: u64
}

#[derive(Debug, PartialEq, Eq)]
pub enum DoneReason { Filled, Rested, Cancelled, Rejected }

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Ack  { id: u64, ts: u64 },
    Fill { taker_id: u64, maker_id: u64, price: i64, qty: u64, ts: u64 },
    Done { id: u64, reason: DoneReason, ts: u64 },
}

#[derive(Debug, PartialEq, Eq)]
pub struct SubmitResult {
    pub events: Vec<Event>
}

