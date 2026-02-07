use reqwest::blocking::Client;
use serde_json::Value;
use std::f64::consts::PI;
use std::thread;
use std::time::Duration;
use chrono::Utc;

#[derive(Debug, Copy, Clone)]
struct Quaternion {
    w: f64,
    x: f64,
    y: f64,
    z: f64,
}

impl Quaternion {
    fn new(w: f64, x: f64, y: f64, z: f64) -> Self {
        Quaternion { w, x, y, z }
    }

    fn norm(&self) -> f64 {
        (self.w.powi(2) + self.x.powi(2) + self.y.powi(2) + self.z.powi(2)).sqrt()
    }

    fn normalized(&self) -> Self {
        let n = self.norm();
        if n == 0.0 {
            Quaternion::new(1.0, 0.0, 0.0, 0.0)
        } else {
            Quaternion::new(self.w / n, self.x / n, self.y / n, self.z / n)
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    println!("BTC Quaternion Gauge Signal Monitor (CoinGecko Live Data)");
    println!("Updates every 45 seconds | Ctrl+C to stop\n");

    loop {
        let now = Utc::now();
        let timestamp_str = now.timestamp().to_string();
        let timestamp_display = now.format("%Y-%m-%d %H:%M:%S UTC").to_string();

        // Base URL with params + cache-busting timestamp
        let url = format!(
            "https://api.coingecko.com/api/v3/coins/bitcoin?localization=false&tickers=false&market_data=true&community_data=false&developer_data=false&sparkline=false&_={}",
            timestamp_str
        );

        let request = client.get(&url)
            .header("Cache-Control", "no-cache, no-store, must-revalidate")
            .header("Pragma", "no-cache")
            .header("Expires", "0")
            .header("User-Agent", "Rust BTC Monitor/1.0");

        match request.send() {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.json::<Value>() {
                        Ok(response) => {
                            // Handle potential API error object first
                            if response.get("error").is_some() {
                                println!("[{}] CoinGecko Error: {} (rate limited? retrying...)\n", timestamp_display, response["error"].as_str().unwrap_or("unknown"));
                                thread::sleep(Duration::from_secs(45));
                                continue;
                            }

                            let market_data = match response["market_data"].as_object() {
                                Some(md) => md,
                                None => {
                                    println!("[{}] No market_data in response (retrying...)\n", timestamp_display);
                                    thread::sleep(Duration::from_secs(45));
                                    continue;
                                }
                            };

                            let current_price = market_data["current_price"]["usd"].as_f64().unwrap_or(0.0);

                            let change_1h = market_data["price_change_percentage_1h_in_currency"]["usd"].as_f64().unwrap_or(0.0);
                            let change_24h = market_data["price_change_percentage_24h_in_currency"]["usd"].as_f64().unwrap_or(0.0);
                            let change_7d = market_data["price_change_percentage_7d_in_currency"]["usd"].as_f64().unwrap_or(0.0);
                            let change_14d = market_data["price_change_percentage_14d_in_currency"]["usd"].as_f64().unwrap_or(0.0);

                            let volume_24h = market_data["total_volume"]["usd"].as_f64().unwrap_or(0.0);

                            // Quaternion: real = primary (24h), imaginaries = phases
                            let q = Quaternion::new(change_24h, change_1h, change_7d, change_14d);

                            let norm = q.norm();
                            let unit_q = q.normalized();

                            let cos_half_angle = unit_q.w.clamp(-1.0, 1.0);
                            let half_angle_rad = cos_half_angle.acos();
                            let rotation_angle_deg = 2.0 * half_angle_rad * 180.0 / PI;

                            let alignment_strength = cos_half_angle.abs();
                            let direction = if cos_half_angle > 0.0 { "BUY" } else { "SELL" };

                            let signal = if norm > 15.0 && alignment_strength > 0.7 {
                                format!("STRONG {} (Low Gauge Curvature: {:.1}° alignment)", direction, rotation_angle_deg)
                            } else if norm > 15.0 && alignment_strength > 0.4 {
                                format!("{} (Moderate Curvature: {:.1}°)", direction, rotation_angle_deg)
                            } else if alignment_strength < 0.3 {
                                format!("HOLD - High Gauge Curvature ({:.1}° phase misalignment)", rotation_angle_deg)
                            } else {
                                "HOLD - Neutral/Low Momentum".to_string()
                            };

                            println!("[{}] Current BTC Price: ${:.2}", timestamp_display, current_price);
                            println!("Changes: 1h: {:.2}%, 24h: {:.2}%, 7d: {:.2}%, 14d: {:.2}%",
                                     change_1h, change_24h, change_7d, change_14d);
                            println!("24h Volume: ${:.0}", volume_24h);
                            println!("Norm: {:.2} | Curvature Angle: {:.1}°", norm, rotation_angle_deg);
                            println!("Signal: {}\n", signal);
                        }
                        Err(e) => {
                            println!("[{}] JSON parse error: {} (retrying...)\n", timestamp_display, e);
                        }
                    }
                } else {
                    println!("[{}] HTTP Error: {} (retrying...)\n", timestamp_display, resp.status());
                }
            }
            Err(e) => {
                println!("[{}] Network error: {} (retrying...)\n", timestamp_display, e);
            }
        }

        thread::sleep(Duration::from_secs(45));
    }
}
