//! What the lab shows: real hashes the miners actually tested, sampled a few times per second.
//!
//! Lives only while the lab window is open. With no pool or while paused it replays the
//! nonces around the genesis block, so the lesson also works offline.

use super::text::LabText;
use crate::state::{NetInfo, Shared};
use crate::work::{Job, WorkUnit, genesis_header, nbits_to_diff, sha256d, verify};
use std::collections::VecDeque;
use std::sync::atomic::Ordering::*;
use std::time::{Duration, Instant};

pub const SAMPLE_EVERY: Duration = Duration::from_millis(125);
const NET_EVERY: Duration = Duration::from_secs(1);
pub const HISTORY: usize = 48;
const EVENTS: usize = 5;
/// The demo starts this many nonces before the genesis solution, reached in about 5 s.
const DEMO_LEAD: u32 = 40;
pub const GENESIS_NONCE: u32 = 2083236893;

/// Leading zero bits of a hash as displayed (the SHA output reversed).
pub fn zero_bits(hash: &[u8; 32]) -> u32 {
    let mut bits = 0;
    for b in hash.iter().rev() {
        if *b == 0 {
            bits += 8;
        } else {
            return bits + b.leading_zeros();
        }
    }
    bits
}

/// Zero bits a hash needs for a difficulty: difficulty 1 is about 32 bits.
pub fn diff_bits(d: f64) -> f64 {
    if d > 0.0 { (32.0 + d.log2()).max(0.0) } else { 0.0 }
}

/// Bit `i` of a hash, most significant displayed bit first.
pub fn bit(hash: &[u8; 32], i: usize) -> bool {
    hash[31 - i / 8] & (0x80 >> (i % 8)) != 0
}

pub fn with_nonce(header: &[u8; 80], nonce: u32) -> [u8; 80] {
    let mut h = *header;
    h[76..80].copy_from_slice(&nonce.to_le_bytes());
    h
}

/// Hashes of `nonce` and `nonce + 1`, and how many of the 256 bits differ.
pub fn avalanche(header: &[u8; 80], nonce: u32) -> ([u8; 32], [u8; 32], u32) {
    let a = sha256d(&with_nonce(header, nonce));
    let b = sha256d(&with_nonce(header, nonce.wrapping_add(1)));
    let flipped = a.iter().zip(&b).map(|(x, y)| (x ^ y).count_ones()).sum();
    (a, b, flipped)
}

/// Displayed form of a hash: bytes reversed.
pub fn rev_hex(b: &[u8]) -> String {
    let mut v = b.to_vec();
    v.reverse();
    hex::encode(v)
}

pub struct Sample {
    pub header: [u8; 80],
    pub nonce: u32,
    pub hash: [u8; 32],
    pub bits: u32,
}

pub struct Avalanche {
    pub a: [u8; 32],
    pub b: [u8; 32],
    pub nonce: u32,
    pub flipped: u32,
    pub at: Instant,
}

pub struct Live {
    pub demo: bool,
    pub sample: Sample,
    pub history: VecDeque<u32>,
    pub avalanche: Option<Avalanche>,
    pub net: NetInfo,
    pub hashrate: f64,
    pub total: u64,
    pub best: f64,
    pub pool_diff: f64,
    pub net_diff: f64,
    pub branches: usize,
    pub en2: u64,
    pub events: VecDeque<(Instant, String)>,
    pub block_at: Option<Instant>,
    /// Sweep of the 32-bit nonce space restarted on another extranonce2.
    work: Option<(u64, WorkUnit)>,
    prevhash: Option<[u8; 32]>,
    accepted: u64,
    demo_nonce: u32,
    last: Instant,
    last_net: Instant,
}

impl Live {
    pub fn new(shared: &Shared) -> Self {
        let header = genesis_header();
        let nonce = GENESIS_NONCE - DEMO_LEAD;
        let hash = sha256d(&with_nonce(&header, nonce));
        let now = Instant::now();
        Live {
            demo: true,
            sample: Sample { header, nonce, hash, bits: zero_bits(&hash) },
            history: VecDeque::with_capacity(HISTORY),
            avalanche: None,
            net: NetInfo::default(),
            hashrate: 0.0,
            total: 0,
            best: shared.best_diff.get(),
            pool_diff: 0.0,
            net_diff: 0.0,
            branches: 0,
            en2: 0,
            events: VecDeque::with_capacity(EVENTS),
            block_at: None,
            work: None,
            prevhash: shared.job.read().unwrap().as_ref().map(|j| j.prevhash),
            accepted: shared.accepted.load(Relaxed),
            demo_nonce: nonce,
            last: now - SAMPLE_EVERY,
            last_net: now - NET_EVERY,
        }
    }

    /// Hashes tested between two samples.
    pub fn hashes_per_sample(&self) -> f64 {
        self.hashrate * SAMPLE_EVERY.as_secs_f64()
    }

    pub fn press_avalanche(&mut self) {
        let (a, b, flipped) = avalanche(&self.sample.header, self.sample.nonce);
        self.avalanche = Some(Avalanche { a, b, nonce: self.sample.nonce, flipped, at: Instant::now() });
    }

    fn event(&mut self, text: String) {
        if let Some(last) = self.events.back_mut().filter(|e| e.1 == text) {
            last.0 = Instant::now();
            return;
        }
        if self.events.len() == EVENTS {
            self.events.pop_front();
        }
        self.events.push_back((Instant::now(), text));
    }

    pub fn tick(&mut self, shared: &Shared, tx: &LabText) {
        if self.last.elapsed() < SAMPLE_EVERY {
            return;
        }
        self.last = Instant::now();
        if self.last_net.elapsed() >= NET_EVERY {
            self.last_net = Instant::now();
            self.net = shared.net.lock().unwrap().clone();
        }
        self.hashrate = shared.hashrate();
        self.total = shared.total_hashes();
        // The stored value is only a default until a pool sets it.
        self.pool_diff = if shared.pool_connected.load(Relaxed) { shared.pool_diff.get() } else { 0.0 };
        self.net_diff = self.net.net_difficulty.unwrap_or(self.net_diff);

        let job = shared.job.read().unwrap().clone();
        if let Some(j) = &job {
            self.watch_job(j, shared, tx);
        }
        let packed = shared.live_sample.load(Relaxed);
        let mining = !shared.paused.load(Relaxed) && self.hashrate > 0.0 && packed != 0;
        match job {
            Some(job) if mining => self.real_sample(&job, packed, shared),
            _ => self.demo_sample(tx),
        }
        if self.history.len() == HISTORY {
            self.history.pop_front();
        }
        self.history.push_back(self.sample.bits);
    }

    fn watch_job(&mut self, job: &Job, shared: &Shared, tx: &LabText) {
        self.branches = job.merkle_branch.len();
        self.net_diff = nbits_to_diff(job.nbits);
        if self.prevhash.is_some_and(|p| p != job.prevhash) {
            self.block_at = Some(Instant::now());
            self.event(tx.ev_new_block.to_string());
        }
        self.prevhash = Some(job.prevhash);

        let accepted = shared.accepted.load(Relaxed);
        if accepted > self.accepted {
            self.event(tx.ev_share.to_string());
        }
        self.accepted = accepted;

        let best = shared.best_diff.get();
        if best > self.best {
            self.event(crate::i18n::fill(tx.ev_best, &[&crate::fmt::diff(best)]));
        }
        self.best = best;
    }

    fn real_sample(&mut self, job: &Job, packed: u64, shared: &Shared) {
        let (en2, nonce) = crate::state::unpack_sample(packed);
        let generation = shared.job_gen.load(Acquire);
        if !self.work.as_ref().is_some_and(|(g, w)| *g == generation && w.en2 == en2) {
            self.work = Some((generation, job.make_work(en2)));
        }
        let (_, work) = self.work.as_ref().expect("just set");
        let (hash, _) = verify(work, nonce);
        self.sample = Sample { header: with_nonce(&work.header, nonce), nonce, hash, bits: zero_bits(&hash) };
        self.en2 = en2;
        self.demo = false;
    }

    fn demo_sample(&mut self, tx: &LabText) {
        self.demo = true;
        self.work = None;
        self.demo_nonce = self.demo_nonce.wrapping_add(1);
        if self.demo_nonce > GENESIS_NONCE + DEMO_LEAD {
            self.demo_nonce = GENESIS_NONCE - DEMO_LEAD;
        }
        let header = with_nonce(&genesis_header(), self.demo_nonce);
        let hash = sha256d(&header);
        self.sample = Sample { header, nonce: self.demo_nonce, hash, bits: zero_bits(&hash) };
        self.en2 = 0;
        if self.demo_nonce == GENESIS_NONCE {
            self.block_at = Some(Instant::now());
            self.event(tx.ev_demo_block.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_bits_counts_displayed_prefix() {
        let mut h = [0xffu8; 32];
        assert_eq!(zero_bits(&h), 0);
        h[31] = 0;
        h[30] = 0x0f;
        assert_eq!(zero_bits(&h), 12);
        assert_eq!(zero_bits(&[0; 32]), 256);
    }

    #[test]
    fn genesis_solution_has_its_famous_zeros() {
        let hash = sha256d(&genesis_header());
        assert!(rev_hex(&hash).starts_with("000000000019d668"));
        assert_eq!(zero_bits(&hash), 43);
        assert_eq!(u32::from_le_bytes(genesis_header()[76..80].try_into().unwrap()), GENESIS_NONCE);
    }

    #[test]
    fn difficulty_maps_to_zero_bits() {
        assert_eq!(diff_bits(1.0), 32.0);
        assert_eq!(diff_bits(1024.0), 42.0);
        assert_eq!(diff_bits(0.0), 0.0);
    }

    #[test]
    fn bit_order_follows_the_displayed_hash() {
        let mut h = [0u8; 32];
        h[31] = 0x80;
        h[0] = 0x01;
        assert!(bit(&h, 0));
        assert!(!bit(&h, 1));
        assert!(bit(&h, 255));
    }

    #[test]
    fn one_nonce_flips_about_half_of_the_bits() {
        let header = genesis_header();
        let n = 2000u32;
        let total: u32 = (0..n).map(|i| avalanche(&header, i).2).sum();
        let mean = total as f64 / n as f64;
        assert!((mean - 128.0).abs() < 1.0, "mean {mean}");
    }

    #[test]
    fn sample_rebuilds_the_hash_the_miner_tested() {
        let job = Job {
            job_id: "1".into(),
            prevhash: [7; 32],
            coinb1: vec![1, 2, 3],
            coinb2: vec![4, 5],
            extranonce1: vec![9],
            extranonce2_size: 4,
            merkle_branch: vec![[3; 32]],
            version: 0x2000_0000,
            nbits: 0x1703_0000,
            ntime: 1_700_000_000,
            pool_diff: 1.0,
        };
        let (en2, nonce) = (42, 123_456);
        let (got_en2, got_nonce) = crate::state::unpack_sample(crate::state::pack_sample(en2, nonce));
        assert_eq!((got_en2, got_nonce), (en2, nonce));
        let work = job.make_work(got_en2);
        assert_eq!(verify(&work, got_nonce).0, sha256d(&with_nonce(&work.header, nonce)));
    }
}
