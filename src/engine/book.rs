use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::collections::HashMap;

use crate::engine::types::DoneReason;
use crate::engine::types::Order;
use crate::engine::types::SubmitResult;
use crate::engine::types::{Resting, Side, Event};
use crate::engine::book::Side::BUY;
use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
struct Level {
    price: i64,
    queue: VecDeque<Resting>
}

#[derive(Debug, PartialEq, Eq)]
struct Book {
    bids: BTreeMap<i64, Level>,
    asks: BTreeMap<i64, Level>,
    id_index: HashMap<u64, (Side, i64)>
}

impl Book {
    pub fn new() -> Self {
        // Initialising a new instance of the orderBook
        let mut new_book = Book { bids: BTreeMap::new(), asks: BTreeMap::new(), id_index: HashMap::new() };
        new_book.bids.insert(0, Level{price: 0, queue: VecDeque::new()});
        new_book.asks.insert(0, Level{price: 0, queue: VecDeque::new()});
        new_book
    }

    pub fn best_bid(&self) -> Option<(i64, u64)> {
        // Look up the highest price level on the bid side, and sum up all of the associated order quantities
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
        // Look up the smallest value on the ask side, and sum up all the associatd quantities
        for (price, level) in &self.asks {
            if *price == 0 { continue; } // Skip dummy level
            let mut counter = 0;
            for x in &level.queue {
                if x.active && x.remaining > 0 {
                    counter += x.remaining
                }
            }
            if counter > 0 {
                return Some((*price, counter.try_into().unwrap()))
            }
        }
        None
    }

    pub fn spread(&self) -> Option<i64> {
        // The difference between the best bid and the best ask

        let best_bid = &self.best_bid().unwrap().0;
        let best_ask = &self.best_ask().unwrap().0;
        let spread = best_bid - best_ask;
        
        return Some(spread)
    }

    pub fn submit(&mut self, o: Order) -> SubmitResult {
        let now = Instant::now();
        let ts = now.elapsed().as_secs(); 
        // If quantity is 0, reject the order
        if o.quantity <= 0 {
            SubmitResult {
                events: vec![Event::Done {id: o.id, reason: DoneReason::Rejected, ts}]
            }
        }

        else if o.price.is_none() {
            // MARKET ORDERS
            if o.side == BUY {
                // MARKET ORDER - BID
                // Get the best ask price
                // iterate through order book asks until 'remaining' = 0 
                let best_ask = self.best_ask().unwrap().0;
                let order_qts = &mut self.asks.get_mut(&best_ask).unwrap();
                let mut counter = o.quantity;
                for x in &mut order_qts.queue {
                    if x.active && x.remaining > 0 {
                        let fill_qty = std::cmp::min(counter, x.remaining);
                        x.remaining -= fill_qty;
                        counter -= fill_qty;
                        if counter <= 0 { break; }
                    }
                }
            }

            else {
                // MARKET ORDER - ASK
                let best_bid = self.best_bid().unwrap().0;
                let order_qts = &mut self.bids.get_mut(&best_bid).unwrap();
                let mut counter = o.quantity;
                for x in &mut order_qts.queue {
                    if x.active && x.remaining > 0 {
                        let fill_qty = std::cmp::min(counter, x.remaining);
                        x.remaining -= fill_qty;
                        counter -= fill_qty;
                        if counter <= 0 { break; }
                    }
                }
            }

            SubmitResult {
                events: vec![Event::Done {id: o.id, reason: DoneReason::Filled, ts}]
            }
        }

        else {
            
            if o.side == BUY {
                let bid_price = o.price.unwrap();
                let mut queue = VecDeque::new();
                queue.push_back(Resting {
                    id: o.id,
                    price: o.price, 
                    remaining: o.quantity,
                    ts,
                    active: true,
                    quantity: o.quantity, 
                });
                self.bids.insert(bid_price, Level { price: bid_price, queue });

                SubmitResult {
                    events: vec![Event::Done {id: o.id, reason: DoneReason::Rested, ts}]
                }
            }

            else {
                let ask_price = o.price.unwrap();
                let mut queue = VecDeque::new();
                queue.push_back(Resting {
                    id: o.id,
                    price: o.price, 
                    remaining: o.quantity,
                    ts,
                    active: true,
                    quantity: o.quantity, 
                });
                self.asks.insert(ask_price, Level { price: ask_price, queue });

                SubmitResult {
                    events: vec![Event::Done {id: o.id, reason: DoneReason::Rested, ts}]
                }
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

    #[test]
    fn test_submit_event() {
        let mut book = Book::new();
        let order = Order { id: 1, side: Side::BUY, price: Some(100), quantity: 10 };
        let result = book.submit(order);
        assert_eq!(result.events.len(), 1);
        // if let Event::Ack { id, .. } = result.events[0] {
        //     assert_eq!(id, 1);
        // } else {
        //     panic!("Expected Ack event");
        // }
    }

    #[test]
    fn test_market_order_fill() {
        let now = Instant::now();
        let ts = now.elapsed().as_secs(); 
        let mut book = Book::new();
        let order1 = Order {id: 1, side: Side::SELL, price: Some(10), quantity: 100 };
        book.submit(order1);
        let order2 = Order {id: 2, side: Side::BUY, price: None, quantity: 10};
        book.submit(order2);
        let mut fake_asks = BTreeMap::new();
        let mut queue = VecDeque::new();
        queue.push_back(Resting {
            id: 1,
            price: Some(10), 
            remaining: 90,
            ts,
            active: true,
            quantity: 100, 
        });
        fake_asks.insert(0, Level{price: 0, queue: VecDeque::new()});
        fake_asks.insert(10, Level{price: 10, queue});

        assert_eq!(book.asks, fake_asks);

    }
}

