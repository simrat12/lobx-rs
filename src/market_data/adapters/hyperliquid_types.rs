// Source: https://api.hyperliquid.xyz/ws (Subscriptions -> l2Book)
#[derive(Debug, serde::Deserialize)]
pub struct WsBook {
    pub coin: String,
    pub levels: (Vec<WsLevel>, Vec<WsLevel>), // (bids, asks)
    pub time: u64,
}

// Wrapper for the actual WebSocket message format
#[derive(Debug, serde::Deserialize)]
pub struct WsMessage {
    pub channel: String,
    pub data: WsBook,
}

#[derive(Debug, serde::Deserialize)]
pub struct WsLevel {
    pub px: String, // price string, e.g. "1234.56"
    pub sz: String, // size string, e.g. "0.01"
    pub n: u32,     // number of orders at this level
}

// Types for REST /info -> { "type": "spotMeta" }
#[derive(Debug, serde::Deserialize)]
pub struct SpotMeta {
    pub tokens: Vec<SpotToken>,
    pub universe: Vec<SpotPair>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SpotToken {
    pub name: String,
    pub szDecimals: u32,
    #[serde(default)]
    pub index: Option<u32>,
    // we ignore the other fields for now
}

#[derive(Debug, serde::Deserialize)]
pub struct SpotPair {
    pub name: String,     // e.g. "ETH/USDC"
    pub tokens: (u32, u32) // (base_token_index, quote_token_index)
    // we ignore the other fields for now
}
