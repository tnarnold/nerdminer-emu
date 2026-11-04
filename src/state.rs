//! State shared between the UI, the stratum client and the miners.

use crate::config::Config;
use crate::work::{Job, WorkUnit};
use crossbeam_channel::{Receiver, Sender};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering::*};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

pub struct Submit {
    pub job_id: String,
    pub extranonce2: Vec<u8>,
    pub ntime: u32,
    pub nonce: u32,
    pub diff: f64,
}

#[derive(Default, Clone)]
pub struct NetInfo {
    /// BTC price per currency code ("USD", "BRL", "EUR", ...).
    pub prices: std::collections::BTreeMap<String, f64>,
    pub block_height: Option<u64>,
    pub net_hashrate: Option<f64>,
    pub net_difficulty: Option<f64>,
    pub fee_fastest: Option<f64>,
    pub fee_hour: Option<f64>,
    pub fee_half_hour: Option<f64>,
    pub fee_economy: Option<f64>,
    pub fee_minimum: Option<f64>,
    pub mempool: Option<MempoolInfo>,
    pub last_block: Option<BlockInfo>,
    pub diff_adj: Option<DiffAdjustment>,
}

impl NetInfo {
    pub fn price(&self, currency: &str) -> Option<f64> {
        self.prices.get(crate::currency::code(currency)).copied()
    }
}

#[derive(Default, Clone, Debug)]
pub struct MempoolInfo {
    pub count: u64,
    /// Total virtual size, vB.
    pub vsize: f64,
    /// vB of transactions paying at least the minimum recommended fee
    /// (the part that actually competes for block space).
    pub backlog_vsize: f64,
    /// Projected next block: (min, median, max) fee in sat/vB and tx count.
    pub next_block: Option<(f64, f64, f64, u64)>,
}

#[derive(Default, Clone, Debug)]
pub struct BlockInfo {
    pub height: u64,
    pub hash: String,
    pub pool: String,
    /// Unix seconds.
    pub time: u64,
}

#[derive(Default, Clone, Debug)]
pub struct DiffAdjustment {
    pub progress_pct: f64,
    pub change_pct: f64,
    pub remaining_blocks: u64,
    pub remaining_secs: f64,
}

pub struct AtomicF64(AtomicU64);
impl AtomicF64 {
    pub fn new(v: f64) -> Self {
        AtomicF64(AtomicU64::new(v.to_bits()))
    }
    pub fn get(&self) -> f64 {
        f64::from_bits(self.0.load(Relaxed))
    }
    pub fn set(&self, v: f64) {
        self.0.store(v.to_bits(), Relaxed)
    }
    /// Store `v` if greater; returns true if updated.
    pub fn fetch_max(&self, v: f64) -> bool {
        let mut cur = self.0.load(Relaxed);
        loop {
            if f64::from_bits(cur) >= v {
                return false;
            }
            match self.0.compare_exchange_weak(cur, v.to_bits(), Relaxed, Relaxed) {
                Ok(_) => return true,
                Err(x) => cur = x,
            }
        }
    }
}

pub struct Shared {
    pub config: RwLock<Config>,
    /// Bumped whenever pool settings change -> stratum reconnects.
    pub config_gen: AtomicU64,

    pub job: RwLock<Option<Arc<Job>>>,
    /// Bumped on every new job (miners restart their work).
    pub job_gen: AtomicU64,
    pub extranonce2_counter: AtomicU64,

    pub paused: AtomicBool,
    pub cpu_enabled: AtomicBool,
    pub cpu_threads: AtomicUsize,
    pub cpu_usage: AtomicU32,
    pub gpu_enabled: AtomicBool,
    pub gpu_usage: AtomicU32,

    // statistics
    pub started: Instant,
    pub hashes_cpu: AtomicU64,
    pub hashes_gpu: AtomicU64,
    pub hashrate_cpu: AtomicF64,
    pub hashrate_gpu: AtomicF64,
    pub templates: AtomicU64,
    pub shares32: AtomicU64,
    pub accepted: AtomicU64,
    pub rejected: AtomicU64,
    pub blocks: AtomicU64,
    pub best_diff: AtomicF64,
    pub pool_diff: AtomicF64,
    pub pool_status: Mutex<String>,
    pub pool_connected: AtomicBool,
    pub connected_since: Mutex<Option<Instant>>,
    pub gpu_status: Mutex<String>,
    pub net: Mutex<NetInfo>,
    pub log: Mutex<Vec<String>>,

    pub submit_tx: Sender<Submit>,
    pub submit_rx: Receiver<Submit>,
}

impl Shared {
    pub fn new(cfg: Config) -> Arc<Self> {
        let (submit_tx, submit_rx) = crossbeam_channel::unbounded();
        Arc::new(Shared {
            paused: AtomicBool::new(cfg.start_paused),
            cpu_enabled: AtomicBool::new(cfg.cpu_enabled),
            cpu_threads: AtomicUsize::new(cfg.cpu_threads),
            cpu_usage: AtomicU32::new(cfg.cpu_usage.clamp(1, 100)),
            gpu_enabled: AtomicBool::new(cfg.gpu_enabled),
            gpu_usage: AtomicU32::new(cfg.gpu_usage.clamp(1, 100)),
            config: RwLock::new(cfg),
            config_gen: AtomicU64::new(0),
            job: RwLock::new(None),
            job_gen: AtomicU64::new(0),
            extranonce2_counter: AtomicU64::new(0),
            started: Instant::now(),
            hashes_cpu: AtomicU64::new(0),
            hashes_gpu: AtomicU64::new(0),
            hashrate_cpu: AtomicF64::new(0.0),
            hashrate_gpu: AtomicF64::new(0.0),
            templates: AtomicU64::new(0),
            shares32: AtomicU64::new(0),
            accepted: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
            blocks: AtomicU64::new(0),
            best_diff: AtomicF64::new(0.0),
            pool_diff: AtomicF64::new(1.0),
            pool_status: Mutex::new("Desconectado".into()),
            pool_connected: AtomicBool::new(false),
            connected_since: Mutex::new(None),
            gpu_status: Mutex::new("Desligada".into()),
            net: Mutex::new(NetInfo::default()),
            log: Mutex::new(Vec::new()),
            submit_tx,
            submit_rx,
        })
    }

    pub fn log(&self, msg: impl Into<String>) {
        let msg = msg.into();
        eprintln!("{msg}");
        let mut l = self.log.lock().unwrap();
        l.push(msg);
        let n = l.len();
        if n > 200 {
            l.drain(..n - 200);
        }
    }

    pub fn set_status(&self, s: impl Into<String>) {
        *self.pool_status.lock().unwrap() = s.into();
    }

    /// Next work unit for the current job, plus its job generation.
    pub fn next_work(&self) -> Option<(WorkUnit, u64)> {
        let generation = self.job_gen.load(Acquire);
        let job = self.job.read().unwrap().clone()?;
        let en2 = self.extranonce2_counter.fetch_add(1, Relaxed);
        Some((job.make_work(en2), generation))
    }

    pub fn set_connected(&self, c: bool) {
        self.pool_connected.store(c, Relaxed);
        let mut since = self.connected_since.lock().unwrap();
        if !c {
            *since = None;
        } else if since.is_none() {
            *since = Some(Instant::now());
        }
    }

    pub fn hashrate(&self) -> f64 {
        self.hashrate_cpu.get() + self.hashrate_gpu.get()
    }

    pub fn total_hashes(&self) -> u64 {
        self.hashes_cpu.load(Relaxed) + self.hashes_gpu.load(Relaxed)
    }

    /// Called by miners for any hash with difficulty >= ~1 (top 32 bits zero) or better.
    pub fn report_candidate(&self, w: &WorkUnit, nonce: u32, diff: f64) {
        if diff >= 1.0 {
            self.shares32.fetch_add(1, Relaxed);
        }
        if self.best_diff.fetch_max(diff) {
            self.log(format!("Nova melhor dificuldade: {}", crate::fmt::diff(diff)));
        }
        if diff >= w.net_diff {
            self.blocks.fetch_add(1, Relaxed);
            self.log("!!! BLOCO VÁLIDO ENCONTRADO !!!");
        }
        if diff >= w.pool_diff {
            let _ = self.submit_tx.send(Submit {
                job_id: w.job_id.clone(),
                extranonce2: w.extranonce2.clone(),
                ntime: w.ntime,
                nonce,
                diff,
            });
        }
    }
}
