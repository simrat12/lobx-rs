// Simple standalone example to run the Hyperliquid market data demo
// Run with: cargo run --example market_data_demo

use lobx_rs::market_data::router;

#[tokio::main]
async fn main() {
    println!("🚀 Hyperliquid Market Data Demo");
    println!("================================");
    println!("This demo will:");
    println!("1. Connect to Hyperliquid WebSocket API");
    println!("2. Subscribe to ETH/USDC order book data");
    println!("3. Normalize price/size data to integer ticks/lots");
    println!("4. Display real-time Best Bid/Offer (BBO) updates");
    println!();
    println!("Press Ctrl+C to stop the demo");
    println!();
    
    // Run the market data demo
    router::run_demo().await;
}
