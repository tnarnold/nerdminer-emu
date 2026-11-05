//! Procedural meshes of the studio renderer. Everything is built from code when the lab opens.

use super::scene::{V3, v3};
use bytemuck::{Pod, Zeroable};
use std::f32::consts::TAU;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vtx {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
}

#[derive(Default)]
pub struct Meshes {
    pub verts: Vec<Vtx>,
    pub index: Vec<u32>,
    /// (first index, index count) of each mesh.
    pub ranges: Vec<(u32, u32)>,
}

impl Meshes {
    /// Starts a mesh; everything added until the next call belongs to it. Returns its id.
    pub fn begin(&mut self) -> usize {
        self.close();
        self.ranges.push((self.index.len() as u32, 0));
        self.ranges.len() - 1
    }

    pub fn close(&mut self) {
        if let Some(r) = self.ranges.last_mut() {
            r.1 = self.index.len() as u32 - r.0;
        }
    }

    fn vertex(&mut self, p: V3, n: V3) -> u32 {
        self.verts.push(Vtx { pos: [p.x, p.y, p.z], normal: [n.x, n.y, n.z] });
        self.verts.len() as u32 - 1
    }

    /// Triangle wound to face along the first normal, whatever the order of the corners.
    fn tri(&mut self, a: (V3, V3), b: (V3, V3), c: (V3, V3)) {
        let facing = (b.0 - a.0).cross(c.0 - a.0).dot(a.1 + b.1 + c.1) >= 0.0;
        let (ia, ib, ic) = (self.vertex(a.0, a.1), self.vertex(b.0, b.1), self.vertex(c.0, c.1));
        self.index.extend_from_slice(&if facing { [ia, ib, ic] } else { [ia, ic, ib] });
    }

    fn quad(&mut self, a: (V3, V3), b: (V3, V3), c: (V3, V3), d: (V3, V3)) {
        self.tri(a, b, c);
        self.tri(a, c, d);
    }

    /// Box with rounded edges. `h` is the half size, `b` the edge radius.
    pub fn bevel_box(&mut self, h: V3, b: f32, steps: usize) {
        let h = [h.x, h.y, h.z];
        let lines = |h: f32| -> Vec<f32> {
            let mut l: Vec<f32> = (0..=steps).map(|k| -h + b * k as f32 / steps as f32).collect();
            l.extend((0..=steps).map(|k| h - b + b * k as f32 / steps as f32));
            l
        };
        let place = |a: usize, s: f32, p: f32, q: f32| -> (V3, V3) {
            let mut c = [0.0; 3];
            c[a] = s * h[a];
            c[(a + 1) % 3] = p;
            c[(a + 2) % 3] = q;
            let inner = [0, 1, 2].map(|i| c[i].clamp(-(h[i] - b), h[i] - b));
            let d = v3(c[0] - inner[0], c[1] - inner[1], c[2] - inner[2]).norm();
            (v3(inner[0], inner[1], inner[2]) + d * b, d)
        };
        for a in 0..3 {
            let (lu, lv) = (lines(h[(a + 1) % 3]), lines(h[(a + 2) % 3]));
            for s in [1.0, -1.0] {
                for i in 0..lu.len() - 1 {
                    for j in 0..lv.len() - 1 {
                        self.quad(
                            place(a, s, lu[i], lv[j]),
                            place(a, s, lu[i + 1], lv[j]),
                            place(a, s, lu[i + 1], lv[j + 1]),
                            place(a, s, lu[i], lv[j + 1]),
                        );
                    }
                }
            }
        }
    }

    /// Surface of revolution around Y. The profile goes bottom to top on the outside.
    pub fn lathe(&mut self, profile: &[(f32, f32)], seg: usize) {
        self.lathe_arc(profile, seg, 0.0, TAU);
    }

    pub fn lathe_arc(&mut self, profile: &[(f32, f32)], seg: usize, from: f32, to: f32) {
        let at = |r: f32, y: f32, a: f32| v3(a.cos() * r, y, a.sin() * r);
        for w in profile.windows(2) {
            let ((r0, y0), (r1, y1)) = (w[0], w[1]);
            let (dr, dy) = (r1 - r0, y1 - y0);
            let l = (dr * dr + dy * dy).sqrt().max(1e-6);
            let (nr, ny) = (dy / l, -dr / l);
            for i in 0..seg {
                let a = from + (to - from) * i as f32 / seg as f32;
                let b = from + (to - from) * (i + 1) as f32 / seg as f32;
                let (na, nb) = (at(nr, ny, a), at(nr, ny, b));
                self.quad((at(r0, y0, a), na), (at(r0, y0, b), nb), (at(r1, y1, b), nb), (at(r1, y1, a), na));
            }
        }
    }

    /// Flat end of an opened shell, at angle `a`.
    pub fn arc_cap(&mut self, profile: &[(f32, f32)], a: f32, facing: f32) {
        let n = v3(-a.sin(), 0.0, a.cos()) * facing;
        let at = |p: (f32, f32)| v3(a.cos() * p.0, p.1, a.sin() * p.0);
        let c = profile.iter().fold((0.0, 0.0), |s, p| (s.0 + p.0, s.1 + p.1));
        let c = at((c.0 / profile.len() as f32, c.1 / profile.len() as f32));
        for w in profile.windows(2) {
            self.tri((c, n), (at(w[0]), n), (at(w[1]), n));
        }
    }

    /// Extruded outline around Y, chamfered, with a round bore (`bore` 0 closes it).
    pub fn prism(&mut self, outline: &[(f32, f32)], half: f32, chamfer: f32, bore: f32) {
        let n = outline.len();
        let at = |p: (f32, f32), y: f32, inset: f32| {
            let r = (p.0 * p.0 + p.1 * p.1).sqrt().max(1e-6);
            let k = (r - inset) / r;
            v3(p.0 * k, y, p.1 * k)
        };
        let up = v3(0.0, 1.0, 0.0);
        for i in 0..n {
            let (p, q) = (outline[i], outline[(i + 1) % n]);
            let e = at(q, 0.0, 0.0) - at(p, 0.0, 0.0);
            let mid = at(p, 0.0, 0.0) + e * 0.5;
            let out = v3(e.z, 0.0, -e.x).norm();
            let out = if out.dot(mid) < 0.0 { out * -1.0 } else { out };
            let (y, c) = (half - chamfer, chamfer);
            self.quad((at(p, -y, 0.0), out), (at(q, -y, 0.0), out), (at(q, y, 0.0), out), (at(p, y, 0.0), out));
            for s in [1.0, -1.0] {
                let slope = (out + up * s).norm();
                self.quad(
                    (at(p, y * s, 0.0), slope),
                    (at(q, y * s, 0.0), slope),
                    (at(q, half * s, c), slope),
                    (at(p, half * s, c), slope),
                );
                let normal = up * s;
                if bore > 0.0 {
                    let (ap, aq) = (p.1.atan2(p.0), q.1.atan2(q.0));
                    let ring = |a: f32| v3(a.cos() * bore, half * s, a.sin() * bore);
                    self.quad((at(p, half * s, c), normal), (at(q, half * s, c), normal), (ring(aq), normal), (ring(ap), normal));
                } else {
                    self.tri((at(p, half * s, c), normal), (at(q, half * s, c), normal), (v3(0.0, half * s, 0.0), normal));
                }
            }
            if bore > 0.0 {
                let (ap, aq) = (p.1.atan2(p.0), q.1.atan2(q.0));
                let wall = |a: f32, y: f32| (v3(a.cos() * bore, y, a.sin() * bore), v3(-a.cos(), 0.0, -a.sin()));
                self.quad(wall(ap, -half), wall(aq, -half), wall(aq, half), wall(ap, half));
            }
        }
    }

    /// Spur gear with `m` module and `z` teeth, centered at the origin.
    pub fn gear(&mut self, m: f32, z: usize, half: f32, bore: f32) {
        let pitch = m * z as f32 / 2.0;
        let (tip, root) = (pitch + m, pitch - 1.25 * m);
        let step = TAU / z as f32;
        let mut o = Vec::with_capacity(z * 4);
        for k in 0..z {
            let c = k as f32 * step;
            for (a, r) in [(-0.34, root), (-0.17, tip), (0.17, tip), (0.34, root)] {
                let t = c + a * step;
                o.push((t.cos() * r, t.sin() * r));
            }
        }
        self.prism(&o, half, 0.03, bore);
    }

    /// Regular polygon prism (bolt heads).
    pub fn ngon(&mut self, sides: usize, r: f32, half: f32, chamfer: f32) {
        let o: Vec<_> = (0..sides).map(|k| (k as f32 * TAU / sides as f32).cos() * r).zip((0..sides).map(|k| (k as f32 * TAU / sides as f32).sin() * r)).collect();
        self.prism(&o, half, chamfer, 0.0);
    }

    /// Tube of radius `r` along a Catmull-Rom curve through `path`.
    pub fn sweep(&mut self, path: &[V3], r: f32, seg: usize) {
        let pts = catmull(path, 14);
        let mut prev: Option<Vec<(V3, V3)>> = None;
        let mut side = v3(0.0, 0.0, 1.0);
        for i in 0..pts.len() {
            let t = (pts[(i + 1).min(pts.len() - 1)] - pts[i.saturating_sub(1)]).norm();
            let u = t.cross(side).norm();
            side = u.cross(t).norm();
            let ring: Vec<_> = (0..=seg)
                .map(|k| {
                    let (s, c) = (k as f32 / seg as f32 * TAU).sin_cos();
                    let n = u * c + side * s;
                    (pts[i] + n * r, n)
                })
                .collect();
            if let Some(p) = &prev {
                for k in 0..seg {
                    self.quad(p[k], p[k + 1], ring[k + 1], ring[k]);
                }
            }
            prev = Some(ring);
        }
    }

    pub fn ground(&mut self, half: f32) {
        let n = v3(0.0, 1.0, 0.0);
        self.quad((v3(-half, 0.0, -half), n), (v3(half, 0.0, -half), n), (v3(half, 0.0, half), n), (v3(-half, 0.0, half), n));
    }
}

pub fn catmull(p: &[V3], per: usize) -> Vec<V3> {
    let mut out = Vec::new();
    for i in 0..p.len() - 1 {
        let (a, b, c, d) = (p[i.saturating_sub(1)], p[i], p[i + 1], p[(i + 2).min(p.len() - 1)]);
        for k in 0..per {
            let t = k as f32 / per as f32;
            let (t2, t3) = (t * t, t * t * t);
            out.push((b * 2.0 + (c - a) * t + (a * 2.0 - b * 5.0 + c * 4.0 - d) * t2 + (b * 3.0 - a - c * 3.0 + d) * t3) * 0.5);
        }
    }
    out.push(*p.last().expect("path"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every triangle must face its own vertex normals, or culling punches holes in the model.
    fn outward(m: &Meshes) -> bool {
        m.index.chunks(3).all(|t| {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| m.verts[i as usize]);
            let [pa, pb, pc] = [a, b, c].map(|v| v3(v.pos[0], v.pos[1], v.pos[2]));
            let n = v3(a.normal[0] + b.normal[0] + c.normal[0], a.normal[1] + b.normal[1] + c.normal[1], a.normal[2] + b.normal[2] + c.normal[2]);
            (pb - pa).cross(pc - pa).dot(n) >= -1e-5
        })
    }

    #[test]
    fn meshes_face_outward() {
        let mut m = Meshes::default();
        m.begin();
        m.bevel_box(v3(1.0, 0.4, 0.6), 0.1, 2);
        m.gear(0.15, 12, 0.2, 0.1);
        m.ngon(6, 0.2, 0.1, 0.02);
        m.lathe(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)], 16);
        m.close();
        assert!(outward(&m));
        assert_eq!(m.ranges[0].1 as usize, m.index.len());
    }

    #[test]
    fn beveled_box_keeps_its_size() {
        let mut m = Meshes::default();
        m.begin();
        m.bevel_box(v3(2.0, 1.0, 0.5), 0.1, 2);
        let max = m.verts.iter().fold([0f32; 3], |a, v| [a[0].max(v.pos[0]), a[1].max(v.pos[1]), a[2].max(v.pos[2])]);
        assert!((max[0] - 2.0).abs() < 1e-4 && (max[1] - 1.0).abs() < 1e-4 && (max[2] - 0.5).abs() < 1e-4);
    }
}
