use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::collections::HashMap;

use crate::engine::types::{DoneReason, Order, SubmitResult, Resting, Side, Event, BookError};
use std::time::Instant;
use tracing::{info, debug, warn, trace, error, instrument};

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
    #[instrument]
    pub fn new() -> Self {
        // Initialising a new instance of the orderBook
        let mut new_book = Book { bids: BTreeMap::new(), asks: BTreeMap::new(), id_index: HashMap::new() };
        new_book.bids.insert(0, Level{price: 0, queue: VecDeque::new()});
        new_book.asks.insert(0, Level{price: 0, queue: VecDeque::new()});
        info!("Initialized new order book");
        new_book
    }

    #[instrument(level = "trace")]
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
                let result = (*price, counter.try_into().unwrap_or_else(|e| {
                    error!(counter = counter, error = %e, "Failed to convert counter to u64");
                    0
                }));
                trace!(price = result.0, quantity = result.1, "Found best bid");
                return Some(result);
            }
        }
        trace!("No best bid found");
        None
    }

    #[instrument(level = "trace")]
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
                let result = (*price, counter.try_into().unwrap_or_else(|e| {
                    error!(counter = counter, error = %e, "Failed to convert counter to u64");
                    0
                }));
                trace!(price = result.0, quantity = result.1, "Found best ask");
                return Some(result);
            }
        }
        trace!("No best ask found");
        None
    }

    #[instrument(level = "trace")]
    pub fn spread(&self) -> Option<i64> {
        // The difference between the best bid and the best ask
        let best_bid = match self.best_bid() {
            Some((price, _)) => price,
            None => {
                trace!("No best bid available for spread calculation");
                return None;
            }
        };
        
        let best_ask = match self.best_ask() {
            Some((price, _)) => price,
            None => {
                trace!("No best ask available for spread calculation");
                return None;
            }
        };
        
        let spread = best_ask - best_bid; // Fixed: ask - bid, not bid - ask
        trace!(best_bid = best_bid, best_ask = best_ask, spread = spread, "Calculated spread");
        Some(spread)
    }

    #[instrument(fields(order_id = o.id))]
    pub fn submit(&mut self, o: Order) -> SubmitResult {
        let start_time = Instant::now();
        let now = Instant::now();
        let ts = now.elapsed().as_secs(); 
        
        // Store order ID for logging after the order is moved
        let order_id = o.id;
        
        debug!(id=order_id, ?o.side, price=?o.price, qty=o.quantity, "Processing order submission");
        
        let result = if o.quantity == 0 {
            let error = BookError::InvalidQuantity { quantity: o.quantity };
            warn!(id=order_id, qty=o.quantity, error=%error, "Rejecting order with invalid quantity");
            SubmitResult {
                events: vec![Event::Done {id: order_id, reason: DoneReason::Rejected, ts}]
            }
        } else if o.price.is_none() {
            // MARKET ORDERS
            debug!(id=order_id, "Processing market order");
            self.execute_market_order(o, ts)
        } else {
            // LIMIT ORDERS
            debug!(id=order_id, price=?o.price, "Processing limit order");
            self.execute_limit_order(o, ts)
        };
        
        let processing_time = start_time.elapsed();
        debug!(
            id=order_id, 
            processing_time_ns = processing_time.as_nanos(),
            events_count = result.events.len(),
            "Order processing completed"
        );
        
        result
    }

    #[instrument(fields(order_id = o.id))]
    pub fn execute_limit_order(&mut self, o: Order, ts: u64) -> SubmitResult {
        let price = match o.price {
            Some(p) => p,
            None => {
                error!(id=o.id, "Limit order missing price");
                return SubmitResult {
                    events: vec![Event::Done {id: o.id, reason: DoneReason::Rejected, ts}]
                };
            }
        };
        
        debug!(id=o.id, side=?o.side, price=price, qty=o.quantity, "Resting limit order");
        self.add_resting_order(o, price, ts)
    }

    fn add_resting_order(&mut self, o: Order, price: i64, ts: u64) -> SubmitResult {
        let resting = Resting {
            id: o.id,
            price: o.price, 
            remaining: o.quantity,
            ts,
            active: true,
            quantity: o.quantity, 
        };

        let level_map = match o.side {
            Side::BUY => &mut self.bids,
            Side::SELL => &mut self.asks,
        };

        if let Some(existing_level) = level_map.get_mut(&price) {
            existing_level.queue.push_back(resting);
        } else {
            let mut queue = VecDeque::new();
            queue.push_back(resting);
            level_map.insert(price, Level { price, queue });
        }
        
        let order_id = o.id;
        let side = o.side;
        self.id_index.insert(order_id, (side, price));
        debug!(id=order_id, price=price, side=?side, "Added order to book");

        SubmitResult {
            events: vec![Event::Done {id: o.id, reason: DoneReason::Rested, ts}]
        }
    }

    #[instrument(fields(order_id = o.id))]
    pub fn execute_market_order(&mut self, o: Order, ts: u64) -> SubmitResult {
        debug!(id=o.id, qty=o.quantity, side=?o.side, "Executing market order");
        
        let mut events = vec![];
        let remaining_qty = match o.side {
            Side::BUY => self.execute_market_buy(o.id, o.quantity, ts, &mut events),
            Side::SELL => self.execute_market_sell(o.id, o.quantity, ts, &mut events),
        };
        
        self.finalize_market_order(o.id, o.quantity, remaining_qty, ts, &mut events);
        SubmitResult { events }
    }

    fn execute_market_buy(&mut self, order_id: u64, quantity: u64, ts: u64, events: &mut Vec<Event>) -> u64 {
        let best_ask_price = match self.best_ask() {
            Some((price, _)) => price,
            None => {
                let error = BookError::NoLiquidity { side: Side::BUY };
                warn!(id=order_id, error=%error, "No liquidity available for market BUY order");
                return quantity; // Return all remaining quantity
            }
        };
        
        let level = match self.asks.get_mut(&best_ask_price) {
            Some(level) => level,
            None => {
                error!(id=order_id, price=best_ask_price, "Best ask level not found");
                return quantity; // Return all remaining quantity
            }
        };
        
        Self::fill_against_level(order_id, quantity, best_ask_price, level, ts, events)
    }

    fn execute_market_sell(&mut self, order_id: u64, quantity: u64, ts: u64, events: &mut Vec<Event>) -> u64 {
        let best_bid_price = match self.best_bid() {
            Some((price, _)) => price,
            None => {
                let error = BookError::NoLiquidity { side: Side::SELL };
                warn!(id=order_id, error=%error, "No liquidity available for market SELL order");
                return quantity; // Return all remaining quantity
            }
        };
        
        let level = match self.bids.get_mut(&best_bid_price) {
            Some(level) => level,
            None => {
                error!(id=order_id, price=best_bid_price, "Best bid level not found");
                return quantity; // Return all remaining quantity
            }
        };
        
        Self::fill_against_level(order_id, quantity, best_bid_price, level, ts, events)
    }

    fn fill_against_level(taker_id: u64, mut remaining_qty: u64, price: i64, level: &mut Level, ts: u64, events: &mut Vec<Event>) -> u64 {
        for resting_order in &mut level.queue {
            if resting_order.active && resting_order.remaining > 0 && remaining_qty > 0 {
                let fill_qty = std::cmp::min(remaining_qty, resting_order.remaining);
                resting_order.remaining -= fill_qty;
                remaining_qty -= fill_qty;
                
                debug!(taker_id=taker_id, maker_id=resting_order.id, price=price, qty=fill_qty, "Fill executed");
                
                events.push(Event::Fill {
                    taker_id, 
                    maker_id: resting_order.id, 
                    price, 
                    qty: fill_qty, 
                    ts
                });
                
                if remaining_qty == 0 { break; }
            }
        }
        remaining_qty
    }

    fn finalize_market_order(&self, order_id: u64, _original_qty: u64, remaining_qty: u64, ts: u64, events: &mut Vec<Event>) {
        if !events.is_empty() {
            if remaining_qty == 0 {
                events.push(Event::Done {id: order_id, reason: DoneReason::Filled, ts});
                debug!(id=order_id, "Market order fully filled");
            } else {
                events.push(Event::Done {id: order_id, reason: DoneReason::Rejected, ts});
                warn!(id=order_id, remaining_qty=remaining_qty, "Market order partially filled - insufficient liquidity");
            }
        } else {
            warn!(id=order_id, "Market order rejected - no fills executed");
            events.push(Event::Done {id: order_id, reason: DoneReason::Rejected, ts});
        }
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

