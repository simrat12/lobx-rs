mod engine;
use engine::book::Book;
use engine::types::{Order, Side};
use std::collections::HashMap;
use std::io::{self, Write};
use tracing_subscriber::EnvFilter;
use anyhow::Result;

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

fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("lobx_rs=info")); 
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .compact()
        .init();

    let mut book = Book::new();
    let mut next_id: u64 = 1;
    let mut order_history: HashMap<u64, Order> = HashMap::new();

    println!("LOBX demo. Commands:");
    println!("  limit BUY  <price> <qty>");
    println!("  limit SELL <price> <qty>");
    println!("  market BUY  <qty>");
    println!("  market SELL <qty>");
    println!("  cancel <order_id>");
    println!("  top    (print best bid/ask)");
    println!("  quit");
    print_top(&book);

    let stdin = io::stdin();
    loop {
        print!("> "); let _ = io::stdout().flush();
        let mut line = String::new();
        if stdin.read_line(&mut line).is_err() { break; }
        let t: Vec<_> = line.split_whitespace().collect();
        if t.is_empty() { continue; }
        match t[0].to_ascii_lowercase().as_str() {
            "quit" | "q" => break,
            "top"        => { print_top(&book); }
            "limit" if t.len()==4 => {
                if let (Some(side), Ok(px), Ok(q)) =
                    (parse_side(t[1]), t[2].parse::<i64>(), t[3].parse::<u64>()) {
                    let o = Order { id: next_id, side, price: Some(px), quantity: q };
                    order_history.insert(next_id, o.clone());
                    next_id += 1;
                    let res = book.submit(&o);
                    println!("events: {:?}", res.events);
                    print_top(&book);
                } else { println!("usage: limit BUY|SELL <price> <qty>"); }
            }
            "market" if t.len()==3 => {
                if let (Some(side), Ok(q)) = (parse_side(t[1]), t[2].parse::<u64>()) {
                    let o = Order { id: next_id, side, price: None, quantity: q };
                    next_id += 1;
                    let res = book.submit(&o);
                    println!("events: {:?}", res.events);
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
    
    Ok(())
}
