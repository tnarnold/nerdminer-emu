//! Bitcoin network info (price, height, hashrate, fees) from mempool.space.

use crate::state::Shared;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

fn get(url: &str) -> Option<Value> {
    let mut resp = ureq::get(url).call().ok()?;
    let body = resp.body_mut().read_to_string().ok()?;
    serde_json::from_str(&body).ok()
}

pub fn spawn(shared: Arc<Shared>) {
    std::thread::Builder::new()
        .name("netinfo".into())
        .spawn(move || {
            loop {
                let base = "https://mempool.space/api";
                let price = get(&format!("{base}/v1/prices")).and_then(|v| v["USD"].as_f64());
                let height = get(&format!("{base}/blocks/tip/height")).and_then(|v| v.as_u64());
                let hr = get(&format!("{base}/v1/mining/hashrate/3d"));
                let fees = get(&format!("{base}/v1/fees/recommended"));
                {
                    let mut n = shared.net.lock().unwrap();
                    if price.is_some() { n.btc_price_usd = price; }
                    if height.is_some() { n.block_height = height; }
                    if let Some(h) = &hr {
                        if let Some(x) = h["currentHashrate"].as_f64() { n.net_hashrate = Some(x); }
                        if let Some(x) = h["currentDifficulty"].as_f64() { n.net_difficulty = Some(x); }
                    }
                    if let Some(f) = &fees {
                        n.fee_fastest = f["fastestFee"].as_f64();
                        n.fee_hour = f["hourFee"].as_f64();
                        n.fee_half_hour = f["halfHourFee"].as_f64();
                    }
                }
                std::thread::sleep(Duration::from_secs(60));
            }
        })
        .expect("spawn netinfo");
}
