use std::env;
use std::io::{self, Write};
use lobx_rs::persist::postgres::PostgresSnapshotStore;
use lobx_rs::persist::postgres::PostgresWalStore;
use lobx_rs::persist::{SnapshotStore, WalStore};
use lobx_rs::persist::snapshot;
use lobx_rs::engine::book::Book;
use lobx_rs::engine::types::{OrderRequest, Side};

// Helper function to count total resting orders across both sides
fn count_resting_orders(book: &Book) -> (usize, usize, usize) {
    let mut bid_orders = 0;
    let mut ask_orders = 0;
    
    for queue in book.bids.values() {
        bid_orders += queue.len();
    }
    
    for queue in book.asks.values() {
        ask_orders += queue.len();
    }
    
    (book.bids.len(), book.asks.len(), bid_orders + ask_orders)
}

// Helper function to print state summary
fn print_state_summary(book: &Book) {
    let (bid_levels, ask_levels, total_orders) = count_resting_orders(book);
    
    println!("\n=== Book State Summary ===");
    println!("Bid levels: {}, Ask levels: {}, Total orders: {}", bid_levels, ask_levels, total_orders);
    
    if let Some((price, qty)) = book.best_bid() {
        println!("Best bid: {} @ {}", qty, price);
    } else {
        println!("Best bid: None");
    }
    
    if let Some((price, qty)) = book.best_ask() {
        println!("Best ask: {} @ {}", qty, price);
    } else {
        println!("Best ask: None");
    }
    
    if let Some(spread) = book.spread() {
        println!("Spread: {}", spread);
    } else {
        println!("Spread: N/A");
    }
    println!("========================\n");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok(); // load .env

    let db_url = env::var("DATABASE_URL")?;
    let symbol = env::var("LOBX_SYMBOL").unwrap_or_else(|_| "BTC-USD".to_string());

    // Build stores (both can share a PgPool internally)
    let mut snap_store = PostgresSnapshotStore::new(&db_url, &symbol).await;
    let wal_store = PostgresWalStore::new(&db_url, &symbol).await;

    // Make a fresh in-memory book
    let mut book = Book::new();

    // 1) Restore the latest snapshot (if any)
    if let Ok(Some(snapshot)) = snap_store.load_snapshot(&symbol).await {
        if let Err(e) = snapshot::apply_to_book(&mut book, &snapshot) {
            eprintln!("Error applying snapshot to book: {:?}", e);
            return Err(e.into());
        }
        let watermark = snapshot.wal_high_watermark;

        // 2) Relay (replay) all WAL ops after that watermark
        let ops = wal_store.relay_ops(watermark).await?;
        for (_id, op) in ops {
            snapshot::apply_op(&mut book, &op)?;
        }
        println!("Restored book from snapshot and replayed {} WAL operations", watermark);
    } else {
        println!("No snapshot found, starting with empty book");
    }

    // CLI loop
    loop {
        print!("\nLOBX CLI> ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let command = input.trim().to_lowercase();
        
        match command.as_str() {
            "help" | "h" => {
                println!("Available commands:");
                println!("  1, buy <price> <qty>  - Submit buy limit order");
                println!("  2, sell <price> <qty> - Submit sell limit order");
                println!("  3, market_buy <qty>    - Submit market buy order");
                println!("  4, market_sell <qty>   - Submit market sell order");
                println!("  top                    - Show top of book");
                println!("  snapshot              - Create and save snapshot");
                println!("  restore               - Load and apply latest snapshot");
                println!("  quit, q               - Exit");
            }
            "1" | "buy" => {
                println!("Enter price and quantity (e.g., '100 10'):");
                let mut order_input = String::new();
                io::stdin().read_line(&mut order_input)?;
                let parts: Vec<&str> = order_input.trim().split_whitespace().collect();
                if parts.len() == 2 {
                    if let (Ok(price), Ok(qty)) = (parts[0].parse::<u64>(), parts[1].parse::<u64>()) {
                        let req = OrderRequest { side: Side::BUY, price: Some(price), quantity: qty };
                        let (id, result) = book.submit(&req);
                        println!("Submitted buy order ID {}: {} @ {}", id, qty, price);
                        for event in result.events {
                            println!("  Event: {:?}", event);
                        }
                    } else { println!("Invalid numbers"); }
                } else { println!("Usage: price quantity"); }
            }
            "2" | "sell" => {
                println!("Enter price and quantity (e.g., '100 10'):");
                let mut order_input = String::new();
                io::stdin().read_line(&mut order_input)?;
                let parts: Vec<&str> = order_input.trim().split_whitespace().collect();
                if parts.len() == 2 {
                    if let (Ok(price), Ok(qty)) = (parts[0].parse::<u64>(), parts[1].parse::<u64>()) {
                        let req = OrderRequest { side: Side::SELL, price: Some(price), quantity: qty };
                        let (id, result) = book.submit(&req);
                        println!("Submitted sell order ID {}: {} @ {}", id, qty, price);
                        for event in result.events {
                            println!("  Event: {:?}", event);
                        }
                    } else { println!("Invalid numbers"); }
                } else { println!("Usage: price quantity"); }
            }
            "3" | "market_buy" => {
                println!("Enter quantity:");
                let mut qty_input = String::new();
                io::stdin().read_line(&mut qty_input)?;
                if let Ok(qty) = qty_input.trim().parse::<u64>() {
                    let req = OrderRequest { side: Side::BUY, price: None, quantity: qty };
                    let (id, result) = book.submit(&req);
                    println!("Submitted market buy order ID {}: {}", id, qty);
                    for event in result.events {
                        println!("  Event: {:?}", event);
                    }
                } else { println!("Invalid quantity"); }
            }
            "4" | "market_sell" => {
                println!("Enter quantity:");
                let mut qty_input = String::new();
                io::stdin().read_line(&mut qty_input)?;
                if let Ok(qty) = qty_input.trim().parse::<u64>() {
                    let req = OrderRequest { side: Side::SELL, price: None, quantity: qty };
                    let (id, result) = book.submit(&req);
                    println!("Submitted market sell order ID {}: {}", id, qty);
                    for event in result.events {
                        println!("  Event: {:?}", event);
                    }
                } else { println!("Invalid quantity"); }
            }
            "top" => {
                print_state_summary(&book);
            }
            "snapshot" => {
                println!("Creating snapshot...");
                let snap = snapshot::from_book(&book);
                if let Err(e) = snap_store.save_snapshot(&snap).await {
                    eprintln!("Error saving snapshot: {:?}", e);
                } else {
                    let (bid_levels, ask_levels, total_orders) = count_resting_orders(&book);
                    println!("✅ Saved snapshot for {}: {} bid levels, {} ask levels, {} total orders", 
                             symbol, bid_levels, ask_levels, total_orders);
                    print_state_summary(&book);
                }
            }
            "restore" => {
                println!("Loading latest snapshot...");
                if let Ok(Some(snap)) = snap_store.load_snapshot(&symbol).await {
                    book = Book::new(); // Clear current book
                    if let Err(e) = snapshot::apply_to_book(&mut book, &snap) {
                        eprintln!("Error applying snapshot: {:?}", e);
                    } else {
                        let (bid_levels, ask_levels, total_orders) = count_resting_orders(&book);
                        println!("✅ Restored snapshot for {}: {} bid levels, {} ask levels, {} total orders", 
                                 symbol, bid_levels, ask_levels, total_orders);
                        print_state_summary(&book);
                    }
                } else {
                    println!("❌ No snapshot found to restore");
                }
            }
            "quit" | "q" | "exit" => {
                println!("Goodbye!");
                break;
            }
            "" => continue,
            _ => {
                println!("Unknown command. Type 'help' for available commands.");
            }
        }
    }

    Ok(())
}
