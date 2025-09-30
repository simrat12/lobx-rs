-- Snapshots (append-only)
CREATE TABLE IF NOT EXISTS snapshots (
  id                 BIGSERIAL PRIMARY KEY,
  symbol             TEXT NOT NULL,
  schema_version     INT  NOT NULL,
  wal_high_watermark BIGINT NOT NULL,
  snapshot_json      JSONB NOT NULL,
  created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS snapshots_symbol_id_idx
  ON snapshots(symbol, id DESC);

-- WAL (append-only)
CREATE TABLE IF NOT EXISTS wal (
  id         BIGSERIAL PRIMARY KEY,
  symbol     TEXT NOT NULL,
  ts         TIMESTAMPTZ NOT NULL DEFAULT now(),
  op_json    JSONB NOT NULL
);

CREATE INDEX IF NOT EXISTS wal_symbol_id_idx
  ON wal(symbol, id);
