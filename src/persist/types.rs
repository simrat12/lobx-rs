use crate::engine::types::{Resting, Side};

pub enum PersistanceError{
    IoFailure,
    SerializationFailure,
    FormatMismatch,
    CorruptWalRecord,
    NotFound,
    Other(String),
}

pub type PersistResult<T> = Result<T, PersistanceError>;


pub const SNAPSHOT_SCHEMA_VERSION: u32 =1;

#[derive(Clone, serde::Serialize)]
pub struct SnapshotData {
    pub version: u32,
    pub bid_side: Vec<SnapshotLevel>,
    pub ask_side: Vec<SnapshotLevel>,
    pub id_index: Vec<(u64, Side, u64)>, // (order_id, price)
    pub next_order_id: u64,
    pub wal_high_watermark: i64
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct SnapshotLevel {
    price: i64,
    orders: Vec<SnapshotResting>
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct SnapshotResting {
    id: u64,
    quantity: u64,
    ts: u64,
    remaining: u64,
    active: bool
}

pub enum WalOp{
    LimitOrderSubmitted{
        order_id: u64, 
        side: Side, 
        price: u64, 
        quantity: u64
    },
    MarketOrderSubmitted{
        order_id: u64, 
        side: Side, 
        quantity: u64
    },
    OrderCancelled{
        order_id: u64
    },
}
