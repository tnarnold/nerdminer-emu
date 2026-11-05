//! Parts bin of the studio: transforms, materials and the procedural meshes every station
//! is assembled from.

use super::mesh::Meshes;
use super::scene::{V3, v3};
use super::studio::Inst;
use eframe::egui::Vec2;
use std::f32::consts::PI;

pub type M = [[f32; 4]; 3];

const ID: M = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];

pub fn mul(a: M, b: M) -> M {
    let mut m = [[0.0; 4]; 3];
    for r in 0..3 {
        for c in 0..4 {
            m[r][c] = (0..3).map(|k| a[r][k] * b[k][c]).sum::<f32>() + if c == 3 { a[r][3] } else { 0.0 };
        }
    }
    m
}

pub fn at(p: V3) -> M {
    let mut m = ID;
    (m[0][3], m[1][3], m[2][3]) = (p.x, p.y, p.z);
    m
}

pub fn scale(x: f32, y: f32, z: f32) -> M {
    let mut m = ID;
    (m[0][0], m[1][1], m[2][2]) = (x, y, z);
    m
}

pub fn rot_x(a: f32) -> M {
    let (s, c) = a.sin_cos();
    [[1.0, 0.0, 0.0, 0.0], [0.0, c, -s, 0.0], [0.0, s, c, 0.0]]
}

pub fn rot_y(a: f32) -> M {
    let (s, c) = a.sin_cos();
    [[c, 0.0, s, 0.0], [0.0, 1.0, 0.0, 0.0], [-s, 0.0, c, 0.0]]
}

pub fn rot_z(a: f32) -> M {
    let (s, c) = a.sin_cos();
    [[c, -s, 0.0, 0.0], [s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]]
}

pub fn point(m: M, p: V3) -> V3 {
    v3(
        m[0][0] * p.x + m[0][1] * p.y + m[0][2] * p.z + m[0][3],
        m[1][0] * p.x + m[1][1] * p.y + m[1][2] * p.z + m[1][3],
        m[2][0] * p.x + m[2][1] * p.y + m[2][2] * p.z + m[2][3],
    )
}

// Mesh ids, in the order of `build_meshes`.
pub const FLOOR: usize = 0;
pub const TILE: usize = 1;
pub const BOARD: usize = 2;
pub const BED: usize = 3;
pub const BOX: usize = 4;
pub const GEAR_DRUM: usize = 5;
pub const GEAR_IDLER: usize = 6;
pub const ROTOR: usize = 7;
pub const SHAFT: usize = 8;
pub const SHELL: usize = 9;
pub const PEDESTAL: usize = 10;
pub const NUT: usize = 11;
pub const BOLT: usize = 12;
pub const PIPE: usize = 13;
pub const FLANGE: usize = 14;
pub const RING: usize = 15;
pub const BEAD: usize = 16;
pub const CRATE: usize = 17;
pub const WHEEL: usize = 18;
pub const LINK: usize = 19;

pub const MODULE: f32 = 0.15;
pub const Z_DRUM: usize = 24;
pub const Z_IDLER: usize = 12;
pub const PIPE_PATH: [V3; 5] = [v3(-4.0, 3.75, 0.0), v3(-4.0, 4.7, 0.0), v3(-2.9, 5.0, 0.25), v3(-1.5, 3.9, 0.3), v3(-1.0, 2.25, 0.3)];

pub fn build_meshes() -> Meshes {
    let mut m = Meshes::default();
    m.begin();
    m.ground(480.0);
    m.begin();
    m.bevel_box(v3(0.275, 0.13, 0.275), 0.06, 2);
    m.begin();
    m.bevel_box(v3(5.5, 0.15, 5.5), 0.08, 2);
    m.begin();
    m.bevel_box(v3(5.6, 0.17, 3.0), 0.09, 2);
    m.begin();
    m.bevel_box(v3(1.0, 1.0, 1.0), 0.06, 2);
    m.begin();
    m.gear(MODULE, Z_DRUM, 0.16, 0.2);
    m.begin();
    m.gear(MODULE, Z_IDLER, 0.16, 0.14);
    m.begin();
    m.gear(0.09, 42, 0.075, 0.2);
    m.begin();
    m.lathe(&[(0.0, -1.0), (0.97, -1.0), (1.0, -0.97), (1.0, 0.97), (0.97, 1.0), (0.0, 1.0)], 28);
    m.begin();
    let shell = [(2.3, 0.0), (2.3, 2.72), (2.22, 2.8), (2.08, 2.8), (2.08, 0.0), (2.3, 0.0)];
    let (from, to) = (PI * 0.5 + 0.95, PI * 2.5 - 0.95);
    m.lathe_arc(&shell, 72, from, to);
    m.arc_cap(&shell, from, -1.0);
    m.arc_cap(&shell, to, 1.0);
    m.begin();
    m.lathe(&[(0.0, 0.0), (2.42, 0.0), (2.42, 0.2), (2.34, 0.28), (0.0, 0.28)], 72);
    m.begin();
    m.lathe(&[(0.0, 0.0), (0.42, 0.0), (0.42, 0.16), (0.3, 0.22), (0.3, 0.42), (0.22, 0.48), (0.0, 0.48)], 28);
    m.begin();
    m.ngon(6, 0.13, 0.06, 0.018);
    m.begin();
    m.sweep(&PIPE_PATH, 0.15, 14);
    m.begin();
    m.lathe(&[(0.0, 0.0), (0.34, 0.0), (0.34, 0.1), (0.26, 0.14), (0.2, 0.3), (0.0, 0.3)], 22);
    m.begin();
    m.lathe(&[(0.995, 0.0), (1.0, 0.0), (1.0, 0.004), (0.995, 0.004), (0.995, 0.0)], 160);
    m.begin();
    let bead: Vec<(f32, f32)> = (0..=10).map(|k| k as f32 / 10.0 * PI).map(|a| (a.sin(), -a.cos())).collect();
    m.lathe(&bead, 18);
    m.begin();
    m.bevel_box(v3(1.0, 1.0, 1.0), 0.14, 3);
    m.begin();
    m.gear(0.14, 16, 0.42, 0.2);
    m.begin();
    let tube: Vec<(f32, f32)> = (0..=14).map(|k| k as f32 / 14.0 * 2.0 * PI).map(|a| (1.0 - a.cos() * 0.2, -a.sin() * 0.2)).collect();
    m.lathe(&tube, 32);
    m.close();
    m
}

#[derive(Clone, Copy)]
pub struct Mat {
    pub color: u32,
    pub metal: f32,
    pub rough: f32,
    pub glow: f32,
}

pub const CERAMIC: Mat = Mat { color: 0xE9E8E4, metal: 0.0, rough: 0.42, glow: 0.0 };
pub const ANODIZED: Mat = Mat { color: 0x1A1B20, metal: 0.55, rough: 0.46, glow: 0.0 };
pub const GRAPHITE: Mat = Mat { color: 0x272A31, metal: 0.0, rough: 0.55, glow: 0.0 };
pub const STEEL: Mat = Mat { color: 0xC2C6CD, metal: 1.0, rough: 0.45, glow: 0.0 };
pub const BRUSHED: Mat = Mat { color: 0x8C9099, metal: 1.0, rough: 0.46, glow: 0.0 };
pub const ACCENT: Mat = Mat { color: 0xF7931A, metal: 0.0, rough: 0.42, glow: 0.0 };
pub const INK: Mat = Mat { color: 0x5C606A, metal: 0.0, rough: 0.8, glow: 0.0 };
pub const DULL: Mat = Mat { color: 0x34383F, ..ANODIZED };

fn linear(c: u32) -> [f32; 3] {
    [(c >> 16) & 255, (c >> 8) & 255, c & 255].map(|v| {
        let v = v as f32 / 255.0;
        if v < 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    })
}

/// Blend of two materials, for parts that change state.
pub fn blend(a: Mat, b: Mat, t: f32) -> Mat {
    let t = t.clamp(0.0, 1.0);
    let channel = |s: u32| {
        let (x, y) = (((a.color >> s) & 255) as f32, ((b.color >> s) & 255) as f32);
        ((x + (y - x) * t) as u32) << s
    };
    Mat {
        color: channel(16) | channel(8) | channel(0),
        metal: a.metal + (b.metal - a.metal) * t,
        rough: a.rough + (b.rough - a.rough) * t,
        glow: a.glow + (b.glow - a.glow) * t,
    }
}

/// Text placed over the model by egui, at a world position.
pub struct Note {
    pub at: V3,
    /// Leader from the point to the label; `None` centers the text on the point.
    pub leader: Option<Vec2>,
    pub text: String,
    pub accent: bool,
}

/// What a station adds to the frame.
#[derive(Default)]
pub struct Kit {
    pub insts: Vec<(usize, Inst)>,
    pub notes: Vec<Note>,
}

impl Kit {
    pub fn add(&mut self, mesh: usize, m: M, mat: Mat) {
        let c = linear(mat.color);
        let kind = if mesh == FLOOR { 1.0 } else { 0.0 };
        self.insts.push((mesh, Inst { m, color: [c[0], c[1], c[2], mat.glow], mat: [mat.metal, mat.rough, kind, 0.0] }));
    }

    /// Round bar between two points.
    pub fn bar(&mut self, a: V3, b: V3, r: f32, mat: Mat) {
        let half = (b - a) * 0.5;
        let y = half.norm();
        let x = if y.y.abs() > 0.9 { v3(1.0, 0.0, 0.0) } else { v3(0.0, 1.0, 0.0) }.cross(y).norm();
        let z = x.cross(y);
        let c = a + half;
        let m = [
            [x.x * r, half.x, z.x * r, c.x],
            [x.y * r, half.y, z.y * r, c.y],
            [x.z * r, half.z, z.z * r, c.z],
        ];
        self.add(SHAFT, m, mat);
    }

    pub fn callout(&mut self, at: V3, leader: Vec2, text: impl Into<String>, accent: bool) {
        self.notes.push(Note { at, leader: Some(leader), text: text.into(), accent });
    }

    pub fn tag(&mut self, at: V3, text: impl Into<String>, accent: bool) {
        self.notes.push(Note { at, leader: None, text: text.into(), accent });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_spans_its_two_ends() {
        let mut k = Kit::default();
        let (a, b) = (v3(1.0, 2.0, 3.0), v3(4.0, 0.0, -1.0));
        k.bar(a, b, 0.2, STEEL);
        let m = k.insts[0].1.m;
        let (p, q) = (point(m, v3(0.0, -1.0, 0.0)), point(m, v3(0.0, 1.0, 0.0)));
        assert!((p - a).dot(p - a) < 1e-6 && (q - b).dot(q - b) < 1e-6);
    }

    #[test]
    fn transforms_compose_in_order() {
        let m = mul(at(v3(1.0, 0.0, 0.0)), scale(2.0, 2.0, 2.0));
        let p = point(m, v3(1.0, 0.0, 0.0));
        assert!((p.x - 3.0).abs() < 1e-6);
    }

    #[test]
    fn every_mesh_id_exists() {
        assert_eq!(build_meshes().ranges.len(), LINK + 1);
    }
}
