//! CPU mining threads with live-adjustable thread count and duty cycle.

use crate::state::Shared;
use crate::work::{Hasher, top32_threshold, verify};
use std::sync::Arc;
use std::sync::atomic::Ordering::*;
use std::time::{Duration, Instant};

/// Hashes per chunk between control checks.
const CHUNK: u32 = 8192;

pub fn max_threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
}

pub fn spawn_cpu(shared: Arc<Shared>) {
    for idx in 0..max_threads() {
        let shared = shared.clone();
        std::thread::Builder::new()
            .name(format!("cpu-miner-{idx}"))
            .spawn(move || cpu_worker(shared, idx))
            .expect("spawn cpu miner");
    }
}

fn active(shared: &Shared, idx: usize) -> bool {
    !shared.paused.load(Relaxed)
        && shared.cpu_enabled.load(Relaxed)
        && idx < shared.cpu_threads.load(Relaxed)
}

fn cpu_worker(shared: Arc<Shared>, idx: usize) {
    'outer: loop {
        if !active(&shared, idx) {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }
        let Some((work, generation)) = shared.next_work() else {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        };
        let threshold = top32_threshold(work.pool_diff);
        let mut hasher = Hasher::new(&work);
        let mut nonce: u32 = 0;
        loop {
            let t0 = Instant::now();
            let end = nonce.saturating_add(CHUNK);
            for n in nonce..end {
                let st = hasher.hash(n);
                if st[7] == 0 || (threshold != 0 && st[7].swap_bytes() <= threshold) {
                    let (_, diff) = verify(&work, n);
                    shared.report_candidate(&work, n, diff);
                }
            }
            shared.hashes_cpu.fetch_add((end - nonce) as u64, Relaxed);
            if end == u32::MAX {
                continue 'outer; // nonce space exhausted -> new extranonce2
            }
            nonce = end;

            if shared.job_gen.load(Acquire) != generation || !active(&shared, idx) {
                continue 'outer;
            }
            // Duty-cycle throttling: work for t, sleep t * (100-u)/u.
            let usage = shared.cpu_usage.load(Relaxed).clamp(1, 100);
            if usage < 100 {
                let worked = t0.elapsed();
                std::thread::sleep(worked * (100 - usage) / usage);
            }
        }
    }
}

/// Computes hashrates once per second (exponential moving average).
pub fn spawn_stats(shared: Arc<Shared>) {
    std::thread::Builder::new()
        .name("stats".into())
        .spawn(move || {
            let mut last_cpu = 0u64;
            let mut last_gpu = 0u64;
            let mut last_t = Instant::now();
            let (mut ema_cpu, mut ema_gpu) = (0f64, 0f64);
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let dt = last_t.elapsed().as_secs_f64();
                last_t = Instant::now();
                let c = shared.hashes_cpu.load(Relaxed);
                let g = shared.hashes_gpu.load(Relaxed);
                let rc = (c - last_cpu) as f64 / dt;
                let rg = (g - last_gpu) as f64 / dt;
                last_cpu = c;
                last_gpu = g;
                ema_cpu = if ema_cpu == 0.0 { rc } else { ema_cpu * 0.7 + rc * 0.3 };
                ema_gpu = if ema_gpu == 0.0 { rg } else { ema_gpu * 0.7 + rg * 0.3 };
                if rc == 0.0 && shared.hashrate_cpu.get() < 1.0 { ema_cpu = 0.0; }
                if rg == 0.0 && shared.hashrate_gpu.get() < 1.0 { ema_gpu = 0.0; }
                shared.hashrate_cpu.set(ema_cpu);
                shared.hashrate_gpu.set(ema_gpu);
            }
        })
        .expect("spawn stats");
}
