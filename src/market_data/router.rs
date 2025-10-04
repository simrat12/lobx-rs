use crate::market_data::adapters::hyperliquid::HyperliquidAdapter;
use crate::market_data::adapters::{MarketEvent, VenueAdapter};
use crate::market_data::external_book::ExternalBook;
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex};
use tokio::time::{interval, Duration};

pub async fn run_demo() {
    println!("🚀 Market Data Demo: Live ETH/USDC from Hyperliquid");
    println!("{}", "=".repeat(60));
    
    // Create adapter and external book
    let adapter = HyperliquidAdapter::new("ETH", "ETH/USDC");
    let external = Arc::new(Mutex::new(ExternalBook::new()));
    let (tx, mut rx) = mpsc::channel::<MarketEvent>(1024);
    let ext_clone = external.clone();

    // Spawn adapter to stream live data
    tokio::spawn(async move {
        adapter.spawn(tx).await;
    });

    // Process market events and update external book
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let MarketEvent::Snapshot { bids, asks, .. } = ev;
            ext_clone.lock().unwrap().apply_snapshot(&bids, &asks);
        }
    });

    // Display BBO updates
    let mut ticker = interval(Duration::from_secs(1));
    for _ in 0..10 {
        ticker.tick().await;
        
        let (bid, ask) = external.lock().unwrap().bbo();
        if let (Some((bid_px, bid_sz)), Some((ask_px, ask_sz))) = (bid, ask) {
            let bid_price = bid_px as f64 / 1_000_000.0;
            let ask_price = ask_px as f64 / 1_000_000.0;
            let spread = ask_price - bid_price;
            let mid = (bid_price + ask_price) / 2.0;
            
            println!("📊 ETH/USDC: ${:.2} / ${:.2} (mid: ${:.2}, spread: ${:.3})", 
                     bid_price, ask_price, mid, spread);
        } else {
            println!("⏳ Waiting for market data...");
        }
    }
    
    println!("\n✅ Demo complete! Live market data streaming from Hyperliquid.");
}

pub async fn run_unified_demo() {
    println!("🚀 Advanced Trading System Demo: Market Making with Unified Book");
    println!("{}", "=".repeat(65));
    println!("This demonstrates a REAL market-making system that:");
    println!("  • Connects to live Hyperliquid market data");
    println!("  • Maintains an in-memory order book with multiple quote levels");
    println!("  • Simulates fills when external market crosses our quotes");
    println!("  • Adjusts quotes based on inventory risk");
    println!("  • Shows unified view combining external + internal liquidity\n");

    // Set up the system (quietly)
    let adapter = HyperliquidAdapter::new("ETH", "ETH/USDC");
    let external = Arc::new(Mutex::new(ExternalBook::new()));
    let internal = Arc::new(Mutex::new(crate::engine::book::Book::new()));
    let (tx, mut rx) = mpsc::channel::<MarketEvent>(1024);
    let ext_clone = external.clone();

    // Start getting live data (silently)
    tokio::spawn(async move { adapter.spawn(tx).await; });
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            let MarketEvent::Snapshot { bids, asks, .. } = ev;
            ext_clone.lock().unwrap().apply_snapshot(&bids, &asks);
        }
    });

    let unified = crate::market_data::unified_book::UnifiedBook::new(internal.clone(), external.clone(), 1_000_000);
    let mut market_maker = crate::market_data::market_maker::MarketMaker::new(internal.clone());
    
    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("📡 Connecting to live market data...");
    let (ext_bid, ext_ask) = external.lock().unwrap().bbo();
    if ext_bid.is_some() && ext_ask.is_some() {
        println!("✅ Connected! Starting market-making strategy...\n");
    } else {
        println!("⏳ Still connecting...\n");
    }

    let mut ticker = interval(Duration::from_secs(2));
    let mut demo_step = 0;
    let mut quotes_posted = false;

    loop {
        println!("🔍 Demo step {}: External data available: {}", demo_step, ext_bid.is_some() && ext_ask.is_some());
        ticker.tick().await;
        demo_step += 1;

        let (ext_bid, ext_ask) = {
            let external_book = external.lock().unwrap();
            external_book.bbo()
        };
        let (cmb_bid, cmb_ask) = unified.combined_bbo_with_source();

        if let (Some((bid_px, _bid_sz)), Some((ask_px, _ask_sz))) = (ext_bid, ext_ask) {
            let market_price = (bid_px + ask_px) as f64 / 2_000_000.0;
            let spread = (ask_px - bid_px) as f64 / 1_000_000.0;
            
            println!("📊 External Market: ${:.2} (spread: ${:.2})", market_price, spread);
            
            // Post initial quotes
            if !quotes_posted {
                quotes_posted = true;
                println!("\n🎯 MARKET MAKER: Posting 3-level quote ladder...");
                let actions = market_maker.update_quotes(ext_bid, ext_ask, 20); // 20 bps spread
                for action in actions {
                    println!("   {}", action);
                }
                println!("   📈 Strategy: Provide liquidity with 20bps spread, 3 levels deep");
            } else {
                // Update quotes every few iterations to show dynamic behavior
                if demo_step % 3 == 0 {
                    println!("\n🔄 MARKET MAKER: Updating quotes based on market conditions...");
                    let actions = market_maker.update_quotes(ext_bid, ext_ask, 20);
                    for action in actions.iter().take(3) { // Show first 3 actions
                        println!("   {}", action);
                    }
                }
                
                // Check for simulated fills
                let fills = market_maker.check_crosses(ext_bid, ext_ask);
                for fill in fills {
                    println!("   {}", fill);
                }
            }

            // Show unified view with source information
            if let (Some(cmb_bid), Some(cmb_ask)) = (cmb_bid, cmb_ask) {
                println!("\n💡 UNIFIED BOOK VIEW:");
                
                let bid_source = match cmb_bid.source {
                    crate::market_data::unified_book::PriceSource::External => "🌐 EXTERNAL (Hyperliquid)",
                    crate::market_data::unified_book::PriceSource::Internal => "🏠 INTERNAL (Our quotes)",
                };
                
                let ask_source = match cmb_ask.source {
                    crate::market_data::unified_book::PriceSource::External => "🌐 EXTERNAL (Hyperliquid)",
                    crate::market_data::unified_book::PriceSource::Internal => "🏠 INTERNAL (Our quotes)",
                };
                
                println!("   Best BUY:  ${:.2} @ {:.2} ETH {}", 
                    cmb_bid.price as f64 / 1_000_000.0, 
                    cmb_bid.size as f64 / 1_000_000.0,
                    bid_source);
                println!("   Best SELL: ${:.2} @ {:.2} ETH {}", 
                    cmb_ask.price as f64 / 1_000_000.0, 
                    cmb_ask.size as f64 / 1_000_000.0,
                    ask_source);
                
                // Show if our quotes are on top
                if matches!(cmb_bid.source, crate::market_data::unified_book::PriceSource::Internal) || 
                   matches!(cmb_ask.source, crate::market_data::unified_book::PriceSource::Internal) {
                    println!("   ✨ Our quotes are providing the best prices!");
                }
                
                // Show market maker status
                println!("   📊 {}", market_maker.get_status());
            }

            // Show depth view occasionally
            if demo_step % 4 == 0 {
                let (top_bids, top_asks) = unified.combined_depth_top_n(3);
                println!("\n📋 TOP 3 LEVELS (Combined External + Internal):");
                println!("   ASKS (Sell):");
                for (px, sz) in top_asks.iter().rev() {
                    println!("     ${:.2} @ {:.2} ETH", *px as f64 / 1_000_000.0, *sz as f64 / 1_000_000.0);
                }
                println!("   BIDS (Buy):");
                for (px, sz) in top_bids {
                    println!("     ${:.2} @ {:.2} ETH", px as f64 / 1_000_000.0, sz as f64 / 1_000_000.0);
                }
            }

        } else {
            println!("⏳ Waiting for market data...");
        }

        println!();

        if demo_step >= 12 {
            println!("🎉 DEMO COMPLETE!");
            println!("Key takeaways:");
            println!("  • In-memory book enables instant quote management (cancel/replace by ID)");
            println!("  • Unified view combines external + internal liquidity seamlessly");
            println!("  • Fill simulation shows realistic market-making behavior");
            println!("  • Inventory-aware quoting adjusts risk based on position");
            println!("  • Multiple quote levels provide depth and flexibility\n");
            break;
        }
    }
}