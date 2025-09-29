use axum::handler::Handler;
use sqlx;
use crate::persist::types::{PersistResult, SnapshotData, PersistanceError};
use crate::persist::SNAPSHOT_SCHEMA_VERSION;
use sqlx::Row;
use crate::persist::SnapshotLevel;
use crate::engine::types::Side;

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
            let schema_version: i32 = row.get("schema_version");
            let wal_high_watermark: i64 = row.get("wal_high_watermark");
            let bid_side_json: String = row.get("bid_side");
            let ask_side_json: String = row.get("ask_side");
            let id_index_json: String = row.get("id_index");
            let next_order_id: i64 = row.get("next_order_id");

            let bid_side: Vec<SnapshotLevel> = serde_json::from_str(&bid_side_json)
                .map_err(|_| PersistanceError::FormatMismatch)?;
            let ask_side: Vec<SnapshotLevel> = serde_json::from_str(&ask_side_json)
                .map_err(|_| PersistanceError::FormatMismatch)?;
            let id_index: Vec<(u64, Side, u64)> = serde_json::from_str(&id_index_json)
                .map_err(|_| PersistanceError::FormatMismatch)?;

            if schema_version != SNAPSHOT_SCHEMA_VERSION as i32 {
                return Err(PersistanceError::FormatMismatch);
            }

            let snapshot = SnapshotData {
                version: schema_version as u32,
                bid_side: bid_side,
                ask_side: ask_side,
                id_index: id_index,
                next_order_id: next_order_id as u64,
                wal_high_watermark,
            };

            return PersistResult::Ok(Some(snapshot));
        }
        // Implementation to load snapshot from PostgreSQL
        Ok(None) // Placeholder
    }

    async fn save_snapshot(&mut self, snapshotData: &SnapshotData) -> PersistResult<()> {
        // Implementation to save snapshot to PostgreSQL
        let wal = sqlx::query::<sqlx::Postgres>(
            "select coalesce(max(id), 0) from wal where symbol = $1"
        )
        .fetch_optional(&self.connection_pool)
        .await
        .map_err(|_| PersistanceError::IoFailure)?;

        let mut sp_data = snapshotData.clone();
        if let Some(row) = wal {
            let wal: i64 = row.get(0);
            sp_data.wal_high_watermark = wal;
            let snapshot_json = serde_json::to_string(&sp_data)
                .map_err(|_| PersistanceError::FormatMismatch)?;

            sqlx::query(
                r#"
                INSERT INTO snapshots (snapshot_json)
                "#,
            ).bind(&snapshot_json)
            .fetch_optional(&self.connection_pool).await
            .map_err(|_| PersistanceError::IoFailure)?;

            PersistResult::Ok(())
        } else {
            Err(PersistanceError::IoFailure)
        }
    }
}