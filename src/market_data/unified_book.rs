use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::engine::book::Book;
use crate::market_data::external_book::ExternalBook;

/// Read-only facade that lets me *query* a combined view.
pub struct UnifiedBook {
    pub internal: Arc<Mutex<Book>>,
    pub external: Arc<Mutex<ExternalBook>>,
    /// Price scaling used by ExternalBook (ticks) so we can compare apples-to-apples.
    /// My internal Book uses integer prices too, but types differ (u64 vs i64).
    pub price_scale: i64, // e.g. 1_000_000 for 6 dp
}

impl UnifiedBook {
    pub fn new(internal: Arc<Mutex<Book>>, external: Arc<Mutex<ExternalBook>>, price_scale: i64) -> Self {
        Self { internal, external, price_scale }
    }

    /// Combined best bid/ask: pick the *better* side from internal vs external.
    /// Returns ((bid_px_ticks, bid_sz), (ask_px_ticks, ask_sz)) in ExternalBook tick scale.
    pub fn combined_bbo(&self) -> (Option<(i64, u64)>, Option<(i64, u64)>) {
        let (ext_bid, ext_ask) = self.external.lock().unwrap().bbo();
        let (int_bid, int_ask) = {
            let b = self.internal.lock().unwrap();
            (b.best_bid().map(|(p, q)| (p as i64, q)), b.best_ask().map(|(p, q)| (p as i64, q)))
        };

        // choose higher bid
        let best_bid = match (ext_bid, int_bid) {
            (None, x) | (x, None) => x,
            (Some(e), Some(i)) => Some(if i.0 > e.0 { i } else { e }),
        };
        // choose lower ask
        let best_ask = match (ext_ask, int_ask) {
            (None, x) | (x, None) => x,
            (Some(e), Some(i)) => Some(if i.0 < e.0 { i } else { e }),
        };
        (best_bid, best_ask)
    }

    /// Very small helper: merge depth maps (sum sizes by price) and return top-N.
    /// This is *demo simple* merging; not an exchange-grade aggregator.
    pub fn combined_depth_top_n(&self, n: usize) -> (Vec<(i64, u64)>, Vec<(i64, u64)>) {
        let ext = self.external.lock().unwrap();
        let mut bids: BTreeMap<i64, u64> = ext.bids.clone(); // i64->u64
        let mut asks: BTreeMap<i64, u64> = ext.asks.clone();

        let book = self.internal.lock().unwrap();
        for (p, q) in book.bids.iter().flat_map(|(px, q)| {
            let sum: u64 = q.iter().filter(|r| r.active && r.remaining > 0).map(|r| r.remaining).sum();
            if sum > 0 { Some((*px as i64, sum)) } else { None }
        }) {
            *bids.entry(p).or_default() += q;
        }
        for (p, q) in book.asks.iter().flat_map(|(px, q)| {
            let sum: u64 = q.iter().filter(|r| r.active && r.remaining > 0).map(|r| r.remaining).sum();
            if sum > 0 { Some((*px as i64, sum)) } else { None }
        }) {
            *asks.entry(p).or_default() += q;
        }

        let top_bids = bids.iter().rev().take(n).map(|(p, s)| (*p, *s)).collect();
        let top_asks = asks.iter().take(n).map(|(p, s)| (*p, *s)).collect();
        (top_bids, top_asks)
    }
}
