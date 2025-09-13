mod engine;
use engine::book::Book;
use engine::types::{Order, Side};
use std::io::{self, Write};

fn print_top(book: &Book) {
    let bb = book.best_bid().map(|(p,q)| format!("BID=({p}, {q})")).unwrap_or("BID=None".into());
    let ba = book.best_ask().map(|(p,q)| format!("ASK=({p}, {q})")).unwrap_or("ASK=None".into());
    println!("TOP: {bb}  {ba}");
}

fn parse_side(s: &str) -> Option<Side> {
    match s.to_ascii_uppercase().as_str() {
        "BUY" => Some(Side::BUY),
        "SELL" => Some(Side::SELL),
        _ => None
    }
}

fn main() {
    let mut book = Book::new();
    let mut next_id: u64 = 1;

    println!("LOBX demo. Commands:");
    println!("  limit BUY  <price> <qty>");
    println!("  limit SELL <price> <qty>");
    println!("  market BUY  <qty>");
    println!("  market SELL <qty>");
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
                    next_id += 1;
                    let res = book.submit(o);
                    println!("events: {:?}", res.events);
                    print_top(&book);
                } else { println!("usage: limit BUY|SELL <price> <qty>"); }
            }
            "market" if t.len()==3 => {
                if let (Some(side), Ok(q)) = (parse_side(t[1]), t[2].parse::<u64>()) {
                    let o = Order { id: next_id, side, price: None, quantity: q };
                    next_id += 1;
                    let res = book.submit(o);
                    println!("events: {:?}", res.events);
                    print_top(&book);
                } else { println!("usage: market BUY|SELL <qty>"); }
            }
            _ => println!("unknown cmd"),
        }
    }
}
