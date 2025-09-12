use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::collections::HashMap;
use crate::engine::types::DoneReason;
use crate::engine::types::Order;
use crate::engine::types::SubmitResult;
use crate::engine::types::{Resting, Side, Event};
use crate::engine::book::Side::BUY;
use std::time::Instant;

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
    pub fn new() -> Self {
        let mut new_book = Book { bids: BTreeMap::new(), asks: BTreeMap::new(), id_index: HashMap::new() };
        new_book.bids.insert(0, Level{price: 0, queue: VecDeque::new()});
        new_book.asks.insert(0, Level{price: 0, queue: VecDeque::new()});
        new_book
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

    pub fn best_ask(&self) -> Option<(i64, u64)> {
        if let Some((i, y)) = &self.bids.first_key_value() {
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

    pub fn spread(&self) -> Option<i64> {

        let best_bid = &self.best_bid().unwrap().0;
        let best_ask = &self.best_ask().unwrap().0;
        let spread = best_bid - best_ask;
        
        return Some(spread)
    }

    pub fn submit(&mut self, o: Order) -> SubmitResult {
        let now = Instant::now();
        let ts = now.elapsed().as_secs(); // or use a suitable conversion to u64 timestamp
        let events = vec![Event::Ack {id: o.id, ts}];
        if o.quantity <= 0 {
            SubmitResult {
                events: vec![Event::Done {id: o.id, reason: DoneReason::Rejected, ts}]
            }
        }

        else if o.price.is_none() {
            if o.side == BUY {
                // Look up the price level
                // iterate through order book asks until 'remaining' = 0 
                let bid_price = o.price.unwrap();
                let order_qts = &mut self.asks.get_mut(&bid_price).unwrap();
                let counter = &o.quantity;
                for x in &mut order_qts.queue {
                    if x.active && x.remaining > 0 && *counter >= x.remaining {
                        x.remaining -= *counter
                    }
                }
            }

            SubmitResult {
                events: vec![Event::Done {id: o.id, reason: DoneReason::Filled, ts}]
            }
        }

        else {
            SubmitResult {
                events
            }
        }

    }


}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialise() {
        let _book = Book::new();
        // Optionally, add assertions here to test initialization
    }

    #[test]
    fn test_best_bid() {
        let book = Book::new();
        let best_bid = book.best_bid().unwrap().0;
        assert_eq!(best_bid, 0);
    }
}

