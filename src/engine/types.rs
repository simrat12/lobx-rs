#[derive(Debug, PartialEq, Eq, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Side {
    BUY,
    SELL
}
// Order request from client/strategy (no ID assigned yet)
#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrderRequest {
    pub price: Option<u64>,
    pub quantity: u64,
    pub side: Side
}

// Order with assigned ID (for internal use)
#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct Order {
    pub id: u64,
    pub price: Option<u64>,
    pub quantity: u64,
    pub side: Side
}

// Resting order in the book (mutable remaining)
#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct Resting {
    pub id: u64,
    pub price: Option<u64>,
    pub quantity: u64,
    pub ts: u64,
    pub remaining: u64,
    pub active: bool
}

 // Fill (execution) event
 #[derive(Debug, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct Fill {
    pub taker_id: u64,
    pub maker_id: u64,
    pub price: Option<u64>,
    pub quantity: u64,
    pub ts: u64
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub enum DoneReason { Filled, Rested, Cancelled, Rejected }

// Error types for better error handling
#[derive(thiserror::Error, Debug)]
pub enum BookError {
    #[error("Invalid order quantity: {quantity}")]
    InvalidQuantity { quantity: u64 },
    
    #[error("Order {id} not found")]
    OrderNotFound { id: u64 },
    
    #[error("No liquidity available for {side:?} market order")]
    NoLiquidity { side: Side },
    
    #[error("Price level {price} not found for {side:?}")]
    PriceLevelNotFound { price: u64, side: Side },
    
    #[error("Invalid price for limit order")]
    InvalidPrice,
    
    #[error("Integer conversion error: {source}")]
    ConversionError { 
        #[from]
        source: std::num::TryFromIntError 
    },
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub enum Event {
    Ack  { id: u64, ts: u64 },
    Fill { taker_id: u64, maker_id: u64, price: u64, qty: u64, ts: u64 },
    Done { id: u64, reason: DoneReason, ts: u64 },
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubmitResult {
    pub events: Vec<Event>
}

pub type BookResult<T> = Result<T, BookError>;

