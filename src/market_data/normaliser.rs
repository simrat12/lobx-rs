// Convert wire strings into internal integer ticks/lots.
// Keep it SIMPLE for the demo: fixed scales.

pub struct Normaliser {
    pub price_scale: i64, // e.g. 1_000_000 => 6 decimal places
    pub size_scale: u64,  // e.g. 10^szDecimals (from SpotMeta)
}

impl Normaliser {
    pub fn new(price_scale: i64, size_decimals: u32) -> Self {
        let size_scale = 10u64.saturating_pow(size_decimals);
        Self { price_scale, size_scale }
    }

    pub fn price_to_ticks(&self, s: &str) -> i64 {
        // Parse decimal string and convert to ticks
        if let Some(dot_pos) = s.find('.') {
            let integer_part = &s[..dot_pos];
            let decimal_part = &s[dot_pos + 1..];
            
            // Parse integer part
            let integer: i64 = integer_part.parse().unwrap_or(0);
            
            // Parse decimal part and scale it
            let decimal: i64 = if decimal_part.is_empty() {
                0
            } else {
                // Pad or truncate decimal part to match our scale
                let scale_power = self.price_scale.to_string().len() - 1; // e.g., 1000000 -> 6
                let decimal_len = decimal_part.len();
                
                if decimal_len >= scale_power {
                    // Truncate if too long
                    decimal_part[..scale_power].parse().unwrap_or(0)
                } else {
                    // Pad with zeros if too short
                    let padded = format!("{:0<width$}", decimal_part, width = scale_power);
                    padded.parse().unwrap_or(0)
                }
            };
            
            integer * self.price_scale + decimal
        } else {
            // No decimal point, just integer
            s.parse::<i64>().unwrap_or(0) * self.price_scale
        }
    }

    pub fn size_to_lots(&self, s: &str) -> u64 {
        // Parse decimal string and convert to lots
        if let Some(dot_pos) = s.find('.') {
            let integer_part = &s[..dot_pos];
            let decimal_part = &s[dot_pos + 1..];
            
            // Parse integer part
            let integer: u64 = integer_part.parse().unwrap_or(0);
            
            // Parse decimal part and scale it
            let decimal: u64 = if decimal_part.is_empty() {
                0
            } else {
                // Pad or truncate decimal part to match our scale
                let scale_power = self.size_scale.to_string().len() - 1; // e.g., 1000 -> 3
                let decimal_len = decimal_part.len();
                
                if decimal_len >= scale_power {
                    // Truncate if too long
                    decimal_part[..scale_power].parse().unwrap_or(0)
                } else {
                    // Pad with zeros if too short
                    let padded = format!("{:0<width$}", decimal_part, width = scale_power);
                    padded.parse().unwrap_or(0)
                }
            };
            
            integer * self.size_scale + decimal
        } else {
            // No decimal point, just integer
            s.parse::<u64>().unwrap_or(0) * self.size_scale
        }
    }
}
