mod engine;
use engine::book::Book;
use engine::types::{Order, OrderRequest, Side};
use std::collections::HashMap;
use std::io::{self, Write};
use tracing_subscriber::EnvFilter;
use anyhow::Result;
use lobx_rs::telemetry;
use std::io::IsTerminal;
use std::time::Duration;
use std::thread;
use rand::Rng;

fn print_top(book: &Book) {
    let bb = book.best_bid().map(|(p,q)| format!("BID=({p}, {q})")).unwrap_or("BID=None".into());
    let ba = book.best_ask().map(|(p,q)| format!("ASK=({p}, {q})")).unwrap_or("ASK=None".into());
    let spread = book.spread().map(|s| format!("SPREAD={s}")).unwrap_or("SPREAD=None".into());
    println!("TOP: {bb}  {ba}  {spread}");
}

fn parse_side(s: &str) -> Option<Side> {
    match s.to_ascii_uppercase().as_str() {
        "BUY" => Some(Side::BUY),
        "SELL" => Some(Side::SELL),
        _ => None
    }
}

fn generate_random_price() -> i64 {
    let mut rng = rand::thread_rng();
    (rng.gen_range(10..110)) as i64
}

fn generate_random_quantity() -> u64 {
    let mut rng = rand::thread_rng();
    (rng.gen_range(10..110)) as u64
}

fn generate_random_side() -> Side {
    let mut rng = rand::thread_rng();
    if rng.gen_bool(0.5) {
        Side::BUY
    } else {
        Side::SELL
    }
}

fn generate_orders(book: &mut Book, order_history: &mut HashMap<u64, Order>, count: u32, order_type: &str) {
    println!("🚀 Generating {} {} orders...", count, order_type);
    
    for i in 1..=count {
        let side = generate_random_side();
        let quantity = generate_random_quantity();
        
        match order_type {
            "random" => {
                let mut rng = rand::thread_rng();
                let is_limit = rng.gen_bool(0.5);
                if is_limit {
                    let price = generate_random_price();
                    let req = OrderRequest { side, price: Some(price), quantity };
                    let (order_id, res) = book.submit(&req);
                    let o = Order { id: order_id, side, price: Some(price), quantity };
                    order_history.insert(order_id, o);
                    println!("Order {}: limit {:?} {} {}", order_id, side, price, quantity);
                } else {
                    let req = OrderRequest { side, price: None, quantity };
                    let (order_id, res) = book.submit(&req);
                    let o = Order { id: order_id, side, price: None, quantity };
                    order_history.insert(order_id, o);
                    println!("Order {}: market {:?} {}", order_id, side, quantity);
                }
            }
            "aggressive_buy" => {
                let price = 50 + i as i64 * 2; // Increasing prices
                let req = OrderRequest { side: Side::BUY, price: Some(price), quantity };
                let (order_id, res) = book.submit(&req);
                let o = Order { id: order_id, side: Side::BUY, price: Some(price), quantity };
                order_history.insert(order_id, o);
                println!("Order {}: limit BUY {} {}", order_id, price, quantity);
            }
            "aggressive_sell" => {
                let price = 150 - i as i64 * 2; // Decreasing prices
                let req = OrderRequest { side: Side::SELL, price: Some(price), quantity };
                let (order_id, res) = book.submit(&req);
                let o = Order { id: order_id, side: Side::SELL, price: Some(price), quantity };
                order_history.insert(order_id, o);
                println!("Order {}: limit SELL {} {}", order_id, price, quantity);
            }
            "market" => {
                let req = OrderRequest { side, price: None, quantity };
                let (order_id, res) = book.submit(&req);
                let o = Order { id: order_id, side, price: None, quantity };
                order_history.insert(order_id, o);
                println!("Order {}: market {:?} {}", order_id, side, quantity);
            }
            "spread" => {
                let buy_price = 50 + i as i64;
                let sell_price = 100 + i as i64;
                let buy_qty = generate_random_quantity();
                let sell_qty = generate_random_quantity();
                
                // Buy order
                let req = OrderRequest { side: Side::BUY, price: Some(buy_price), quantity: buy_qty };
                let (order_id, res) = book.submit(&req);
                let o = Order { id: order_id, side: Side::BUY, price: Some(buy_price), quantity: buy_qty };
                order_history.insert(order_id, o);
                println!("Order {}: limit BUY {} {}", order_id, buy_price, buy_qty);
                
                // Sell order
                let req = OrderRequest { side: Side::SELL, price: Some(sell_price), quantity: sell_qty };
                let (order_id, res) = book.submit(&req);
                let o = Order { id: order_id, side: Side::SELL, price: Some(sell_price), quantity: sell_qty };
                order_history.insert(order_id, o);
                println!("Order {}: limit SELL {} {}", order_id, sell_price, sell_qty);
            }
            _ => {}
        }
        
        // Small delay between orders
        thread::sleep(Duration::from_millis(50));
        
        if i % 10 == 0 {
            print_top(book);
        }
    }
    
    println!("✅ Generated {} {} orders! Check your Grafana dashboard at http://localhost:3000", count, order_type);
    print_top(book);
}

fn main() -> anyhow::Result<()> {
    telemetry::init_tracing("lobx_rs=info");
    telemetry::init_metrics();

    let mut book = Book::new();
    let mut order_history: HashMap<u64, Order> = HashMap::new();

    // Check if we're in automated mode (input from pipe)
    let is_automated = !io::stdin().is_terminal();
    
    if !is_automated {
        println!("LOBX Order Engine - Choose an option:");
        println!("  1. Generate 50 random orders (mixed types)");
        println!("  2. Generate 100 aggressive buy orders");
        println!("  3. Generate 100 aggressive sell orders");
        println!("  4. Generate 200 market orders");
        println!("  5. Generate 500 spread trading orders");
        println!("  6. Manual order entry");
        println!("  7. Show current book state");
        println!("  8. Quit");
        println!("");
        println!("Manual Commands (when in manual mode):");
        println!("  limit BUY  <price> <qty>");
        println!("  limit SELL <price> <qty>");
        println!("  market BUY  <qty>");
        println!("  market SELL <qty>");
        println!("  cancel <order_id>");
        println!("  top    (print best bid/ask)");
        println!("  quit");
    } else {
        println!("LOBX running in automated mode");
    }
    print_top(&book);

    let stdin = io::stdin();
    loop {
        if !is_automated {
            print!("> "); 
            let _ = io::stdout().flush();
        }
        let mut line = String::new();
        if stdin.read_line(&mut line).is_err() { break; }
        let t: Vec<_> = line.split_whitespace().collect();
        if t.is_empty() { continue; }
        
        // Handle menu options
        if t.len() == 1 {
            match t[0] {
                "1" => generate_orders(&mut book, &mut order_history, 50, "random"),
                "2" => generate_orders(&mut book, &mut order_history, 100, "aggressive_buy"),
                "3" => generate_orders(&mut book, &mut order_history, 100, "aggressive_sell"),
                "4" => generate_orders(&mut book, &mut order_history, 200, "market"),
                "5" => generate_orders(&mut book, &mut order_history, 500, "spread"),
                "6" => {
                    println!("Manual mode - enter orders directly:");
                    println!("  limit BUY  <price> <qty>");
                    println!("  limit SELL <price> <qty>");
                    println!("  market BUY  <qty>");
                    println!("  market SELL <qty>");
                    println!("  cancel <order_id>");
                    println!("  top    (print best bid/ask)");
                    println!("  quit");
                }
                "7" => print_top(&book),
                "8" | "quit" | "q" => break,
                _ => println!("Invalid option. Choose 1-8."),
            }
        } else {
            // Handle manual commands
            match t[0].to_ascii_lowercase().as_str() {
                "quit" | "q" => break,
                "top"        => { print_top(&book); }
                "limit" if t.len()==4 => {
                    if let (Some(side), Ok(px), Ok(q)) =
                        (parse_side(t[1]), t[2].parse::<i64>(), t[3].parse::<u64>()) {
                        let req = OrderRequest { side, price: Some(px), quantity: q };
                        let (order_id, res) = book.submit(&req);
                        let o = Order { id: order_id, side, price: Some(px), quantity: q };
                        order_history.insert(order_id, o);
                        println!("Order ID: {}, events: {:?}", order_id, res.events);
                        print_top(&book);
                    } else { println!("usage: limit BUY|SELL <price> <qty>"); }
                }
                "market" if t.len()==3 => {
                    if let (Some(side), Ok(q)) = (parse_side(t[1]), t[2].parse::<u64>()) {
                        let req = OrderRequest { side, price: None, quantity: q };
                        let (order_id, res) = book.submit(&req);
                        let o = Order { id: order_id, side, price: None, quantity: q };
                        order_history.insert(order_id, o);
                        println!("Order ID: {}, events: {:?}", order_id, res.events);
                        print_top(&book);
                    } else { println!("usage: market BUY|SELL <qty>"); }
                }
                "cancel" if t.len()==2 => {
                    if let Ok(order_id) = t[1].parse::<u64>() {
                        if let Some(original_order) = order_history.get(&order_id).cloned() {
                            let now = std::time::Instant::now();
                            let ts = now.elapsed().as_secs();
                            match book.cancel_limit_order(original_order, ts) {
                                Some(result) => {
                                    println!("events: {:?}", result.events);
                                    order_history.remove(&order_id);
                                }
                                None => println!("Order {} not found or already cancelled", order_id)
                            }
                            print_top(&book);
                        } else {
                            println!("Order {} not found in history", order_id);
                        }
                    } else { println!("usage: cancel <order_id>"); }
                }
                _ => println!("unknown cmd"),
            }
        }
    }
    
    Ok(())
}