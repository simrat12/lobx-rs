use axum::handler::Handler;
use sqlx;
use crate::persist::types::{PersistResult, SnapshotData, PersistanceError};
use crate::persist::SNAPSHOT_SCHEMA_VERSION;
use sqlx::Row;
use crate::persist::SnapshotLevel;
use crate::engine::types::Side;
use crate::persist::wal::{op_to_json, op_from_json};
use crate::persist::{WalStore, WalOp};

use crate::persist::SnapshotStore;
struct PostgresSnapshotStore {
    connection_pool: sqlx::PgPool,
    symbol: String,
}

impl PostgresSnapshotStore {
    pub async fn new(database_url: &str, symbol: &str) -> Self {
        let pool = sqlx::PgPool::connect(database_url).await.unwrap();
        Self {
            connection_pool: pool,
            symbol: symbol.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl SnapshotStore for PostgresSnapshotStore {
    async fn load_snapshot(& self, symbol: & str) -> PersistResult<Option<SnapshotData>> {
        let snapshot = sqlx::query(
            r#"
            SELECT id, schema_version, wal_high_watermark, snapshot_json
            FROM snapshots
            WHERE symbol = $1
            ORDER BY id DESC
            LIMIT 1
            "#
        )
        .bind(symbol)
        .fetch_optional(&self.connection_pool)
        .await
        .map_err(|e| PersistanceError::IoFailure)?;

        if let Some(row) = snapshot {
            let snapshot_json: String = row.get("snapshot_json");
            let mut snapshot: SnapshotData = serde_json::from_str(&snapshot_json)
                .map_err(|_| PersistanceError::FormatMismatch)?;

            snapshot.wal_high_watermark = row.get("wal_high_watermark");

            return PersistResult::Ok(Some(snapshot));
        }
        // Implementation to load snapshot from PostgreSQL
        Ok(None) // Placeholder
    }

    async fn save_snapshot(&mut self, snapshot_data: &SnapshotData) -> PersistResult<()> {
        // Implementation to save snapshot to PostgreSQL
        let wal = sqlx::query::<sqlx::Postgres>(
            "select coalesce(max(id), 0) from wal where symbol = $1"
        )
        .bind(&self.symbol)
        .fetch_optional(&self.connection_pool)
        .await
        .map_err(|_| PersistanceError::IoFailure)?;

        let mut sp_data = snapshot_data.clone();
        if let Some(row) = wal {
            let wal: i64 = row.get(0);
            sp_data.wal_high_watermark = wal;
            let snapshot_json = serde_json::to_string(&sp_data)
                .map_err(|_| PersistanceError::FormatMismatch)?;

            sqlx::query(
                r#"
                INSERT INTO snapshots (symbol, schema_version, wal_high_watermark, snapshot_json)VALUES ($1, $2, $3, $4)
                "#,
            )
            .bind(&snapshot_json)
            .bind(&self.symbol)
            .bind(SNAPSHOT_SCHEMA_VERSION as i32)
            .bind(sp_data.wal_high_watermark)
            .execute(&self.connection_pool).await
            .map_err(|_| PersistanceError::IoFailure)?;

            PersistResult::Ok(())
        } else {
            Err(PersistanceError::IoFailure)
        }
    }
}

struct PostgresWalStore {
    pool: sqlx::PgPool,
    symbol: String,
}

impl PostgresWalStore {
    pub async fn new(database_url: &str, symbol: &str) -> Self {
        let pool = sqlx::PgPool::connect(database_url).await.unwrap();
        Self { pool, symbol: symbol.to_string() }
    }
}

#[async_trait::async_trait]
impl WalStore for PostgresWalStore {
    async fn append_op(&mut self, op: &WalOp) -> PersistResult<()> {
        // This signature should take &self — adjust your trait in mod.rs to:
        // async fn append_op(&self, op: &WalOp) -> PersistResult<()>;
        // (same for relay_ops/rotate)
        let json_string = op_to_json(op)?;
        sqlx::query(
            r#"
            INSERT INTO wal (symbol, op_json) VALUES ($1, $2)
            "#
        )
        .bind(&self.symbol)
        .bind(&json_string)
        .execute(&self.pool)
        .await
        .map_err(|_| PersistanceError::IoFailure)?;

        Ok(())
    }

    async fn relay_ops(&self, after_id: i64) -> PersistResult<Vec<(i64, WalOp)>> {
        // read WAL rows strictly greater than `after_id`
        let rows = sqlx::query(
            r#"
            SELECT id, op_json
            FROM wal
            WHERE symbol = $1 AND id > $2
            ORDER BY id ASC
            "#
        )
        .bind(&self.symbol)
        .bind(after_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| PersistanceError::IoFailure)?;

        let mut ops = Vec::new();
        for row in rows {
            let id: i64 = row.get("id");
            let op_json: String = row.get("op_json");

            let op = op_from_json(&op_json)?;
            ops.push((id, op));
        }

        Ok(ops)
    }


}