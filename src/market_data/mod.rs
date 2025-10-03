// Market data module entrypoint
pub mod adapters;       // venue-specific fetchers (e.g. Hyperliquid)
pub mod normaliser;     // converts strings -> ticks/lots
pub mod external_book;  // in-memory representation of external book
pub mod unified_book;   // read-only facade merging internal + external books
pub mod market_maker;   // market-making logic with quote management
pub mod router;         // orchestrates everything for demo
