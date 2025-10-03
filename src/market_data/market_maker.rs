use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::engine::book::Book;
use crate::engine::types::{OrderRequest, Side, Order};
use crate::market_data::external_book::ExternalBook;

#[derive(Debug)]
pub struct MarketMaker {
    pub book: Arc<Mutex<Book>>,
    pub active_quotes: HashMap<String, u64>, // "bid_level_1" -> order_id
    pub inventory: i64, // positive = long, negative = short
    pub next_quote_id: u64,
}

impl MarketMaker {
    pub fn new(book: Arc<Mutex<Book>>) -> Self {
        Self {
            book,
            active_quotes: HashMap::new(),
            inventory: 0,
            next_quote_id: 1000, // Start from 1000 to avoid conflicts with demo orders
        }
    }

    /// Cancel all existing quotes and post new ones based on external market
    pub fn update_quotes(&mut self, ext_bid: Option<(i64, u64)>, ext_ask: Option<(i64, u64)>, spread_bps: u64) -> Vec<String> {
        let mut actions = Vec::new();
        
        // Get external market mid
        if let (Some((bid_px, _)), Some((ask_px, _))) = (ext_bid, ext_ask) {
            let mid = (bid_px + ask_px) / 2;
            let spread_ticks = (mid * spread_bps as i64) / 10000; // Convert bps to ticks
            
            // Calculate inventory-adjusted spread (wider when we have inventory risk)
            let inventory_adjustment = (self.inventory.abs() * spread_ticks) / 100; // 1% wider per 100 units
            let adjusted_spread = spread_ticks + inventory_adjustment;
            
            let our_bid = mid - adjusted_spread / 2;
            let our_ask = mid + adjusted_spread / 2;
            
            // Cancel existing quotes
            for (quote_type, order_id) in self.active_quotes.clone() {
                if let Ok(mut book) = self.book.try_lock() {
                    let order = Order {
                        id: order_id,
                        side: if quote_type.contains("bid") { Side::BUY } else { Side::SELL },
                        price: Some(order_id as u64), // dummy price for cancellation
                        quantity: 0,
                    };
                    if book.cancel_limit_order(order, 0).is_some() {
                        actions.push(format!("Cancelled {} quote (ID: {})", quote_type, order_id));
                    }
                }
            }
            self.active_quotes.clear();
            
            // Post new quotes (multiple levels)
            let quote_sizes = vec![100_000_000, 50_000_000, 25_000_000]; // 100, 50, 25 ETH
            
            for (i, size) in quote_sizes.iter().enumerate() {
                let level = i + 1;
                
                // Bid quote
                if let Ok(mut book) = self.book.try_lock() {
                    let bid_price = our_bid - (i as i64 * 100_000); // 10 cent increments
                    let req = OrderRequest {
                        side: Side::BUY,
                        price: Some(bid_price as u64),
                        quantity: *size,
                    };
                    let (order_id, _) = book.submit(&req);
                    self.active_quotes.insert(format!("bid_level_{}", level), order_id);
                    actions.push(format!("Posted bid level {}: ${:.2} @ {:.2} ETH", 
                        level, bid_price as f64 / 1_000_000.0, *size as f64 / 1_000_000.0));
                }
                
                // Ask quote  
                if let Ok(mut book) = self.book.try_lock() {
                    let ask_price = our_ask + (i as i64 * 100_000); // 10 cent increments
                    let req = OrderRequest {
                        side: Side::SELL,
                        price: Some(ask_price as u64),
                        quantity: *size,
                    };
                    let (order_id, _) = book.submit(&req);
                    self.active_quotes.insert(format!("ask_level_{}", level), order_id);
                    actions.push(format!("Posted ask level {}: ${:.2} @ {:.2} ETH", 
                        level, ask_price as f64 / 1_000_000.0, *size as f64 / 1_000_000.0));
                }
            }
        }
        
        actions
    }

    /// Check if external market has crossed our quotes and simulate fills
    pub fn check_crosses(&mut self, ext_bid: Option<(i64, u64)>, ext_ask: Option<(i64, u64)>) -> Vec<String> {
        let mut fills = Vec::new();
        
        // Get our best bid/ask from the book
        let our_best = if let Ok(book) = self.book.try_lock() {
            (book.best_bid(), book.best_ask())
        } else {
            return fills;
        };
        
        // Check if external market crossed our quotes
        if let (Some((our_bid_px, our_bid_qty)), Some((ext_ask_px, _))) = (our_best.0, ext_ask) {
            if ext_ask_px <= our_bid_px as i64 {
                // External ask crossed our bid - simulate a fill
                let fill_qty = std::cmp::min(our_bid_qty, 10_000_000); // Fill 10 ETH
                self.inventory += fill_qty as i64; // We bought, so inventory goes positive
                fills.push(format!("🔄 SIMULATED FILL: Bought {:.2} ETH at ${:.2} (external ask crossed our bid)", 
                    fill_qty as f64 / 1_000_000.0, our_bid_px as f64 / 1_000_000.0));
            }
        }
        
        if let (Some((our_ask_px, our_ask_qty)), Some((ext_bid_px, _))) = (our_best.1, ext_bid) {
            if ext_bid_px >= our_ask_px as i64 {
                // External bid crossed our ask - simulate a fill
                let fill_qty = std::cmp::min(our_ask_qty, 10_000_000); // Fill 10 ETH
                self.inventory -= fill_qty as i64; // We sold, so inventory goes negative
                fills.push(format!("🔄 SIMULATED FILL: Sold {:.2} ETH at ${:.2} (external bid crossed our ask)", 
                    fill_qty as f64 / 1_000_000.0, our_ask_px as f64 / 1_000_000.0));
            }
        }
        
        fills
    }

    pub fn get_status(&self) -> String {
        format!("Inventory: {:.2} ETH", self.inventory as f64 / 1_000_000.0)
    }
}
