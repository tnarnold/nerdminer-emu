// Double SHA-256 of a Bitcoin block header, one nonce per invocation.
// Uses the midstate of the first 64 header bytes (computed on the CPU).

struct Params {
    mid: array<u32, 8>,
    b0: u32,          // header bytes 64..68 (big-endian word)
    b1: u32,          // header bytes 68..72 (ntime)
    b2: u32,          // header bytes 72..76 (nbits)
    base: u32,        // first nonce of this dispatch
    threshold: u32,   // candidate threshold on the top 32 bits of the hash
};

struct Out {
    count: atomic<u32>,
    nonces: array<u32, 64>,
};

@group(0) @binding(0) var<storage, read> p: Params;
@group(0) @binding(1) var<storage, read_write> out: Out;

var<private> K: array<u32, 64> = array<u32, 64>(
    0x428a2f98u, 0x71374491u, 0xb5c0fbcfu, 0xe9b5dba5u, 0x3956c25bu, 0x59f111f1u, 0x923f82a4u, 0xab1c5ed5u,
    0xd807aa98u, 0x12835b01u, 0x243185beu, 0x550c7dc3u, 0x72be5d74u, 0x80deb1feu, 0x9bdc06a7u, 0xc19bf174u,
    0xe49b69c1u, 0xefbe4786u, 0x0fc19dc6u, 0x240ca1ccu, 0x2de92c6fu, 0x4a7484aau, 0x5cb0a9dcu, 0x76f988dau,
    0x983e5152u, 0xa831c66du, 0xb00327c8u, 0xbf597fc7u, 0xc6e00bf3u, 0xd5a79147u, 0x06ca6351u, 0x14292967u,
    0x27b70a85u, 0x2e1b2138u, 0x4d2c6dfcu, 0x53380d13u, 0x650a7354u, 0x766a0abbu, 0x81c2c92eu, 0x92722c85u,
    0xa2bfe8a1u, 0xa81a664bu, 0xc24b8b70u, 0xc76c51a3u, 0xd192e819u, 0xd6990624u, 0xf40e3585u, 0x106aa070u,
    0x19a4c116u, 0x1e376c08u, 0x2748774cu, 0x34b0bcb5u, 0x391c0cb3u, 0x4ed8aa4au, 0x5b9cca4fu, 0x682e6ff3u,
    0x748f82eeu, 0x78a5636fu, 0x84c87814u, 0x8cc70208u, 0x90befffau, 0xa4506cebu, 0xbef9a3f7u, 0xc67178f2u
);

fn rotr(x: u32, n: u32) -> u32 {
    return (x >> n) | (x << (32u - n));
}

fn bswap(x: u32) -> u32 {
    return ((x & 0xffu) << 24u) | ((x & 0xff00u) << 8u) | ((x >> 8u) & 0xff00u) | (x >> 24u);
}

fn compress(s: array<u32, 8>, block: array<u32, 16>) -> array<u32, 8> {
    var w: array<u32, 64>;
    for (var i = 0u; i < 16u; i++) {
        w[i] = block[i];
    }
    for (var i = 16u; i < 64u; i++) {
        let x = w[i - 15u];
        let y = w[i - 2u];
        let s0 = rotr(x, 7u) ^ rotr(x, 18u) ^ (x >> 3u);
        let s1 = rotr(y, 17u) ^ rotr(y, 19u) ^ (y >> 10u);
        w[i] = w[i - 16u] + s0 + w[i - 7u] + s1;
    }
    var a = s[0]; var b = s[1]; var c = s[2]; var d = s[3];
    var e = s[4]; var f = s[5]; var g = s[6]; var h = s[7];
    for (var i = 0u; i < 64u; i++) {
        let S1 = rotr(e, 6u) ^ rotr(e, 11u) ^ rotr(e, 25u);
        let ch = (e & f) ^ (~e & g);
        let t1 = h + S1 + ch + K[i] + w[i];
        let S0 = rotr(a, 2u) ^ rotr(a, 13u) ^ rotr(a, 22u);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = S0 + maj;
        h = g; g = f; f = e; e = d + t1;
        d = c; c = b; b = a; a = t1 + t2;
    }
    return array<u32, 8>(s[0] + a, s[1] + b, s[2] + c, s[3] + d, s[4] + e, s[5] + f, s[6] + g, s[7] + h);
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>) {
    let idx = gid.x + gid.y * nwg.x * 256u;
    let nonce = p.base + idx;

    let blk2 = array<u32, 16>(
        p.b0, p.b1, p.b2, bswap(nonce), 0x80000000u, 0u, 0u, 0u,
        0u, 0u, 0u, 0u, 0u, 0u, 0u, 640u
    );
    let s1 = compress(p.mid, blk2);

    let blk3 = array<u32, 16>(
        s1[0], s1[1], s1[2], s1[3], s1[4], s1[5], s1[6], s1[7],
        0x80000000u, 0u, 0u, 0u, 0u, 0u, 0u, 256u
    );
    let iv = array<u32, 8>(
        0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
        0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u
    );
    let s2 = compress(iv, blk3);

    if (s2[7] == 0u || (p.threshold != 0u && bswap(s2[7]) <= p.threshold)) {
        let slot = atomicAdd(&out.count, 1u);
        if (slot < 64u) {
            out.nonces[slot] = nonce;
        }
    }
}
