//! The eight stations of the assembly line, as machines driven by live data.
//!
//! All of them share one language: ceramic, graphite and steel, and a single accent on the
//! part that is working right now.

use super::live::{GENESIS_NONCE, Live, bit, diff_bits, rev_hex};
use super::model::*;
use super::scene::{V3, v3};
use super::studio::Look;
use super::text::LabText;
use crate::i18n::fill;
use eframe::egui::{Color32, Vec2};
use std::f32::consts::{PI, TAU};
use std::time::Duration;

pub const COUNT: usize = 8;
pub const SPACING: f32 = 32.0;
/// Half length of a station along the line, where the connecting track starts and ends.
const REACH: f32 = 13.0;

pub const ORANGE: Color32 = Color32::from_rgb(247, 147, 26);

/// (start, end) byte of each header field.
const FIELD_BYTES: [(usize, usize); 6] = [(0, 4), (4, 36), (36, 68), (68, 72), (72, 76), (76, 80)];
const FIELD_SECONDS: f32 = 3.0;

const DRUM_R: f32 = MODULE * Z_DRUM as f32 / 2.0;
const IDLER_R: f32 = MODULE * Z_IDLER as f32 / 2.0;
const DRUM_X: [f32; 2] = [-9.4, -9.4 + 2.0 * (DRUM_R + IDLER_R)];
const GEAR_Y: f32 = 3.32;
const BOARD_POS: V3 = v3(4.75, 1.9, 0.0);
const BOARD_TILT: f32 = 0.30;
const PITCH: f32 = 0.62;

pub fn origin(i: usize) -> V3 {
    v3(i as f32 * SPACING, 0.0, 0.0)
}

/// The header field highlighted at time `t`.
pub fn active_field(t: f32) -> usize {
    (t / FIELD_SECONDS) as usize % FIELD_BYTES.len()
}

/// Per-tile state of the hash board, eased so it breathes instead of flickering.
pub struct Board {
    heights: [f32; 256],
    glow: [f32; 256],
}

impl Board {
    pub fn new() -> Self {
        Board { heights: [0.0; 256], glow: [0.0; 256] }
    }
}

pub struct Ctx<'a> {
    pub k: &'a mut Kit,
    /// Seconds since the window opened.
    pub t: f32,
    /// Seconds since the last frame.
    pub dt: f32,
    pub live: &'a Live,
    pub tx: &'a LabText,
    pub look: Look,
    pub board: &'a mut Board,
    /// Detailed labels, for the station in focus.
    pub detail: bool,
}

/// Deterministic pseudo-random in 0..1, so the scene does not flicker between frames.
fn rnd(i: u32) -> f32 {
    let x = i.wrapping_add(1).wrapping_mul(2654435761).rotate_left(13).wrapping_mul(2246822519);
    (x >> 8) as f32 / (1u32 << 24) as f32
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn box_at(k: &mut Kit, p: V3, h: V3, mat: Mat) {
    k.add(BOX, mul(at(p), scale(h.x, h.y, h.z)), mat);
}

fn crate_at(k: &mut Kit, p: V3, s: f32, mat: Mat) {
    k.add(CRATE, mul(at(p), scale(s, s, s)), mat);
}

fn bolts(k: &mut Kit, frame: M, x: f32, z: f32, y: f32) {
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        k.add(BOLT, mul(frame, at(v3(x * sx, y, z * sz))), STEEL);
    }
}

/// Base plate of a station, `w` by `d` long, top face at height 0.34.
fn bed(k: &mut Kit, p: V3, w: f32, d: f32) {
    let frame = at(p + v3(0.0, 0.17, 0.0));
    k.add(BED, mul(frame, scale(w / 5.6, 1.0, d / 3.0)), GRAPHITE);
    bolts(k, frame, w - 0.35, d - 0.35, 0.22);
}

/// Thin rings on the floor and the track that leads to the next station.
pub fn floor(c: &mut Ctx, i: usize) {
    let o = origin(i);
    for r in [7.0, 13.5] {
        c.k.add(RING, mul(at(o + v3(-0.5, 0.0, 0.5)), scale(r, 1.0, r)), INK);
    }
    if i + 1 == COUNT {
        return;
    }
    let (from, to) = (o.x + REACH, o.x + SPACING - REACH);
    let (mid, half) = ((from + to) / 2.0, (to - from) / 2.0);
    for z in [-0.55, 0.55] {
        box_at(c.k, v3(mid, 0.1, z), v3(half, 0.05, 0.05), BRUSHED);
    }
    for k in 0..7 {
        let x = from + (k as f32 + 0.5) * (to - from) / 7.0;
        box_at(c.k, v3(x, 0.05, 0.0), v3(0.09, 0.05, 0.8), DULL);
    }
    for k in 0..2 {
        let x = from + ((c.t * 0.4 + k as f32 * 0.5 + i as f32 * 0.13) % 1.0) * (to - from);
        crate_at(c.k, v3(x, 0.42, 0.0), 0.28, ACCENT);
    }
}

pub fn draw(i: usize, c: &mut Ctx) {
    match i {
        0 => mempool(c),
        1 => merkle(c),
        2 => header(c),
        3 => nonce(c),
        4 => sha(c),
        5 => target(c),
        6 => race(c),
        _ => chain(c),
    }
}

fn mempool(c: &mut Ctx) {
    let o = origin(0);
    const SLOTS: u32 = 48;
    const R: f32 = 2.0;
    let period = 7.0;
    let phase = (c.t % period) / period;
    bed(c.k, o + v3(-3.6, 0.0, 0.0), 6.0, 4.4);
    bed(c.k, o + v3(6.2, 0.0, 0.0), 3.0, 3.0);
    // The block being filled: an open steel cage.
    let cage = o + v3(6.2, 0.34 + R, 0.0);
    for (x, z) in [(-R, -R), (R, -R), (R, R), (-R, R)] {
        c.k.bar(cage + v3(x, -R, z), cage + v3(x, R, z), 0.07, STEEL);
        c.k.add(NUT, mul(at(cage + v3(x, R - 0.05, z)), scale(0.3, 0.3, 0.3)), ANODIZED);
    }
    for y in [-R, R] {
        for k in 0..4 {
            let (a, b) = ([(-R, -R), (R, -R), (R, R), (-R, R)][k], [(R, -R), (R, R), (-R, R), (-R, -R)][k]);
            c.k.bar(cage + v3(a.0, y, a.1), cage + v3(b.0, y, b.1), 0.07, STEEL);
        }
    }
    for i in 0..SLOTS {
        let (ix, iy, iz) = (i % 6, i / 24, (i / 6) % 4);
        let fee = rnd(i);
        let size = 0.3 + rnd(i + 99) * 0.3;
        let home = o + v3(-7.4 + ix as f32 * 1.5, 0.95 + iy as f32 * 1.45, -2.2 + iz as f32 * 1.48);
        let mat = blend(if fee < 0.4 { DULL } else { CERAMIC }, ACCENT, (fee - 0.45) * 2.2);
        let mut p = home + v3(0.0, (c.t * 1.3 + i as f32).sin() * 0.04, 0.0);
        if fee > 0.72 {
            let slot = cage + v3(-1.3 + rnd(i + 7) * 2.6, -1.3 + rnd(i + 8) * 2.6, -1.3 + rnd(i + 9) * 2.6);
            let go = smooth((phase - 0.1 - (1.0 - fee)) * 2.6);
            let back = smooth((phase - 0.85) * 7.0);
            let k = go * (1.0 - back);
            p = p.lerp(slot, k) + v3(0.0, (k * PI).sin() * 3.0, 0.0);
        }
        crate_at(c.k, p, size, mat);
    }
    if c.detail {
        if let Some(m) = &c.live.net.mempool {
            c.k.callout(o + v3(-3.6, 3.6, -1.0), Vec2::new(-20.0, -70.0), fill(c.tx.s_tx, &[&crate::fmt::num(m.count as f64, 0)]), false);
        }
        c.k.callout(cage + v3(0.0, R, -R), Vec2::new(24.0, -60.0), c.tx.s_next_block, true);
    }
}

fn merkle(c: &mut Ctx) {
    let o = origin(1);
    let levels = 4usize;
    let phase = (c.t % 8.0) / 8.0 * (levels as f32 + 1.0);
    let node = |level: usize, i: usize| -> V3 {
        let n = 8usize >> level;
        let gap = 1.9 * (1 << level) as f32;
        o + v3((i as f32 - (n as f32 - 1.0) / 2.0) * gap, 1.0 + level as f32 * 2.3, 0.0)
    };
    bed(c.k, o + v3(0.0, 0.0, 0.0), 8.4, 2.2);
    for level in 0..levels {
        let n = 8usize >> level;
        let on = smooth(phase - level as f32);
        for i in 0..n {
            let p = node(level, i);
            let root = level + 1 == levels;
            let lit = if root { ACCENT } else { CERAMIC };
            let mut mat = blend(Mat { color: 0x4A4E57, ..DULL }, lit, on);
            if level == 0 && i == 0 {
                mat = Mat { glow: 0.25, ..ACCENT };
            }
            crate_at(c.k, p, 0.5 + level as f32 * 0.08, mat);
            if level + 1 < levels {
                let parent = node(level + 1, i / 2);
                c.k.bar(p, parent, 0.05, BRUSHED);
                let run = phase - level as f32 - 0.5;
                if (0.0..1.0).contains(&run) {
                    c.k.add(BEAD, mul(at(p.lerp(parent, run)), scale(0.15, 0.15, 0.15)), Mat { glow: 3.0, ..ACCENT });
                }
            }
        }
    }
    if c.detail {
        let root = rev_hex(&c.live.sample.header[36..68]);
        c.k.callout(node(3, 0) + v3(0.0, 0.74, 0.0), Vec2::new(26.0, -36.0), format!("{}: {}…", c.tx.s_merkle_root, &root[..12]), true);
        c.k.callout(node(0, 0) + v3(0.0, -0.3, 0.5), Vec2::new(30.0, 58.0), c.tx.s_coinbase, true);
    }
}

fn header(c: &mut Ctx) {
    let o = origin(2);
    let h = &c.live.sample.header;
    const COLS: usize = 20;
    let active = active_field(c.t);
    bed(c.k, o, 8.2, 3.6);
    let place = |byte: usize| o + v3(-7.2 + (byte % COLS) as f32 * 0.76, 0.34, -2.25 + (byte / COLS) as f32 * 1.5);
    for (f, (a, b)) in FIELD_BYTES.iter().enumerate() {
        for byte in *a..*b {
            let height = 0.14 + h[byte] as f32 / 255.0 * 2.4;
            let mat = if f == active { Mat { glow: 0.2, ..ACCENT } } else if f % 2 == 0 { CERAMIC } else { Mat { color: 0xA9ACB3, ..CERAMIC } };
            box_at(c.k, place(byte) + v3(0.0, height / 2.0, 0.0), v3(0.31, height / 2.0, 0.6), mat);
        }
    }
    if c.detail {
        // Each label hangs from the edge of the plate next to its field, clear of the columns.
        let edge = |byte: usize, dz: f32| place(byte) + v3(0.0, 0.06, dz);
        let spots = [
            (edge(1, -0.62), Vec2::new(-24.0, -64.0)),
            (edge(12, -0.62), Vec2::new(24.0, -64.0)),
            (edge(59, 0.0) + v3(0.42, 0.0, 0.0), Vec2::new(34.0, -30.0)),
            (edge(69, 0.62), Vec2::new(-34.0, 40.0)),
            (edge(73, 0.62), Vec2::new(-10.0, 66.0)),
            (edge(77, 0.62), Vec2::new(34.0, 40.0)),
        ];
        for (f, (at, leader)) in spots.into_iter().enumerate() {
            c.k.callout(at, leader, c.tx.fields[f], f == active);
        }
    }
}

/// Segments a..g of a hex digit.
fn segments(d: u32) -> u8 {
    [0x7E, 0x30, 0x6D, 0x79, 0x33, 0x5B, 0x5F, 0x70, 0x7F, 0x7B, 0x77, 0x1F, 0x4E, 0x3D, 0x4F, 0x47][d as usize & 15]
}

fn nonce(c: &mut Ctx) {
    let o = origin(3);
    let n = c.live.sample.nonce;
    let moving = c.live.hashrate > 0.0 || c.live.demo;
    bed(c.k, o + v3(0.0, 0.0, -2.4), 8.6, 2.0);
    let (axis_y, axis_z) = (2.75, -2.4);
    for x in [-7.8, 7.8] {
        box_at(c.k, o + v3(x, 1.55, axis_z), v3(0.16, 1.21, 0.7), ANODIZED);
    }
    c.k.bar(o + v3(-8.0, axis_y, axis_z), o + v3(8.0, axis_y, axis_z), 0.17, STEEL);
    let panel = mul(at(o + v3(0.0, 1.45, 2.3)), rot_x(0.75));
    c.k.add(BED, mul(panel, scale(1.5, 0.5, 0.6)), Mat { rough: 0.8, ..GRAPHITE });
    bolts(c.k, panel, 7.95, 1.45, 0.11);
    for (i, d) in format!("{n:08x}").chars().enumerate() {
        let x = -6.3 + i as f32 * 1.8;
        let digit = d.to_digit(16).unwrap_or(0);
        // The left wheels sit on their real digit; the right ones run too fast to read.
        let speed = (i as f32 / 7.0).powi(3) * 9.0;
        let spin = if moving { c.t * speed } else { 0.0 } + digit as f32 * TAU / 16.0;
        let wheel = mul(mul(at(o + v3(x, axis_y, axis_z)), rot_x(-spin)), rot_z(PI / 2.0));
        c.k.add(WHEEL, wheel, if i % 2 == 0 { ANODIZED } else { BRUSHED });
        c.k.add(BOLT, mul(wheel, at(v3(0.86, 0.47, 0.0))), Mat { glow: 0.5, ..ACCENT });
        let seg = segments(digit);
        let bars: [(f32, f32, bool); 7] = [
            (0.0, -0.8, true),
            (0.42, -0.4, false),
            (0.42, 0.4, false),
            (0.0, 0.8, true),
            (-0.42, 0.4, false),
            (-0.42, -0.4, false),
            (0.0, 0.0, true),
        ];
        for (k, (sx, sz, flat)) in bars.iter().enumerate() {
            let on = seg & (0x40 >> k) != 0;
            let size = if *flat { v3(0.34, 0.03, 0.07) } else { v3(0.07, 0.03, 0.34) };
            let mat = if on { Mat { glow: 1.1, ..ACCENT } } else { Mat { color: 0x2F3238, ..GRAPHITE } };
            c.k.add(BOX, mul(panel, mul(at(v3(x + sx, 0.115, *sz)), scale(size.x, size.y, size.z))), mat);
        }
    }
    let frac = n as f32 / u32::MAX as f32;
    box_at(c.k, o + v3(0.0, 0.14, 5.6), v3(7.6, 0.14, 0.3), GRAPHITE);
    box_at(c.k, o + v3(-7.4 + 7.4 * frac, 0.24, 5.6), v3((7.4 * frac).max(0.03), 0.1, 0.2), Mat { glow: 0.3, ..ACCENT });
    if c.detail {
        c.k.callout(o + v3(-5.0, 0.3, 5.9), Vec2::new(26.0, 58.0), c.tx.s_nonce_sweep, false);
    }
}

fn sha(c: &mut Ctx) {
    let (live, t) = (c.live, c.t);
    let o = origin(4) + v3(1.5, 0.0, -1.0);
    let world = at(o);
    let place = |p: V3| mul(world, at(p));
    let bed_x = (DRUM_X[0] + DRUM_X[1]) / 2.0;
    bed(c.k, o + v3(bed_x, 0.0, 0.0), 5.6, 3.0);
    let spin = t * 0.9;
    for (k, x) in DRUM_X.iter().enumerate() {
        let base = v3(*x, 0.34, 0.0);
        c.k.add(PEDESTAL, place(base), ANODIZED);
        c.k.add(SHELL, place(base + v3(0.0, 0.28, 0.0)), CERAMIC);
        c.k.add(SHAFT, mul(place(v3(*x, 2.1, 0.0)), scale(0.2, 1.95, 0.2)), STEEL);
        for j in 0..9 {
            let dir = if j % 2 == 0 { 1.0 } else { -1.0 };
            let a = spin * dir * (2.2 + j as f32 * 0.12) + j as f32 * 0.7 + k as f32;
            c.k.add(ROTOR, mul(place(v3(*x, 0.85 + j as f32 * 0.235, 0.0)), rot_y(a)), if j % 2 == 0 { STEEL } else { ANODIZED });
        }
        c.k.add(GEAR_DRUM, mul(place(v3(*x, GEAR_Y, 0.0)), rot_y(-spin)), ANODIZED);
        c.k.add(NUT, place(v3(*x, GEAR_Y + 0.14, 0.0)), STEEL);
        for b in 0..6 {
            let a = b as f32 * TAU / 6.0 - spin;
            c.k.add(BOLT, mul(place(v3(*x + a.cos() * 1.05, GEAR_Y + 0.2, a.sin() * 1.05)), rot_y(-a)), STEEL);
        }
    }
    // The idler sits half a tooth off, so the teeth really mesh.
    let idler_x = DRUM_X[0] + DRUM_R + IDLER_R;
    let idler = spin * Z_DRUM as f32 / Z_IDLER as f32 - PI / Z_IDLER as f32;
    c.k.add(GEAR_IDLER, mul(place(v3(idler_x, GEAR_Y, 0.0)), rot_y(idler)), BRUSHED);
    c.k.add(SHAFT, mul(place(v3(idler_x, (GEAR_Y + 0.34) / 2.0, 0.0)), scale(0.22, (GEAR_Y - 0.34) / 2.0, 0.22)), ANODIZED);
    c.k.add(NUT, place(v3(idler_x, GEAR_Y + 0.14, 0.0)), STEEL);

    // Header cartridge on its rails.
    let rail_x = DRUM_X[0];
    for dx in [-0.75, 0.75] {
        c.k.add(BOX, mul(place(v3(rail_x + dx, 0.2, 5.3)), scale(0.08, 0.08, 2.9)), STEEL);
    }
    for z in [2.9, 7.7] {
        c.k.add(BOX, mul(place(v3(rail_x, 0.13, z)), scale(1.05, 0.13, 0.2)), GRAPHITE);
    }
    let cycle = (t / 3.2).fract();
    let slide = smooth((cycle - 0.1) / 0.35) * (1.0 - smooth((cycle - 0.72) / 0.25));
    let z = 6.7 - slide * 2.6;
    c.k.add(BOX, mul(place(v3(rail_x, 0.48, z)), scale(1.05, 0.2, 0.72)), ANODIZED);
    let mut cursor = -0.95;
    for (i, w) in [0.05, 0.4, 0.4, 0.05, 0.05, 0.05].iter().enumerate() {
        let m = mul(place(v3(rail_x + cursor + w, 0.7, z)), scale(*w, 0.025, 0.6));
        c.k.add(BOX, m, if i == 5 { Mat { glow: 0.6, ..ACCENT } } else { Mat { color: 0xB9BCC3, ..INK } });
        cursor += w * 2.0 + 0.03;
    }

    // The board of 256 bits.
    let frame = mul(place(BOARD_POS), rot_x(BOARD_TILT));
    c.k.add(BOARD, frame, GRAPHITE);
    bolts(c.k, frame, 5.1, 5.1, 0.21);
    for k in 0..16 {
        let p = (k as f32 - 7.5) * PITCH;
        let long = if k % 4 == 0 { 0.13 } else { 0.07 };
        c.k.add(BOX, mul(frame, mul(at(v3(p, 0.16, -5.28)), scale(0.012, 0.01, long))), INK);
        c.k.add(BOX, mul(frame, mul(at(v3(-5.28, 0.16, p)), scale(long, 0.01, 0.012))), INK);
    }
    for (x, z, h) in [(-4.2, -3.2, 3.0), (4.2, -3.2, 3.0), (-4.2, 3.2, 1.2), (4.2, 3.2, 1.2)] {
        c.k.add(SHAFT, mul(place(BOARD_POS + v3(x, -BOARD_POS.y + h / 2.0, z)), scale(0.16, h / 2.0, 0.16)), STEEL);
        c.k.add(NUT, mul(place(BOARD_POS + v3(x, -BOARD_POS.y, z)), scale(1.2, 0.5, 1.2)), ANODIZED);
    }
    let ava = live.avalanche.as_ref().filter(|a| a.at.elapsed() < Duration::from_secs(12));
    let since = ava.map_or(1e3, |a| a.at.elapsed().as_secs_f32());
    // The change spreads from where the pipe lands, so the eye follows the avalanche.
    let entry = v3(-5.2, 0.0, 0.3);
    for i in 0..256 {
        let (cx, row) = (i % 16, i / 16);
        let p = v3((cx as f32 - 7.5) * PITCH, 0.0, (row as f32 - 7.5) * PITCH);
        let delay = (p - entry).dot(p - entry).sqrt() * 0.055;
        let reached = since >= delay;
        let hash = match ava {
            Some(a) if reached => &a.b,
            Some(a) => &a.a,
            None => &live.sample.hash,
        };
        let on = bit(hash, i);
        let flipped = ava.is_some_and(|a| reached && bit(&a.a, i) != bit(&a.b, i));
        let target = if on { 1.0 } else { 0.0 };
        c.board.heights[i] += (target - c.board.heights[i]) * (1.0 - (-c.dt * 16.0).exp());
        c.board.glow[i] += (if flipped { 1.0 } else { 0.0 } - c.board.glow[i]) * (1.0 - (-c.dt * 7.0).exp());
        let pop = (1.0 - ((since - delay) / 0.45).clamp(0.0, 1.0)).powi(2) * if flipped && reached { 0.28 } else { 0.0 };
        let g = c.board.glow[i];
        let y = 0.17 + c.board.heights[i] * 0.2 + pop;
        let mat = if g > 0.04 {
            // On the pale studio a strong glow washes the accent out.
            let rest = if c.look == Look::Light { 0.12 } else { 0.25 };
            Mat { glow: rest * g + pop * 3.0, ..ACCENT }
        } else if on {
            CERAMIC
        } else {
            DULL
        };
        c.k.add(TILE, mul(frame, at(p + v3(0.0, y, 0.0))), mat);
    }

    // Pipe, and the packet running through it.
    c.k.add(PIPE, world, Mat { color: 0x2B2E35, metal: 0.2, rough: 0.4, glow: 0.0 });
    c.k.add(FLANGE, place(PIPE_PATH[0] - v3(0.0, 0.25, 0.0)), STEEL);
    c.k.add(FLANGE, mul(place(PIPE_PATH[4] + v3(0.0, -0.1, 0.0)), rot_x(PI)), STEEL);
    let pts = super::mesh::catmull(&PIPE_PATH, 6);
    let run = ((t / 3.2).fract() - 0.45) / 0.3;
    if (0.0..1.0).contains(&run) {
        let i = ((run * (pts.len() - 1) as f32) as usize).min(pts.len() - 1);
        c.k.add(BEAD, mul(place(pts[i]), scale(0.2, 0.2, 0.2)), Mat { glow: 4.0, ..ACCENT });
    }

    if c.detail {
        c.k.callout(o + v3(rail_x, 0.8, z), Vec2::new(70.0, 34.0), c.tx.l_header, false);
        c.k.callout(o + v3(bed_x, GEAR_Y + 0.3, 0.0), Vec2::new(24.0, -118.0), c.tx.l_rotors, false);
        let board = point(frame, v3(3.6, 0.5, -5.0));
        match ava {
            Some(a) => c.k.callout(board, Vec2::new(30.0, -46.0), fill(c.tx.s_flipped, &[&a.flipped]), true),
            None => c.k.callout(board, Vec2::new(30.0, -46.0), c.tx.l_board, false),
        }
    }
}

fn target(c: &mut Ctx) {
    let o = origin(5);
    let live = c.live;
    let scale_y = 0.092; // height per zero bit
    let col = o + v3(3.2, 0.0, -0.9);
    bed(c.k, o + v3(-1.2, 0.0, 0.0), 10.4, 2.8);
    c.k.bar(col + v3(-2.2, 0.3, -1.1), col + v3(-2.2, 9.0, -1.1), 0.1, STEEL);
    c.k.bar(col + v3(2.2, 0.3, -1.1), col + v3(2.2, 9.0, -1.1), 0.1, STEEL);
    c.k.bar(col + v3(-2.2, 0.3, 1.1), col + v3(-2.2, 9.0, 1.1), 0.1, STEEL);
    c.k.bar(col + v3(2.2, 0.3, 1.1), col + v3(2.2, 9.0, 1.1), 0.1, STEEL);
    for b in (0..=96).step_by(8) {
        let y = b as f32 * scale_y + 0.34;
        let long = b % 16 == 0;
        box_at(c.k, col + v3(-2.75, y, 1.1), v3(if long { 0.42 } else { 0.24 }, 0.025, 0.05), ANODIZED);
        if c.detail && long {
            c.k.tag(col + v3(-3.6, y, 1.1), b.to_string(), false);
        }
    }
    let levels = [
        (live.sample.bits as f64, c.tx.s_hash_now, CERAMIC, false),
        (diff_bits(live.best), c.tx.s_best, STEEL, false),
        (diff_bits(live.pool_diff), c.tx.s_pool, DULL, false),
        (diff_bits(live.net_diff), c.tx.s_network, Mat { glow: 0.3, ..ACCENT }, true),
    ];
    let mut used: Vec<f32> = Vec::with_capacity(levels.len());
    for (bits, name, mat, accent) in levels {
        if bits <= 0.0 {
            continue;
        }
        let y = bits as f32 * scale_y + 0.34;
        box_at(c.k, col + v3(0.0, y, 0.0), v3(2.15, 0.06, 1.05), mat);
        if c.detail {
            let mut lift = -14.0;
            while used.iter().any(|u| (u - (y * 26.0 - lift)).abs() < 19.0) {
                lift -= 19.0;
            }
            used.push(y * 26.0 - lift);
            c.k.callout(col + v3(2.15, y, 1.0), Vec2::new(30.0, lift), format!("{name}: {bits:.0}"), accent);
        }
    }
    // Recent samples: almost every hash stays near the floor.
    let bars = o + v3(-11.0, 0.0, 1.3);
    for (i, bits) in live.history.iter().enumerate() {
        let h = (*bits as f32 * scale_y).max(0.04) / 2.0;
        let newest = i + 1 == live.history.len();
        let x = i as f32 * 0.17;
        box_at(c.k, bars + v3(x, 0.34 + h, 0.3), v3(0.065, h, 0.32), if newest { Mat { glow: 0.5, ..ACCENT } } else { CERAMIC });
    }
    if c.detail {
        c.k.callout(bars + v3(3.0, 0.4, 1.3), Vec2::new(30.0, 56.0), c.tx.s_zero_bits, false);
    }
}

fn race(c: &mut Ctx) {
    let base = origin(6) + v3(-2.0, 0.0, 0.0);
    let o = base + v3(0.0, 5.0, 0.0);
    const N: u32 = 200;
    let golden = PI * (3.0 - 5f32.sqrt());
    let spin = c.t * 0.2;
    c.k.add(PEDESTAL, at(base), ANODIZED);
    c.k.bar(base + v3(0.0, 0.2, 0.0), o, 0.28, STEEL);
    c.k.add(BEAD, mul(at(o), scale(2.1, 2.1, 2.1)), GRAPHITE);
    for i in 0..N {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / N as f32;
        let r = (1.0 - y * y).sqrt();
        let a = i as f32 * golden + spin;
        let dir = v3(a.cos() * r, y, a.sin() * r);
        let radius = 4.0 + (c.t * 1.1 + rnd(i) * 6.0).sin() * 0.14;
        crate_at(c.k, o + dir * radius, 0.15, if i % 3 == 0 { STEEL } else { CERAMIC });
        c.k.bar(o + dir * 2.0, o + dir * radius, 0.02, BRUSHED);
    }
    let stand = origin(6) + v3(7.6, 0.0, 3.4);
    let you = stand + v3(0.0, 2.1, 0.0);
    c.k.add(PEDESTAL, mul(at(stand), scale(0.32, 1.0, 0.32)), ANODIZED);
    c.k.bar(stand, you, 0.05, STEEL);
    c.k.add(BEAD, mul(at(you), scale(0.14, 0.14, 0.14)), Mat { glow: 3.0, ..ACCENT });
    for r in [0.5, 0.85] {
        let pulse = r + (c.t * 2.0).sin().abs() * 0.12;
        c.k.add(RING, mul(mul(at(you), rot_x(PI / 2.0)), scale(pulse, 1.0, pulse)), Mat { glow: 1.2, ..ACCENT });
    }
    if c.detail {
        c.k.callout(you, Vec2::new(30.0, -46.0), c.tx.s_you, true);
        c.k.callout(o + v3(1.0, 4.2, 0.0), Vec2::new(-30.0, -36.0), c.tx.s_network_cloud, false);
        if let Some(net) = c.live.net.net_hashrate.filter(|_| c.live.hashrate > 0.0) {
            c.k.callout(stand + v3(0.0, 0.3, 0.0), Vec2::new(26.0, 36.0), fill(c.tx.s_one_in, &[&crate::fmt::big(net / c.live.hashrate)]), false);
        }
    }
}

fn chain(c: &mut Ctx) {
    let o = origin(7);
    const BLOCKS: i32 = 5;
    let period = 9.0;
    let flash = c.live.block_at.map_or(0.0, |t| (1.0 - t.elapsed().as_secs_f32() / 4.0).max(0.0));
    let slide = smooth(((c.t % period) / period - 0.8) * 5.0);
    let height = c.live.net.last_block.as_ref().map(|b| b.height);
    bed(c.k, o, 9.6, 2.2);
    for i in 0..BLOCKS {
        let x = -7.0 + i as f32 * 3.5;
        let newest = i == BLOCKS - 1;
        let y = if newest { 1.5 + (1.0 - slide) * 4.0 } else { 1.5 };
        let mat = if newest { Mat { glow: 0.15 + flash * 1.5, ..ACCENT } } else { blend(DULL, CERAMIC, 0.45 + i as f32 * 0.18) };
        crate_at(c.k, o + v3(x, y, 0.0), 1.16, mat);
        if i > 0 && (!newest || slide > 0.95) {
            let mid = o + v3(x - 1.75, 1.5, 0.0);
            for (dx, m) in [(-0.22, rot_x(PI / 2.0)), (0.22, mul(rot_y(0.0), rot_z(PI / 2.0)))] {
                let ring = mul(at(mid + v3(dx, 0.0, 0.0)), m);
                c.k.add(LINK, mul(ring, scale(0.36, 0.5, 0.36)), STEEL);
            }
        }
        if c.detail {
            if let Some(h) = height {
                let shown = h as i64 - (BLOCKS - 2 - i) as i64;
                let label = if newest { "?".to_string() } else { shown.to_string() };
                c.k.tag(o + v3(x, y, 1.3), label, newest);
            }
        }
    }
    if c.detail {
        let won = c.live.sample.nonce == GENESIS_NONCE;
        let hash = rev_hex(&c.live.sample.hash);
        c.k.callout(o + v3(7.0, 6.7, 0.0), Vec2::new(-30.0, -44.0), format!("{}…", &hash[..20]), won);
    }
}
