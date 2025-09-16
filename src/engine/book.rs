use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::collections::HashMap;

use crate::engine::types::DoneReason;
use crate::engine::types::Order;
use crate::engine::types::SubmitResult;
use crate::engine::types::{Resting, Side, Event};
use crate::engine::book::Side::BUY;
use std::time::Instant;
use tracing::{info, debug, warn, trace};

#[derive(Debug, PartialEq, Eq)]
struct Level {
    price: i64,
    queue: VecDeque<Resting>
}

#[derive(Debug, PartialEq, Eq)]
pub struct Book {
    pub bids: BTreeMap<i64, Level>,
    pub asks: BTreeMap<i64, Level>,
    pub id_index: HashMap<u64, (Side, i64)>
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
        for (price, level) in self.bids.iter().rev() {
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
        debug!(id=o.id, ?o.side, price=?o.price, qty=o.quantity, "submit");
        // If quantity is 0, reject the order
        if o.quantity <= 0 {
            SubmitResult {
                events: vec![Event::Done {id: o.id, reason: DoneReason::Rejected, ts}]
            }
        }

        else if o.price.is_none() {
            // MARKET ORDERS
            self.execute_market_order(o, ts)
        }

        else {
            // LIMIT ORDERS
            self.execute_limit_order(o, ts)
        }

    }

    pub fn execute_limit_order(&mut self, o: Order, ts: u64) -> SubmitResult {
        let bid_price = o.price.unwrap();
        let mut queue = VecDeque::new();
        info!(side=?o.side, px=?o.price.unwrap(), qty=o.quantity, "rested_limit");
        match o.side {
            Side::BUY => {
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
            Side::SELL => {
                queue.push_back(Resting {
                    id: o.id,
                    price: o.price, 
                    remaining: o.quantity,
                    ts,
                    active: true,
                    quantity: o.quantity, 
                });
                self.asks.insert(bid_price, Level { price: bid_price, queue });

                SubmitResult {
                    events: vec![Event::Done {id: o.id, reason: DoneReason::Rested, ts}]
                }
            }
        }
    }

    pub fn execute_market_order(&mut self, o: Order, ts: u64) -> SubmitResult {
        let mut events = vec![];
        match o.side { 
            Side::BUY => {
                // MARKET ORDER - BID
                // Get the best ask price
                let best_ask = self.best_ask().unwrap().0;
                let order_qts = &mut self.asks.get_mut(&best_ask).unwrap();
                let mut counter = o.quantity;
                // iterate through order book asks until 'remaining' = 0 
                for x in &mut order_qts.queue {
                    if x.active && x.remaining > 0 {
                        let fill_qty = std::cmp::min(counter, x.remaining);
                        x.remaining -= fill_qty;
                        counter -= fill_qty;
                        
                        events.push(Event::Fill {taker_id: o.id, maker_id: x.id, price: best_ask, qty: fill_qty, ts});
                        
                        if counter <= 0 { break; }
                    }
                }
            }
            Side::SELL => {
                // MARKET ORDER - ASK
                let best_bid = self.best_bid().unwrap().0;
                let order_qts = &mut self.bids.get_mut(&best_bid).unwrap();
                let mut counter = o.quantity;
                for x in &mut order_qts.queue {
                    if x.active && x.remaining > 0 {
                        let fill_qty = std::cmp::min(counter, x.remaining);
                        x.remaining -= fill_qty;
                        counter -= fill_qty;
                        
                        events.push(Event::Fill {taker_id: o.id, maker_id: x.id, price: best_bid, qty: fill_qty, ts});
                        
                        if counter <= 0 { break; }
                    }
                }
            }
        }
        
        // Add Done event if any fills occurred
        if !events.is_empty() {
            events.push(Event::Done {id: o.id, reason: DoneReason::Filled, ts});
        }
        
        SubmitResult { events }
    }


}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialise() {
        let book = Book::new();
        // Test that the book is initialized with dummy levels
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.asks.len(), 1);
        assert!(book.bids.contains_key(&0));
        assert!(book.asks.contains_key(&0));
        assert_eq!(book.id_index.len(), 0);
        
        // Test that dummy levels have empty queues
        assert_eq!(book.bids.get(&0).unwrap().queue.len(), 0);
        assert_eq!(book.asks.get(&0).unwrap().queue.len(), 0);
    }

    #[test]
    fn test_best_bid() {
        let book = Book::new();
        // With only dummy levels, best_bid should return None
        assert_eq!(book.best_bid(), None);
        
        // Add a real bid and test
        let mut book_with_bid = Book::new();
        let order = Order { id: 1, side: Side::BUY, price: Some(100), quantity: 10 };
        book_with_bid.submit(order);
        let best_bid = book_with_bid.best_bid().unwrap().0;
        assert_eq!(best_bid, 100);
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

    #[test]
    fn test_market_order_fill_events() {
        let now = Instant::now();
        let ts = now.elapsed().as_secs(); 
        let mut book = Book::new();
        let order1 = Order {id: 1, side: Side::SELL, price: Some(10), quantity: 100 };
        book.submit(order1);
        let order2 = Order {id: 2, side: Side::BUY, price: None, quantity: 10};
        let result = book.submit(order2);
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.events[0], Event::Fill {taker_id: 2, maker_id: 1, price: 10, qty: 10, ts});
        assert_eq!(result.events[1], Event::Done {id: 2, reason: DoneReason::Filled, ts});
    }

    #[test]
    fn test_market_order_no_liquidity() {
        let mut book = Book::new();
        // Submit a BUY market order when there are no asks (no liquidity)
        let market_order = Order {id: 1, side: Side::BUY, price: None, quantity: 10};
        
        book.submit(market_order);
    }
}

