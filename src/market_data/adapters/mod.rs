// Shared trait + event for market data adapters

pub enum MarketEvent {
    // Full snapshot at a point in time (already normalized to ints)
    Snapshot {
        coin: String,
        bids: Vec<(i64, u64)>, // (price_ticks, size_lots)
        asks: Vec<(i64, u64)>,
        ts_ms: u64,
    },
}

#[async_trait::async_trait]
pub trait VenueAdapter {
    // Send events into the router; you'll pass an mpsc::Sender<MarketEvent> from router.
    async fn spawn(&self, tx: tokio::sync::mpsc::Sender<MarketEvent>);
}

// Make the Hyperliquid adapter visible
pub mod hyperliquid;
pub mod hyperliquid_types; 
