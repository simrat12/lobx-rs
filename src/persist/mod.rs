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

pub trait WalStore {
    async fn append_op(&Op) -> Result<(), Op>;
    async fn relay_ops() -> Result<Vec<Op>, ()>;
    async fn rotate() -> PersistResult<()>;
}

pub trait PersistenceEngine {
    async fn restore() -> PersistResult<Option<SnapshotData>>;
    async fn replay() -> Result<Vec<Op>, ()>;
    async fn record(&Op) -> Result<(), Op>;
    async fn checkpoint(snapshot:&SnapshotData) -> PersistResult<()>;
    
}