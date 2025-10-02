// Simple test to verify Hyperliquid WebSocket connectivity
// Run with: cargo run --example test_websocket

use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures::{SinkExt, StreamExt};
use serde_json;

#[tokio::main]
async fn main() {
    println!("🔍 Testing Hyperliquid WebSocket connectivity...");
    
    let url = "wss://api.hyperliquid.xyz/ws";
    println!("Connecting to: {}", url);
    
    match connect_async(url).await {
        Ok((ws_stream, response)) => {
            println!("✅ Successfully connected!");
            println!("Response status: {}", response.status());
            println!("Response headers: {:?}", response.headers());
            
            let (mut write, mut read) = ws_stream.split();
            
            // Send a simple ping message
            let ping_msg = serde_json::json!({
                "method": "ping"
            });
            
            println!("Sending ping message: {}", ping_msg);
            
            if let Err(e) = write.send(Message::Text(ping_msg.to_string())).await {
                eprintln!("Failed to send ping: {}", e);
                return;
            }
            
            println!("✅ Ping sent successfully!");
            
            // Try to read a response
            let timeout = tokio::time::Duration::from_secs(5);
            match tokio::time::timeout(timeout, read.next()).await {
                Ok(Some(msg)) => {
                    match msg {
                        Ok(Message::Text(text)) => {
                            println!("✅ Received response: {}", text);
                        }
                        Ok(Message::Close(_)) => {
                            println!("⚠️  Connection closed by server");
                        }
                        Ok(other) => {
                            println!("Received other message type: {:?}", other);
                        }
                        Err(e) => {
                            eprintln!("❌ Error reading message: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    println!("⚠️  No message received (connection closed)");
                }
                Err(_) => {
                    println!("⏰ Timeout waiting for response");
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Failed to connect: {}", e);
            
            // Provide troubleshooting tips
            println!("\n🔧 Troubleshooting tips:");
            println!("1. Check your internet connection");
            println!("2. Verify the WebSocket URL is correct");
            println!("3. Check if there are any firewall restrictions");
            println!("4. Try using a VPN if you're in a restricted region");
            println!("5. Check Hyperliquid's status page for any service outages");
        }
    }
}
