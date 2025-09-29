use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::collections::HashMap;

use crate::engine::types::{DoneReason, Order, OrderRequest, SubmitResult, Resting, Side, Event, BookError};
use std::time::Instant;
use tracing::{info, debug, warn, trace, error, instrument};

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct Book {
    pub bids: BTreeMap<i64, VecDeque<Resting>>,
    pub asks: BTreeMap<i64, VecDeque<Resting>>,
    pub id_index: HashMap<u64, (Side, i64)>,
    next_order_id: u64,
}

impl Book {
    #[instrument]
    pub fn new() -> Self {
        // Initialising a new instance of the orderBook
        let new_book = Book { 
            bids: BTreeMap::new(), 
            asks: BTreeMap::new(), 
            id_index: HashMap::new(),
            next_order_id: 1,
        };
        info!("Initialized new order book");
        new_book
    }

    #[instrument(level = "trace")]
    pub fn best_bid(&self) -> Option<(i64, u64)> {
        // Look up the highest price level on the bid side, and sum up all of the associated order quantities
        for (price, queue) in self.bids.iter().rev() {
            let mut counter = 0;
            for x in queue {
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
        for (price, queue) in &self.asks {
            let mut counter = 0;
            for x in queue {
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

    #[instrument(skip(self, req), fields(side = ?req.side, price = ?req.price))]
    pub fn submit(&mut self, req: &OrderRequest) -> (u64, SubmitResult) {
        let start_time = Instant::now();
        let now = Instant::now();
        let ts = now.elapsed().as_secs(); 
        
        // Generate unique order ID
        let order_id = self.next_order_id;
        self.next_order_id += 1;
        
        debug!(id=order_id, ?req.side, price=?req.price, qty=req.quantity, "Processing order submission");
        
        // Create internal Order with generated ID
        let o = Order {
            id: order_id,
            price: req.price,
            quantity: req.quantity,
            side: req.side,
        };
        
        let result = if req.quantity == 0 {
            let error = BookError::InvalidQuantity { quantity: req.quantity };
            warn!(id=order_id, qty=req.quantity, error=%error, "Rejecting order with invalid quantity");
            SubmitResult {
                events: vec![Event::Done {id: order_id, reason: DoneReason::Rejected, ts}]
            }
        } else if req.price.is_none() {
            // MARKET ORDERS
            debug!(id=order_id, "Processing market order");
            self.execute_market_order(&o, ts)
        } else {
            // LIMIT ORDERS
            debug!(id=order_id, price=?req.price, "Processing limit order");
            self.execute_limit_order(&o, ts)
        };
        
        let processing_time = start_time.elapsed();
        debug!(
            id=order_id, 
            processing_time_ns = processing_time.as_nanos(),
            events_count = result.events.len(),
            "Order processing completed"
        );

        metrics::counter!("lobx_submit_total").increment(1);

        // Record metrics
        metrics::histogram!("lobx_submit_latency_ns").record(processing_time.as_nanos() as f64);
        
        (order_id, result)
    }

    #[instrument(skip(self, o), fields(order_id = o.id, side = ?o.side, price = ?o.price))]
    pub fn execute_limit_order(&mut self, o: &Order, ts: u64) -> SubmitResult {
        let start_time = Instant::now();
        
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

        // Match on whether it's a buy or sell limit order
        // Initialise counter
        // Look up the price level for the bid or ask side
        // While the counter is less than order quantity and the counter is greater that the order quantity,
        // iterate through each element in the VecDeque at that price level and remove the resting order from the queue
        let mut events: Vec<Event> = vec![];
        let mut remaining_qty = o.quantity;
        
        match o.side {
            Side::BUY => {
                // Walk the book from best ask upward until filled or price > limit
                while remaining_qty > 0 {
                    let best_ask_price = match self.best_ask() {
                        Some((price, _)) => price,
                        None => break, // No liquidity available
                    };
                    
                    // Stop if best ask price is higher than our limit
                    if best_ask_price > price {
                        break;
                    }
                    
                    // Fill against this price level
                    if let Some(queue) = self.asks.get_mut(&best_ask_price) {
                        let filled_qty = Self::fill_against_level(o.id, remaining_qty, best_ask_price, queue, ts, &mut events);
                        remaining_qty -= remaining_qty - filled_qty;
                        
                        // Remove empty price levels
                        if queue.is_empty() || queue.iter().all(|r| !r.active || r.remaining == 0) {
                            self.asks.remove(&best_ask_price);
                        }
                    } else {
                        break; // Level not found, stop matching
                    }
                }
            },
            Side::SELL => {
                // Walk the book from best bid downward until filled or price < limit
                while remaining_qty > 0 {
                    let best_bid_price = match self.best_bid() {
                        Some((price, _)) => price,
                        None => break, // No liquidity available
                    };
                    
                    // Stop if best bid price is lower than our limit
                    if best_bid_price < price {
                        break;
                    }
                    
                    // Fill against this price level
                    if let Some(queue) = self.bids.get_mut(&best_bid_price) {
                        let filled_qty = Self::fill_against_level(o.id, remaining_qty, best_bid_price, queue, ts, &mut events);
                        remaining_qty -= remaining_qty - filled_qty;
                        
                        // Remove empty price levels
                        if queue.is_empty() || queue.iter().all(|r| !r.active || r.remaining == 0) {
                            self.bids.remove(&best_bid_price);
                        }
                    } else {
                        break; // Level not found, stop matching
                    }
                }
            }
        }

        // Only add the order to the book if there's remaining quantity after matching
        if remaining_qty > 0 {
            let resting_result = self.add_resting_order(o, price, ts);
            events.extend(resting_result.events);
        } else {
            // Order was fully matched, add a Done event
            events.push(Event::Done {id: o.id, reason: DoneReason::Filled, ts});
        }
        
        // Record limit order execution latency
        let limit_order_latency = start_time.elapsed();
        metrics::histogram!("lobx_limit_order_latency_ns").record(limit_order_latency.as_nanos() as f64);
        debug!(
            id=o.id,
            limit_order_latency_ns = limit_order_latency.as_nanos(),
            "Limit order execution completed"
        );
        
        SubmitResult { events }
    }

    fn add_resting_order(&mut self, o: &Order, price: i64, ts: u64) -> SubmitResult {
        let start_time = Instant::now();
        
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

        if let Some(existing_queue) = level_map.get_mut(&price) {
            existing_queue.push_back(resting);
        } else {
            let mut queue = VecDeque::new();
            queue.push_back(resting);
            level_map.insert(price, queue);
        }
        
        let order_id = o.id;
        let side = o.side;
        self.id_index.insert(order_id, (side, price));
        debug!(id=order_id, price=price, side=?side, "Added order to book");

        // Record order resting latency
        let resting_latency = start_time.elapsed();
        metrics::histogram!("lobx_order_resting_latency_ns").record(resting_latency.as_nanos() as f64);
        debug!(
            id=order_id,
            resting_latency_ns = resting_latency.as_nanos(),
            "Order resting operation completed"
        );

        SubmitResult {
            events: vec![Event::Done {id: o.id, reason: DoneReason::Rested, ts}]
        }
    }

    #[instrument(skip(self, o), fields(order_id = o.id, side = ?o.side, price = ?o.price))]
    pub fn execute_market_order(&mut self, o: &Order, ts: u64) -> SubmitResult {
        let start_time = Instant::now();
        debug!(id=o.id, qty=o.quantity, side=?o.side, "Executing market order");
        
        let mut events = vec![];
        let remaining_qty = match o.side {
            Side::BUY => self.execute_market_buy(o.id, o.quantity, ts, &mut events),
            Side::SELL => self.execute_market_sell(o.id, o.quantity, ts, &mut events),
        };
        
        self.finalize_market_order(o.id, o.quantity, remaining_qty, ts, &mut events);
        
        // Record market order execution latency
        let market_order_latency = start_time.elapsed();
        metrics::histogram!("lobx_market_order_latency_ns").record(market_order_latency.as_nanos() as f64);
        debug!(
            id=o.id,
            market_order_latency_ns = market_order_latency.as_nanos(),
            "Market order execution completed"
        );
        
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
        
        let queue = match self.asks.get_mut(&best_ask_price) {
            Some(queue) => queue,
            None => {
                error!(id=order_id, price=best_ask_price, "Best ask level not found");
                return quantity; // Return all remaining quantity
            }
        };
        
        Self::fill_against_level(order_id, quantity, best_ask_price, queue, ts, events)
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
        
        let queue = match self.bids.get_mut(&best_bid_price) {
            Some(queue) => queue,
            None => {
                error!(id=order_id, price=best_bid_price, "Best bid level not found");
                return quantity; // Return all remaining quantity
            }
        };
        
        Self::fill_against_level(order_id, quantity, best_bid_price, queue, ts, events)
    }

    fn fill_against_level(taker_id: u64, mut remaining_qty: u64, price: i64, queue: &mut VecDeque<Resting>, ts: u64, events: &mut Vec<Event>) -> u64 {
        let start_time = Instant::now();
        let mut fills_count = 0;
        
        for resting_order in queue {
            if resting_order.active && resting_order.remaining > 0 && remaining_qty > 0 {
                let fill_qty = std::cmp::min(remaining_qty, resting_order.remaining);
                resting_order.remaining -= fill_qty;
                remaining_qty -= fill_qty;
                fills_count += 1;
                
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
        
        // Record order matching latency
        let matching_latency = start_time.elapsed();
        metrics::histogram!("lobx_order_matching_latency_ns").record(matching_latency.as_nanos() as f64);
        if fills_count > 0 {
            debug!(
                taker_id=taker_id,
                fills_count=fills_count,
                matching_latency_ns = matching_latency.as_nanos(),
                "Order matching completed"
            );
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

    pub fn cancel_limit_order(&mut self, o: Order, ts: u64) -> Option<SubmitResult> {
        let start_time = Instant::now();
        debug!(id=o.id, "Attempting to cancel limit order");
        // Look up order id in id_index hashmap
        // Extract the tuple represeting the (Side, Price)
        // Remove this entry from the Hashmap
        // Match based on whether the Side is a BUY or SELL 
        // Look up the price in BTreeMap to get to the Level Struct
        // Look up the price inside the Level struct to get to the queue 
        // Iterate through the VecDeque object until we find one where the corresponding resting.id matches the order id
        // Remove the resting order from Level VecDeque
        if let Some(&(side, price)) = self.id_index.get(&o.id) {
            debug!(id=o.id, price=price, side=?side, "Cancelling limit order");
            self.id_index.remove(&o.id);
            match side {
                Side::BUY => {
                    if let Some(queue) = self.bids.get_mut(&price) {
                        let mut counter = 0;
                        for order in queue.iter() {
                            if order.id == o.id {
                                debug!(?queue, "Found limit order to cancel");
                                queue.remove(counter);
                                debug!(?queue, "Limit order cancelled");
                                break;

                            }

                            counter += 1;
                        }

                        // Record cancel order latency for successful cancellation
                        let cancel_latency = start_time.elapsed();
                        metrics::histogram!("lobx_cancel_order_latency_ns").record(cancel_latency.as_nanos() as f64);
                        debug!(
                            id=o.id,
                            cancel_latency_ns = cancel_latency.as_nanos(),
                            "Cancel order operation completed"
                        );

                        Some(SubmitResult {events: vec![Event::Done {id: o.id, reason: DoneReason::Cancelled, ts}]})
                    }

                    else {
                        // Record cancel order latency for failed cancellation
                        let cancel_latency = start_time.elapsed();
                        metrics::histogram!("lobx_cancel_order_latency_ns").record(cancel_latency.as_nanos() as f64);
                        debug!(
                            id=o.id,
                            cancel_latency_ns = cancel_latency.as_nanos(),
                            "Cancel order operation completed"
                        );
                        None
                    }
                }

                Side::SELL => {
                    if let Some(queue) = self.asks.get_mut(&price) {
                        let mut counter = 0;
                        for order in queue.iter() {
                            if order.id == o.id {
                                queue.remove(counter);
                                break;
                            }

                            counter += 1;
                        }
                        
                        // Record cancel order latency for successful cancellation
                        let cancel_latency = start_time.elapsed();
                        metrics::histogram!("lobx_cancel_order_latency_ns").record(cancel_latency.as_nanos() as f64);
                        debug!(
                            id=o.id,
                            cancel_latency_ns = cancel_latency.as_nanos(),
                            "Cancel order operation completed"
                        );
                        
                        Some(SubmitResult {events: vec![Event::Done {id: o.id, reason: DoneReason::Cancelled, ts}]})

                    }

                    else {
                        // Record cancel order latency for failed cancellation
                        let cancel_latency = start_time.elapsed();
                        metrics::histogram!("lobx_cancel_order_latency_ns").record(cancel_latency.as_nanos() as f64);
                        debug!(
                            id=o.id,
                            cancel_latency_ns = cancel_latency.as_nanos(),
                            "Cancel order operation completed"
                        );
                        None
                    }
                }
            }
        }

        else {
            // Record cancel order latency for order not found
            let cancel_latency = start_time.elapsed();
            metrics::histogram!("lobx_cancel_order_latency_ns").record(cancel_latency.as_nanos() as f64);
            debug!(
                id=o.id,
                cancel_latency_ns = cancel_latency.as_nanos(),
                "Cancel order operation completed"
            );
            None
        }

    }


}



#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;
    use tracing_subscriber::EnvFilter;

    static INIT: Once = Once::new();

    fn init_tracing() {
        INIT.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(
                    EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| EnvFilter::new("lobx_rs=trace")),
                )
                .with_test_writer() // <- routes logs to test output
                .try_init();
        });
    }

    #[test]
    fn test_initialise() {
        let book = Book::new();
        // Test that the book is initialized with empty maps
        assert_eq!(book.bids.len(), 0);
        assert_eq!(book.asks.len(), 0);
        assert_eq!(book.id_index.len(), 0);
    }

    #[test]
    fn test_best_bid() {
        let book = Book::new();
        // With empty book, best_bid should return None
        assert_eq!(book.best_bid(), None);
        
        // Add a real bid and test
        let mut book_with_bid = Book::new();
        let req = OrderRequest { side: Side::BUY, price: Some(100), quantity: 10 };
        book_with_bid.submit(&req);
        let best_bid = book_with_bid.best_bid().unwrap().0;
        assert_eq!(best_bid, 100);
    }

    #[test]
    fn test_submit_event() {
        let mut book = Book::new();
        let req = OrderRequest { side: Side::BUY, price: Some(100), quantity: 10 };
        let (order_id, result) = book.submit(&req);
        assert_eq!(order_id, 1); // First order should have ID 1
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
        let req1 = OrderRequest {side: Side::SELL, price: Some(10), quantity: 100 };
        book.submit(&req1);
        let req2 = OrderRequest {side: Side::BUY, price: None, quantity: 10};
        book.submit(&req2);
        let mut fake_asks = BTreeMap::new();
        let mut queue = VecDeque::new();
        queue.push_back(Resting {
            id: 1, // First order gets ID 1
            price: Some(10), 
            remaining: 90,
            ts,
            active: true,
            quantity: 100, 
        });
        fake_asks.insert(10, queue);

        assert_eq!(book.asks, fake_asks);

    }

    #[test]
    fn test_cancel_market_order() {
        init_tracing();
        let now = Instant::now();
        let ts = now.elapsed().as_secs(); 
        let mut book = Book::new();
        let req1 = OrderRequest {side: Side::BUY, price: Some(10), quantity: 100 };
        let (order_id, _) = book.submit(&req1);
        let order1 = Order {id: order_id, side: Side::BUY, price: Some(10), quantity: 100 };
        let mut fake_bids = BTreeMap::new();
        let mut queue = VecDeque::new();
        queue.push_back(Resting {
            id: order_id,
            price: Some(10), 
            remaining: 100,
            ts,
            active: true,
            quantity: 100, 
        });
        fake_bids.insert(10, queue);

        assert_eq!(book.bids, fake_bids);

        book.cancel_limit_order(order1.clone(), ts);

        if let Some(queue) = fake_bids.get_mut(&10) {
            queue.retain(|r| r.id != order1.id); // remove just that order
        }

        assert_eq!(book.bids, fake_bids);

    }

    #[test]
    fn test_limit_order_matching() {
        let now = Instant::now();
        let ts = now.elapsed().as_secs(); 
        let mut book = Book::new();
        let req1 = OrderRequest {side: Side::SELL, price: Some(10), quantity: 100 };
        let (maker_id, _) = book.submit(&req1);
        let req2 = OrderRequest {side: Side::BUY, price: Some(10), quantity: 10};
        let (taker_id, result) = book.submit(&req2);
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.events[0], Event::Fill {taker_id, maker_id, price: 10, qty: 10, ts});
        assert_eq!(result.events[1], Event::Done {id: taker_id, reason: DoneReason::Filled, ts});
    }

    #[test]
    fn test_market_order_fill_events() {
        let now = Instant::now();
        let ts = now.elapsed().as_secs(); 
        let mut book = Book::new();
        let req1 = OrderRequest {side: Side::SELL, price: Some(10), quantity: 100 };
        let (maker_id, _) = book.submit(&req1);
        let req2 = OrderRequest {side: Side::BUY, price: None, quantity: 10};
        let (taker_id, result) = book.submit(&req2);
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.events[0], Event::Fill {taker_id, maker_id, price: 10, qty: 10, ts});
        assert_eq!(result.events[1], Event::Done {id: taker_id, reason: DoneReason::Filled, ts});
    }

    #[test]
    fn test_market_order_no_liquidity() {
        let mut book = Book::new();
        // Submit a BUY market order when there are no asks (no liquidity)
        let req = OrderRequest {side: Side::BUY, price: None, quantity: 10};
        
        let (order_id, _) = book.submit(&req);
        assert_eq!(order_id, 1); // Should still get an ID even if no liquidity
    }

    #[test]
    fn test_no_negative_spread_buy_limit_matches_lower_ask() {
        let mut book = Book::new();
        
        // Add a SELL order at price 11
        let sell_req = OrderRequest {side: Side::SELL, price: Some(11), quantity: 100};
        let (sell_id, _) = book.submit(&sell_req);
        assert_eq!(sell_id, 1);
        
        // Add a BUY order at price 50 (should match against SELL at 11)
        let buy_req = OrderRequest {side: Side::BUY, price: Some(50), quantity: 50};
        let (buy_id, result) = book.submit(&buy_req);
        assert_eq!(buy_id, 2);
        
        // Should have a fill event
        assert_eq!(result.events.len(), 2);
        assert!(matches!(result.events[0], Event::Fill {..}));
        assert!(matches!(result.events[1], Event::Done {..}));
        
        // Check that spread is not negative
        if let Some(spread) = book.spread() {
            assert!(spread >= 0, "Spread should not be negative, got: {}", spread);
        }
        
        // Verify the SELL order was partially filled
        let best_ask = book.best_ask();
        assert!(best_ask.is_some());
        if let Some((price, qty)) = best_ask {
            assert_eq!(price, 11);
            assert_eq!(qty, 50); // 100 - 50 = 50 remaining
        }
    }

    #[test]
    fn test_no_negative_spread_sell_limit_matches_higher_bid() {
        let mut book = Book::new();
        
        // Add a BUY order at price 50
        let buy_req = OrderRequest {side: Side::BUY, price: Some(50), quantity: 100};
        let (buy_id, _) = book.submit(&buy_req);
        assert_eq!(buy_id, 1);
        
        // Add a SELL order at price 11 (should match against BUY at 50)
        let sell_req = OrderRequest {side: Side::SELL, price: Some(11), quantity: 30};
        let (sell_id, result) = book.submit(&sell_req);
        assert_eq!(sell_id, 2);
        
        // Should have a fill event
        assert_eq!(result.events.len(), 2);
        assert!(matches!(result.events[0], Event::Fill {..}));
        assert!(matches!(result.events[1], Event::Done {..}));
        
        // Check that spread is not negative
        if let Some(spread) = book.spread() {
            assert!(spread >= 0, "Spread should not be negative, got: {}", spread);
        }
        
        // Verify the BUY order was partially filled
        let best_bid = book.best_bid();
        assert!(best_bid.is_some());
        if let Some((price, qty)) = best_bid {
            assert_eq!(price, 50);
            assert_eq!(qty, 70); // 100 - 30 = 70 remaining
        }
    }

    #[test]
    fn test_walk_the_book_multiple_levels() {
        let mut book = Book::new();
        
        // Add multiple SELL orders at different price levels
        let sell_req1 = OrderRequest {side: Side::SELL, price: Some(10), quantity: 20};
        let sell_req2 = OrderRequest {side: Side::SELL, price: Some(12), quantity: 30};
        let sell_req3 = OrderRequest {side: Side::SELL, price: Some(15), quantity: 25};
        
        book.submit(&sell_req1);
        book.submit(&sell_req2);
        book.submit(&sell_req3);
        
        // Add a BUY order that should match against all three levels
        let buy_req = OrderRequest {side: Side::BUY, price: Some(20), quantity: 50};
        let (buy_id, result) = book.submit(&buy_req);
        
        // Should have multiple fill events (20 + 30 = 50, so only 2 fills needed)
        let fill_events: Vec<_> = result.events.iter().filter(|e| matches!(e, Event::Fill {..})).collect();
        assert_eq!(fill_events.len(), 2); // Should fill against first two levels
        
        // Check that spread is not negative
        if let Some(spread) = book.spread() {
            assert!(spread >= 0, "Spread should not be negative, got: {}", spread);
        }
        
        // Verify remaining quantities
        let best_ask = book.best_ask();
        assert!(best_ask.is_some());
        if let Some((price, qty)) = best_ask {
            assert_eq!(price, 15); // Highest remaining ask
            assert_eq!(qty, 25); // 25 remaining at price 15
        }
    }

    #[test]
    fn test_limit_order_no_match_rests_correctly() {
        let mut book = Book::new();
        
        // Add a SELL order at price 20
        let sell_req = OrderRequest {side: Side::SELL, price: Some(20), quantity: 100};
        book.submit(&sell_req);
        
        // Add a BUY order at price 10 (should not match, should rest)
        let buy_req = OrderRequest {side: Side::BUY, price: Some(10), quantity: 50};
        let (buy_id, result) = book.submit(&buy_req);
        
        // Should only have a Done event (rested)
        assert_eq!(result.events.len(), 1);
        assert!(matches!(result.events[0], Event::Done {..}));
        
        // Check that spread is positive
        if let Some(spread) = book.spread() {
            assert!(spread > 0, "Spread should be positive, got: {}", spread);
            assert_eq!(spread, 10); // 20 - 10 = 10
        }
        
        // Verify both orders are in the book
        let best_bid = book.best_bid();
        let best_ask = book.best_ask();
        assert!(best_bid.is_some());
        assert!(best_ask.is_some());
        
        if let Some((bid_price, bid_qty)) = best_bid {
            assert_eq!(bid_price, 10);
            assert_eq!(bid_qty, 50);
        }
        
        if let Some((ask_price, ask_qty)) = best_ask {
            assert_eq!(ask_price, 20);
            assert_eq!(ask_qty, 100);
        }
    }
}

