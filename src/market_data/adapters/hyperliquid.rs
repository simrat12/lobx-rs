// Hyperliquid adapter skeleton with CONCRETE endpoints & TODOs

use super::{MarketEvent, VenueAdapter};
use crate::market_data::normaliser::Normaliser;
use super::hyperliquid_types::{SpotMeta, WsBook, WsLevel, WsMessage};
use futures::{SinkExt, StreamExt};
use tokio_tungstenite;

pub struct HyperliquidAdapter {
    pub coin: String,     // e.g. "ETH"
    pub pair: String,     // e.g. "ETH/USDC" (used to find decimals)
    pub info_url: String, // "https://api.hyperliquid.xyz/info"
    pub ws_url: String,   // "wss://api.hyperliquid.xyz/ws"
}

impl HyperliquidAdapter {
    pub fn new(coin: &str, pair: &str) -> Self {
        Self {
            coin: coin.to_string(),
            pair: pair.to_string(),
            info_url: "https://api.hyperliquid.xyz/info".into(),
            ws_url: "wss://api.hyperliquid.xyz/ws".into(),
        }
    }

    // 1) REST POST /info {"type":"spotMeta"} to learn szDecimals
    async fn fetch_spot_meta(&self) -> anyhow::Result<SpotMeta> {
        // - POST self.info_url with JSON body: {"type":"spotMeta"}
        // - Deserialize into SpotMeta (use serde_json + reqwest)
        // - Return it
        let client = reqwest::Client::new();
        let res = client.post(&self.info_url)
            .json(&serde_json::json!({"type": "spotMeta"}))
            .send()
            .await
            .unwrap();

        let spot_meta: SpotMeta = res.json().await.unwrap();
        Ok(spot_meta)
    }

    // Helper: pick szDecimals for `self.pair` from SpotMeta
    fn sz_decimals_for_pair(&self, meta: &SpotMeta) -> u32 {
        // Find `SpotPair` where pair.name == self.pair
        for pair in &meta.universe {
            if pair.name == self.pair {
                // The pair has tokens: (base_idx, quote_idx)
                let base_idx = pair.tokens.0;
                // Find base token in meta.tokens[base_idx] and read szDecimals
                if let Some(token) = meta.tokens.get(base_idx as usize) {
                    return token.szDecimals;
                }
            }
        }
        // Default fallback
        6
    }

    // 2) WS subscribe to l2Book for the coin and read WsBook messages
    async fn stream_l2book(&self, normaliser: &Normaliser, tx: tokio::sync::mpsc::Sender<MarketEvent>) {
        println!("Attempting to connect to WebSocket: {}", self.ws_url);
        
        // Connect to self.ws_url with tokio-tungstenite
        match tokio_tungstenite::connect_async(&self.ws_url).await {
            Ok((ws_stream, response)) => {
                println!("✅ Successfully connected to WebSocket!");
                println!("Response status: {}", response.status());
                println!("Response headers: {:?}", response.headers());
                let (mut write, mut read) = ws_stream.split();
                
                // Send subscription message
                let subscribe_msg = serde_json::json!({
                    "method": "subscribe",
                    "subscription": {
                        "type": "l2Book",
                        "coin": self.coin
                    }
                });
                
                println!("Sending subscription message: {}", subscribe_msg);
                
                if let Err(e) = write.send(tokio_tungstenite::tungstenite::Message::Text(subscribe_msg.to_string())).await {
                    eprintln!("Failed to send subscription: {}", e);
                    return;
                }
                
                println!("✅ Subscription sent successfully!");
                
                // Read messages from websocket
                let mut message_count = 0;
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                            message_count += 1;
                            println!("Received message #{}: {}", message_count, text);
                            
                            // Try to deserialize WsMessage first
                            match serde_json::from_str::<WsMessage>(&text) {
                                Ok(ws_message) => {
                                    // Only process l2Book messages
                                    if ws_message.channel == "l2Book" {
                                        let ws_book = ws_message.data;
                                        
                                        // Normalize levels using Normaliser
                                        let bids = self.norm_side(normaliser, &ws_book.levels.0);
                                        let asks = self.norm_side(normaliser, &ws_book.levels.1);
                                        
                                        // Clone values for logging before moving them
                                        let coin = ws_book.coin.clone();
                                        let bid_count = bids.len();
                                        let ask_count = asks.len();
                                        
                                        let event = MarketEvent::Snapshot {
                                            coin: ws_book.coin,
                                            bids,
                                            asks,
                                            ts_ms: ws_book.time,
                                        };
                                        
                                        // Send event to router
                                        if let Err(e) = tx.send(event).await {
                                            eprintln!("Failed to send market event: {}", e);
                                            break;
                                        }
                                        
                                        println!("✅ Processed book update for {}: {} bids, {} asks", 
                                                 coin, bid_count, ask_count);
                                    } else {
                                        println!("📨 Received {} message (ignoring)", ws_message.channel);
                                    }
                                }
                                Err(e) => {
                                    println!("⚠️  Failed to parse as WsMessage: {}", e);
                                    println!("Raw message: {}", text);
                                }
                            }
                        }
                        Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => {
                            println!("WebSocket connection closed by server");
                            break;
                        }
                        Ok(other) => {
                            println!("Received non-text message: {:?}", other);
                        }
                        Err(e) => {
                            eprintln!("Error reading from WebSocket: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to connect to websocket: {}", self.ws_url);
                eprintln!("Error details: {}", e);
                
                // Try to provide more helpful error information
                if e.to_string().contains("Connection refused") {
                    eprintln!("💡 This might be a network connectivity issue or the server is down");
                } else if e.to_string().contains("timeout") {
                    eprintln!("💡 Connection timed out - the server might be slow to respond");
                } else if e.to_string().contains("TLS") {
                    eprintln!("💡 TLS/SSL connection issue - check if the certificate is valid");
                }
            }
        }
    }

    // Convert vector of WsLevel into normalized (price_ticks, size_lots)
    fn norm_side(&self, norm: &Normaliser, side: &[WsLevel]) -> Vec<(i64, u64)> {
        // IMPORTANT:
        // - px and sz are strings; convert using Normaliser methods.
        // - Decide tick/lot scale now (see Normaliser notes).
        side.iter()
            .map(|lvl| {
                let p = norm.price_to_ticks(&lvl.px); // i64
                let s = norm.size_to_lots(&lvl.sz);   // u64
                (p, s)
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl VenueAdapter for HyperliquidAdapter {
    async fn spawn(&self, tx: tokio::sync::mpsc::Sender<MarketEvent>) {
        // Step A: fetch spot meta (REST) to learn decimals for this pair
        let meta = match self.fetch_spot_meta().await {
            Ok(meta) => meta,
            Err(e) => {
                eprintln!("Failed to fetch spot meta: {}", e);
                return;
            }
        };
        
        let sz_dec = self.sz_decimals_for_pair(&meta);
        println!("Using size decimals: {} for pair: {}", sz_dec, self.pair);

        // Step B: construct a Normaliser with the decimals you need
        // Use 6 decimal places for price (typical for crypto prices)
        let price_scale = 1_000_000i64; // 6 decimal places
        let normaliser = Normaliser::new(price_scale, sz_dec);

        // Step C: open websocket and stream l2Book, emitting MarketEvent::Snapshot
        self.stream_l2book(&normaliser, tx).await;
    }
}
