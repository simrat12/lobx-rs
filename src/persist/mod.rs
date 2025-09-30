pub mod types;
pub use types::*;
pub mod snapshot;
pub mod wal;
pub mod postgres;
use async_trait::async_trait;

#[async_trait]
pub trait SnapshotStore {
    async fn load_snapshot(&self, symbol: &str) -> PersistResult<Option<SnapshotData>>;
    async fn save_snapshot(&mut self ,snapshot: &SnapshotData) -> PersistResult<()>;   

}

#[async_trait]
pub trait WalStore {
    async fn append_op(&mut self, wal: &WalOp) -> PersistResult<()>;
    async fn relay_ops(&self, id: i64) -> PersistResult<Vec<(i64, WalOp)>>;
}

#[async_trait]
pub trait PersistenceEngine {
    async fn restore() -> PersistResult<Option<SnapshotData>>;
    async fn replay() -> PersistResult<()>;
    async fn record(wal: &WalOp) -> PersistResult<()>;
    async fn checkpoint(snapshot:&SnapshotData) -> PersistResult<()>;
    
}