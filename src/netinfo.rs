//! Bitcoin network info from mempool.space (price, blocks, mempool, fees, difficulty)
//! plus BTC/BRL from Coinbase (fallback Binance), and the fee/congestion logic
//! shown in the tray menu.

use crate::state::{BlockInfo, DiffAdjustment, MempoolInfo, NetInfo, Shared};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

const BASE: &str = "https://mempool.space/api";
/// Max virtual size of a block, vB.
pub const BLOCK_VSIZE: f64 = 1_000_000.0;

fn get(url: &str) -> Option<Value> {
    let mut resp = ureq::get(url).call().ok()?;
    let body = resp.body_mut().read_to_string().ok()?;
    serde_json::from_str(&body).ok()
}

fn price_brl() -> Option<f64> {
    let num = |v: &Value| v.as_str().and_then(|s| s.parse::<f64>().ok());
    get("https://api.coinbase.com/v2/prices/BTC-BRL/spot")
        .and_then(|v| num(&v["data"]["amount"]))
        .or_else(|| get("https://api.binance.com/api/v3/ticker/price?symbol=BTCBRL").and_then(|v| num(&v["price"])))
}

/// vB of mempool transactions paying at least `min_fee` sat/vB.
/// `histogram` is mempool.space's `fee_histogram`: `[[feerate, vsize], ...]`.
pub fn backlog_vsize(histogram: &Value, min_fee: f64) -> f64 {
    histogram
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|e| Some((e[0].as_f64()?, e[1].as_f64()?)))
                .filter(|(rate, _)| *rate >= min_fee)
                .map(|(_, vs)| vs)
                .sum()
        })
        .unwrap_or(0.0)
}

fn parse_mempool(m: &Value, blocks: Option<&Value>, min_fee: f64) -> Option<MempoolInfo> {
    let next_block = blocks.and_then(|b| b.get(0)).and_then(|b| {
        let r = b["feeRange"].as_array()?;
        Some((
            r.first()?.as_f64()?,
            b["medianFee"].as_f64()?,
            r.last()?.as_f64()?,
            b["nTx"].as_u64()?,
        ))
    });
    Some(MempoolInfo {
        count: m["count"].as_u64()?,
        vsize: m["vsize"].as_f64()?,
        backlog_vsize: backlog_vsize(&m["fee_histogram"], min_fee),
        next_block,
    })
}

fn parse_block(b: &Value) -> Option<BlockInfo> {
    Some(BlockInfo {
        height: b["height"].as_u64()?,
        hash: b["id"].as_str()?.to_string(),
        pool: b["extras"]["pool"]["name"].as_str().unwrap_or("?").to_string(),
        time: b["timestamp"].as_u64()?,
    })
}

fn parse_diff_adj(d: &Value) -> Option<DiffAdjustment> {
    Some(DiffAdjustment {
        progress_pct: d["progressPercent"].as_f64()?,
        change_pct: d["difficultyChange"].as_f64()?,
        remaining_blocks: d["remainingBlocks"].as_u64()?,
        remaining_secs: d["remainingTime"].as_f64()? / 1000.0,
    })
}

pub fn spawn(shared: Arc<Shared>) {
    std::thread::Builder::new()
        .name("netinfo".into())
        .spawn(move || {
            let mut tick: u64 = 0;
            loop {
                // Fast-changing data every 30 s, the rest every 60 s.
                let fees = get(&format!("{BASE}/v1/fees/recommended"));
                let mempool = get(&format!("{BASE}/mempool"));
                let mblocks = get(&format!("{BASE}/v1/fees/mempool-blocks"));
                let blocks = get(&format!("{BASE}/v1/blocks"));
                let slow = tick % 2 == 0;
                let price = if slow { get(&format!("{BASE}/v1/prices")).and_then(|v| v["USD"].as_f64()) } else { None };
                let brl = if slow { price_brl() } else { None };
                let hr = if slow { get(&format!("{BASE}/v1/mining/hashrate/3d")) } else { None };
                let adj = if slow { get(&format!("{BASE}/v1/difficulty-adjustment")) } else { None };
                {
                    let mut n = shared.net.lock().unwrap();
                    if price.is_some() { n.btc_price_usd = price; }
                    if brl.is_some() { n.btc_price_brl = brl; }
                    if let Some(h) = &hr {
                        if let Some(x) = h["currentHashrate"].as_f64() { n.net_hashrate = Some(x); }
                        if let Some(x) = h["currentDifficulty"].as_f64() { n.net_difficulty = Some(x); }
                    }
                    if let Some(f) = &fees {
                        n.fee_fastest = f["fastestFee"].as_f64();
                        n.fee_half_hour = f["halfHourFee"].as_f64();
                        n.fee_hour = f["hourFee"].as_f64();
                        n.fee_economy = f["economyFee"].as_f64();
                        n.fee_minimum = f["minimumFee"].as_f64();
                    }
                    let min_fee = n.fee_minimum.unwrap_or(1.0);
                    if let Some(m) = mempool.as_ref().and_then(|m| parse_mempool(m, mblocks.as_ref(), min_fee)) {
                        n.mempool = Some(m);
                    }
                    if let Some(b) = blocks.as_ref().and_then(|b| b.get(0)).and_then(parse_block) {
                        if n.block_height.is_none_or(|h| b.height >= h) {
                            n.block_height = Some(b.height);
                        }
                        n.last_block = Some(b);
                    }
                    if let Some(a) = adj.as_ref().and_then(parse_diff_adj) {
                        n.diff_adj = Some(a);
                    }
                }
                tick += 1;
                std::thread::sleep(Duration::from_secs(30));
            }
        })
        .expect("spawn netinfo");
}

// ------------------------------------------------------------------ congestion / fees

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Congestion {
    Free,
    Moderate,
    Congested,
    Severe,
}

impl Congestion {
    /// From the backlog (in blocks) of transactions paying at least the minimum fee.
    pub fn from_backlog(blocks: f64) -> Self {
        if blocks <= 1.0 {
            Congestion::Free
        } else if blocks <= 6.0 {
            Congestion::Moderate
        } else if blocks <= 36.0 {
            Congestion::Congested
        } else {
            Congestion::Severe
        }
    }
    pub fn dot(self) -> &'static str {
        match self {
            Congestion::Free => "🟢",
            Congestion::Moderate => "🟡",
            Congestion::Congested => "🟠",
            Congestion::Severe => "🔴",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Congestion::Free => "Livre",
            Congestion::Moderate => "Moderada",
            Congestion::Congested => "Congestionada",
            Congestion::Severe => "Muito congestionada",
        }
    }
}

/// Fee tiers as offered by mempool.space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    NextBlock,
    HalfHour,
    Hour,
    Economy,
}

impl Tier {
    pub const ALL: [Tier; 4] = [Tier::NextBlock, Tier::HalfHour, Tier::Hour, Tier::Economy];
    pub fn label(self) -> &'static str {
        match self {
            Tier::NextBlock => "🚀 Próximo bloco (~10 min)",
            Tier::HalfHour => "⏱ ~30 min",
            Tier::Hour => "🕐 ~1 hora",
            Tier::Economy => "🐢 Sem pressa",
        }
    }
    pub fn fee(self, n: &NetInfo) -> Option<f64> {
        match self {
            Tier::NextBlock => n.fee_fastest,
            Tier::HalfHour => n.fee_half_hour,
            Tier::Hour => n.fee_hour,
            Tier::Economy => n.fee_economy,
        }
    }
}

pub fn congestion(n: &NetInfo) -> Option<(Congestion, f64)> {
    let m = n.mempool.as_ref()?;
    let blocks = m.backlog_vsize / BLOCK_VSIZE;
    Some((Congestion::from_backlog(blocks), blocks))
}

/// Recommended tier for the current congestion and the advice shown with it.
pub fn recommendation(c: Congestion) -> (Tier, &'static str) {
    match c {
        Congestion::Free => (Tier::Economy, "rede livre, não pague mais"),
        Congestion::Moderate => (Tier::HalfHour, "deve confirmar em ~30 min"),
        Congestion::Congested => (Tier::Hour, "taxa baixa pode demorar horas"),
        Congestion::Severe => (Tier::Hour, "rede cheia: envie só se for urgente"),
    }
}

/// Fraction (0..1) for the congestion bar: each level gets a quarter of the bar.
pub fn congestion_fraction(blocks: f64) -> f64 {
    let seg = |x: f64, lo: f64, hi: f64| ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    if blocks <= 1.0 {
        0.25 * seg(blocks, 0.0, 1.0)
    } else if blocks <= 6.0 {
        0.25 + 0.25 * seg(blocks, 1.0, 6.0)
    } else if blocks <= 36.0 {
        0.5 + 0.25 * seg(blocks, 6.0, 36.0)
    } else {
        0.75 + 0.25 * seg(blocks, 36.0, 216.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn backlog_counts_only_paying_txs() {
        let h = json!([[5.0, 50_000], [1.2, 300_000], [1.0, 500_000], [0.5, 2_000_000], [0.1, 9_000_000]]);
        assert_eq!(backlog_vsize(&h, 1.0), 850_000.0);
        assert_eq!(backlog_vsize(&h, 0.1), 11_850_000.0);
    }

    #[test]
    fn congestion_levels() {
        assert_eq!(Congestion::from_backlog(0.84), Congestion::Free);
        assert_eq!(Congestion::from_backlog(3.0), Congestion::Moderate);
        assert_eq!(Congestion::from_backlog(20.0), Congestion::Congested);
        assert_eq!(Congestion::from_backlog(80.0), Congestion::Severe);
        assert_eq!(recommendation(Congestion::Free).0, Tier::Economy);
    }

    #[test]
    fn congestion_bar_is_monotonic() {
        let mut last = -1.0;
        for i in 0..400 {
            let f = congestion_fraction(i as f64 * 0.75);
            assert!(f >= last && (0.0..=1.0).contains(&f));
            last = f;
        }
        assert!((congestion_fraction(1.0) - 0.25).abs() < 1e-9);
    }
}
