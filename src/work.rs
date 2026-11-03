//! Bitcoin block header construction and SHA-256d helpers (stratum v1).

use sha2::block_api::compress256;
use sha2::{Digest, Sha256};

pub const IV: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Difficulty-1 target expressed as f64 (0xFFFF * 2^208).
pub const DIFF1: f64 = 65535.0 * 4.113761393303015e62; // 2^208 = 4.113761393303015e62

/// A job received from the pool via `mining.notify`.
#[derive(Clone, Debug)]
pub struct Job {
    pub job_id: String,
    pub prevhash: [u8; 32], // already in header byte order
    pub coinb1: Vec<u8>,
    pub coinb2: Vec<u8>,
    pub extranonce1: Vec<u8>,
    pub extranonce2_size: usize,
    pub merkle_branch: Vec<[u8; 32]>,
    pub version: u32,
    pub nbits: u32,
    pub ntime: u32,
    /// Share difficulty requested by the pool.
    pub pool_diff: f64,
}

/// One unit of work: a fixed extranonce2, scanning the 32-bit nonce space.
#[derive(Clone, Debug)]
pub struct WorkUnit {
    pub job_id: String,
    pub extranonce2: Vec<u8>,
    pub ntime: u32,
    pub header: [u8; 80],
    /// SHA-256 state after the first 64 bytes of the header.
    pub midstate: [u32; 8],
    pub pool_diff: f64,
    pub net_diff: f64,
}

pub fn sha256d(data: &[u8]) -> [u8; 32] {
    let first = Sha256::digest(data);
    Sha256::digest(first).into()
}

/// Convert stratum prevhash (8 words, each byte-swapped) into header order.
pub fn stratum_prevhash(hex_str: &str) -> Option<[u8; 32]> {
    let raw = hex::decode(hex_str).ok()?;
    if raw.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in raw.chunks(4).enumerate() {
        for j in 0..4 {
            out[i * 4 + j] = chunk[3 - j];
        }
    }
    Some(out)
}

/// Network difficulty from compact nbits.
pub fn nbits_to_diff(nbits: u32) -> f64 {
    let exp = (nbits >> 24) as i32;
    let mant = (nbits & 0x00ff_ffff) as f64;
    if mant == 0.0 {
        return 0.0;
    }
    let target = mant * 256f64.powi(exp - 3);
    DIFF1 / target
}

/// Difficulty of a hash (hash bytes in natural SHA output order, interpreted little-endian).
pub fn hash_diff(hash: &[u8; 32]) -> f64 {
    let mut v = 0f64;
    for b in hash.iter().rev() {
        v = v * 256.0 + *b as f64;
    }
    if v == 0.0 { f64::MAX } else { DIFF1 / v }
}

impl Job {
    pub fn make_work(&self, extranonce2_counter: u64) -> WorkUnit {
        let mut en2 = vec![0u8; self.extranonce2_size];
        let bytes = extranonce2_counter.to_be_bytes();
        let n = self.extranonce2_size.min(8);
        en2[self.extranonce2_size - n..].copy_from_slice(&bytes[8 - n..]);

        let mut coinbase =
            Vec::with_capacity(self.coinb1.len() + self.extranonce1.len() + en2.len() + self.coinb2.len());
        coinbase.extend_from_slice(&self.coinb1);
        coinbase.extend_from_slice(&self.extranonce1);
        coinbase.extend_from_slice(&en2);
        coinbase.extend_from_slice(&self.coinb2);

        let mut root = sha256d(&coinbase);
        for branch in &self.merkle_branch {
            let mut buf = [0u8; 64];
            buf[..32].copy_from_slice(&root);
            buf[32..].copy_from_slice(branch);
            root = sha256d(&buf);
        }

        let mut header = [0u8; 80];
        header[0..4].copy_from_slice(&self.version.to_le_bytes());
        header[4..36].copy_from_slice(&self.prevhash);
        header[36..68].copy_from_slice(&root);
        header[68..72].copy_from_slice(&self.ntime.to_le_bytes());
        header[72..76].copy_from_slice(&self.nbits.to_le_bytes());

        let mut midstate = IV;
        let mut first = [0u8; 64];
        first.copy_from_slice(&header[..64]);
        compress256(&mut midstate, &[first]);

        WorkUnit {
            job_id: self.job_id.clone(),
            extranonce2: en2,
            ntime: self.ntime,
            header,
            midstate,
            pool_diff: self.pool_diff,
            net_diff: nbits_to_diff(self.nbits),
        }
    }
}

/// Fast hasher for one work unit using the midstate.
pub struct Hasher {
    midstate: [u32; 8],
    block2: [u8; 64],
    block3: [u8; 64],
}

impl Hasher {
    pub fn new(w: &WorkUnit) -> Self {
        let mut block2 = [0u8; 64];
        block2[..16].copy_from_slice(&w.header[64..80]);
        block2[16] = 0x80;
        block2[62] = 0x02; // 640 bits
        block2[63] = 0x80;
        let mut block3 = [0u8; 64];
        block3[32] = 0x80;
        block3[62] = 0x01; // 256 bits
        block3[63] = 0x00;
        Hasher { midstate: w.midstate, block2, block3 }
    }

    /// Returns the final state words; word 7 == 0 means the top 32 bits are zero.
    #[inline]
    pub fn hash(&mut self, nonce: u32) -> [u32; 8] {
        self.block2[12..16].copy_from_slice(&nonce.to_le_bytes());
        let mut s1 = self.midstate;
        compress256(&mut s1, &[self.block2]);
        for (i, w) in s1.iter().enumerate() {
            self.block3[i * 4..i * 4 + 4].copy_from_slice(&w.to_be_bytes());
        }
        let mut s2 = IV;
        compress256(&mut s2, &[self.block3]);
        s2
    }
}

/// Candidate threshold on the most significant 32 bits of the hash
/// (`state[7].swap_bytes()`): hashes at or below it are checked on the CPU.
pub fn top32_threshold(pool_diff: f64) -> u32 {
    if pool_diff >= 1.0 {
        0
    } else {
        (1.0 / pool_diff.max(1e-9)).min(u32::MAX as f64) as u32
    }
}

#[cfg(test)]
pub fn state_to_hash(s: &[u32; 8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, w) in s.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&w.to_be_bytes());
    }
    out
}

/// Full (slow) verification of a nonce for a work unit.
pub fn verify(w: &WorkUnit, nonce: u32) -> ([u8; 32], f64) {
    let mut h = w.header;
    h[76..80].copy_from_slice(&nonce.to_le_bytes());
    let hash = sha256d(&h);
    (hash, hash_diff(&hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Bitcoin genesis block header.
    const GENESIS: &str = "0100000000000000000000000000000000000000000000000000000000000000000000003ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a29ab5f49ffff001d1dac2b7c";

    #[test]
    fn genesis_hash_and_midstate_path() {
        let raw = hex::decode(GENESIS).unwrap();
        let mut header = [0u8; 80];
        header.copy_from_slice(&raw);
        let mut hash = sha256d(&header);
        hash.reverse();
        assert_eq!(
            hex::encode(hash),
            "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"
        );

        let mut midstate = IV;
        let mut first = [0u8; 64];
        first.copy_from_slice(&header[..64]);
        compress256(&mut midstate, &[first]);
        let w = WorkUnit {
            job_id: String::new(),
            extranonce2: vec![],
            ntime: 0,
            header,
            midstate,
            pool_diff: 1.0,
            net_diff: 1.0,
        };
        let nonce = u32::from_le_bytes(header[76..80].try_into().unwrap());
        let mut hs = Hasher::new(&w);
        let st = hs.hash(nonce);
        assert_eq!(st[7], 0);
        assert_eq!(state_to_hash(&st), sha256d(&header));
        let d = hash_diff(&sha256d(&header));
        assert!(d > 2000.0 && d < 3000.0, "genesis diff {d}");
    }

    #[test]
    fn nbits_diff1() {
        assert!((nbits_to_diff(0x1d00ffff) - 1.0).abs() < 1e-9);
    }
}
