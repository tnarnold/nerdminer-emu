use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub pool_host: String,
    pub pool_port: u16,
    pub btc_address: String,
    pub worker_name: String,
    pub pool_password: String,
    /// Difficulty suggested to the pool (`mining.suggest_difficulty`), 0 = don't send.
    pub suggest_diff: f64,
    pub cpu_enabled: bool,
    pub cpu_threads: usize,
    /// Duty cycle per thread, 1..=100 %.
    pub cpu_usage: u32,
    pub gpu_enabled: bool,
    /// Duty cycle of the GPU, 1..=100 %.
    pub gpu_usage: u32,
    pub always_on_top: bool,
    pub start_hidden: bool,
    pub start_paused: bool,
    /// Scale of the 320x170 screen.
    pub zoom: f32,
    /// Fields shown next to the tray icon in the macOS menu bar
    /// ("hashrate", "fee", "last_block", "best_diff", "price").
    pub tray_fields: Vec<String>,
    /// Transaction size (vB) used to estimate the cost of sending now.
    pub tx_vbytes: u32,
    /// Display currency for the BTC price and fee costs (ISO code, see `currency`).
    pub currency: String,
    /// UI language: "auto" (system), or a code from `i18n::Lang` ("pt-BR", "en").
    pub language: String,
}

impl Default for Config {
    fn default() -> Self {
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        Config {
            pool_host: "public-pool.io".into(),
            pool_port: 21496,
            btc_address: String::new(),
            worker_name: "nerdminer-emu".into(),
            pool_password: "x".into(),
            suggest_diff: 1.0,
            cpu_enabled: true,
            cpu_threads: (cores / 2).max(1),
            cpu_usage: 50,
            gpu_enabled: false,
            gpu_usage: 30,
            always_on_top: false,
            start_hidden: false,
            start_paused: false,
            zoom: 1.5,
            tray_fields: vec!["hashrate".into(), "fee".into()],
            tx_vbytes: 141,
            currency: crate::currency::default_for_system(),
            language: "auto".into(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        if let Ok(p) = std::env::var("NERDMINER_CONFIG") {
            return PathBuf::from(p);
        }
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nerdminer-emu")
            .join("config.json")
    }

    pub fn load() -> Config {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, s);
        }
    }

    pub fn username(&self) -> String {
        if self.worker_name.trim().is_empty() {
            self.btc_address.trim().to_string()
        } else {
            format!("{}.{}", self.btc_address.trim(), self.worker_name.trim())
        }
    }
}
