use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::collections::HashMap;
use crate::engine::types::{Resting, Side};

struct Level {
    price: i64,
    queue: VecDeque<Resting>
}

struct Book {
    bids: BTreeMap<i64, Level>,
    asks: BTreeMap<i64, Level>,
    id_index: HashMap<u64, (Side, i64)>
}

impl Book {
    pub fn new(self) -> Self {
        Book { bids: BTreeMap::new(), asks: BTreeMap::new(), id_index: HashMap::new() }
    }

    pub fn best_bid(&self) -> Option<(i64, u64)> {
        if let Some((i, y)) = &self.bids.last_key_value() {
            let best_price = *i;
            let mut counter = 0;
            for x in &y.queue {
                if x.active && x.remaining > 0 {
                    counter += x.remaining
                }
            }

            return Some((*best_price, counter.try_into().unwrap()))
        }

        else {
            None
        }
    }
}


