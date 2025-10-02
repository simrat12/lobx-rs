// Market data module entrypoint
pub mod adapters;       // venue-specific fetchers (e.g. Hyperliquid)
pub mod normaliser;     // converts strings -> ticks/lots
pub mod external_book;  // in-memory representation of external book
pub mod router;         // orchestrates everything for demo
