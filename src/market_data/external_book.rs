use std::collections::BTreeMap;

// External book holds normalized prices/sizes
pub struct ExternalBook {
    // price -> size (aggregate)
    pub bids: BTreeMap<i64, u64>, // highest price = best bid
    pub asks: BTreeMap<i64, u64>, // lowest price  = best ask
}

impl ExternalBook {
    pub fn new() -> Self {
        Self { bids: BTreeMap::new(), asks: BTreeMap::new() }
    }

    // Replace the whole book with a fresh snapshot
    pub fn apply_snapshot(&mut self, bids: &[(i64, u64)], asks: &[(i64, u64)]) {
        self.bids.clear();
        self.asks.clear();

        for &(p, s) in bids {
            self.bids.insert(p, s);
        }
        for &(p, s) in asks {
            self.asks.insert(p, s);
        }
    }

    pub fn bbo(&self) -> (Option<(i64, u64)>, Option<(i64, u64)>) {
        let best_bid = self.bids.iter().next_back().map(|(p, s)| (*p, *s));
        let best_ask = self.asks.iter().next().map(|(p, s)| (*p, *s));
        (best_bid, best_ask)
    }
}
