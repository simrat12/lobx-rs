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
        // Connect to self.ws_url with tokio-tungstenite
        match tokio_tungstenite::connect_async(&self.ws_url).await {
            Ok((ws_stream, _response)) => {
                let (mut write, mut read) = ws_stream.split();
                
                // Send subscription message
                let subscribe_msg = serde_json::json!({
                    "method": "subscribe",
                    "subscription": {
                        "type": "l2Book",
                        "coin": self.coin
                    }
                });
                
                if let Err(_) = write.send(tokio_tungstenite::tungstenite::Message::Text(subscribe_msg.to_string())).await {
                    return;
                }
                
                // Read messages from websocket silently
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                            // Try to deserialize WsMessage first
                            if let Ok(ws_message) = serde_json::from_str::<WsMessage>(&text) {
                                // Only process l2Book messages
                                if ws_message.channel == "l2Book" {
                                    let ws_book = ws_message.data;
                                    
                                    // Normalize levels using Normaliser
                                    let bids = self.norm_side(normaliser, &ws_book.levels.0);
                                    let asks = self.norm_side(normaliser, &ws_book.levels.1);
                                    
                                    let event = MarketEvent::Snapshot {
                                        coin: ws_book.coin,
                                        bids,
                                        asks,
                                        ts_ms: ws_book.time,
                                    };
                                    
                                    // Send event to router
                                    if let Err(_) = tx.send(event).await {
                                        break;
                                    }
                                }
                            }
                        }
                        Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => {
                            break;
                        }
                        Ok(_) => {
                            // Ignore other message types
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }
            }
            Err(_) => {
                // Connection failed silently
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
            Err(_) => {
                return;
            }
        };
        
        let sz_dec = self.sz_decimals_for_pair(&meta);

        // Step B: construct a Normaliser with the decimals you need
        // Use 6 decimal places for price (typical for crypto prices)
        let price_scale = 1_000_000i64; // 6 decimal places
        let normaliser = Normaliser::new(price_scale, sz_dec);

        // Step C: open websocket and stream l2Book, emitting MarketEvent::Snapshot
        self.stream_l2book(&normaliser, tx).await;
    }
}
