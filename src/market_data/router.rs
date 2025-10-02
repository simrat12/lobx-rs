// Router orchestrates adapter + book
use crate::market_data::adapters::{MarketEvent, VenueAdapter};
use crate::market_data::adapters::hyperliquid::HyperliquidAdapter;
use crate::market_data::external_book::ExternalBook;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::time::Duration;

pub async fn run_demo() {
    println!("Starting Hyperliquid market data demo...");
    
    // 1. Create adapter (HyperliquidAdapter) for ETH/USDC
    let adapter = HyperliquidAdapter::new("ETH", "ETH/USDC");
    
    // 2. Create external book to store normalized data
    let book = Arc::new(Mutex::new(ExternalBook::new()));
    
    // 3. Create channel for market events
    let (tx, mut rx) = mpsc::channel::<MarketEvent>(1000);
    
    // 4. Clone the book for the adapter task
    let book_clone = Arc::clone(&book);
    let adapter_task = tokio::spawn(async move {
        adapter.spawn(tx).await;
    });
    
    // 5. Spawn book update task
    let book_update_task = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                MarketEvent::Snapshot { coin, bids, asks, ts_ms } => {
                    let mut book_guard = book_clone.lock().unwrap();
                    book_guard.apply_snapshot(&bids, &asks);
                    println!("[{}] Updated book for {} with {} bids and {} asks", 
                             ts_ms, coin, bids.len(), asks.len());
                }
            }
        }
    });
    
    // 6. Print BBO every second
    let bbo_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let book_guard = book.lock().unwrap();
            let (best_bid, best_ask) = book_guard.bbo();
            
            match (best_bid, best_ask) {
                (Some((bid_price, bid_size)), Some((ask_price, ask_size))) => {
                    // Convert back to human-readable prices (divide by scale)
                    let bid_price_f64 = bid_price as f64 / 1_000_000.0;
                    let ask_price_f64 = ask_price as f64 / 1_000_000.0;
                    println!("BBO: BID {:.6} @ {:.6} | ASK {:.6} @ {:.6} | Spread: {:.6}", 
                             bid_price_f64, bid_size, ask_price_f64, ask_size, 
                             ask_price_f64 - bid_price_f64);
                }
                (Some((bid_price, bid_size)), None) => {
                    let bid_price_f64 = bid_price as f64 / 1_000_000.0;
                    println!("BBO: BID {:.6} @ {} | ASK: None", bid_price_f64, bid_size);
                }
                (None, Some((ask_price, ask_size))) => {
                    let ask_price_f64 = ask_price as f64 / 1_000_000.0;
                    println!("BBO: BID: None | ASK {:.6} @ {}", ask_price_f64, ask_size);
                }
                (None, None) => {
                    println!("BBO: No data available");
                }
            }
        }
    });
    
    // Wait for all tasks to complete (they run indefinitely)
    tokio::select! {
        _ = adapter_task => println!("Adapter task completed"),
        _ = book_update_task => println!("Book update task completed"),
        _ = bbo_task => println!("BBO display task completed"),
    }
}
