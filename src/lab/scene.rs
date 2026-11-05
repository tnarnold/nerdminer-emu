//! Vectors, the orbit camera and the projection that ties the egui labels to the 3D model.

use eframe::egui::{Pos2, Rect, Vec2};
use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn v3(x: f32, y: f32, z: f32) -> V3 {
    V3 { x, y, z }
}

impl Add for V3 {
    type Output = V3;
    fn add(self, o: V3) -> V3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for V3 {
    type Output = V3;
    fn sub(self, o: V3) -> V3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f32> for V3 {
    type Output = V3;
    fn mul(self, k: f32) -> V3 {
        v3(self.x * k, self.y * k, self.z * k)
    }
}
impl V3 {
    pub fn dot(self, o: V3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: V3) -> V3 {
        v3(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }
    pub fn norm(self) -> V3 {
        let l = self.dot(self).sqrt().max(1e-6);
        self * (1.0 / l)
    }
    pub fn lerp(self, o: V3, t: f32) -> V3 {
        self + (o - self) * t
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub target: V3,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
}

pub const NEAR: f32 = 1.0;
/// A long lens, as in product photography.
pub const FOV: f32 = 0.5;

pub struct View {
    pub eye: V3,
    pub right: V3,
    pub up: V3,
    pub fwd: V3,
    focal: f32,
    center: Pos2,
}

impl Camera {
    pub fn view(&self, rect: Rect) -> View {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let eye = self.target + v3(sy * cp, sp, cy * cp) * self.dist;
        let fwd = (self.target - eye).norm();
        let right = fwd.cross(v3(0.0, 1.0, 0.0)).norm();
        let up = right.cross(fwd);
        View { eye, right, up, fwd, focal: 0.5 * rect.height() / (FOV / 2.0).tan(), center: rect.center() }
    }
}

impl View {
    /// Screen position and depth; `None` behind the camera.
    pub fn project(&self, p: V3) -> Option<(Pos2, f32)> {
        let d = p - self.eye;
        let z = d.dot(self.fwd);
        if z < NEAR {
            return None;
        }
        let k = self.focal / z;
        Some((self.center + Vec2::new(d.dot(self.right) * k, -d.dot(self.up) * k), z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_centers_target_and_rejects_points_behind() {
        let cam = Camera { target: v3(1.0, 2.0, 3.0), yaw: 0.7, pitch: 0.3, dist: 10.0 };
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let v = cam.view(rect);
        let (p, depth) = v.project(cam.target).unwrap();
        assert!((p - rect.center()).length() < 0.01);
        assert!((depth - 10.0).abs() < 0.01);
        assert!(v.project(v.eye - v.fwd).is_none());
    }

    #[test]
    fn farther_points_land_closer_to_the_center() {
        let cam = Camera { target: v3(0.0, 0.0, 0.0), yaw: 0.0, pitch: 0.0, dist: 10.0 };
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let v = cam.view(rect);
        let near = v.project(v3(2.0, 0.0, 5.0)).unwrap().0;
        let far = v.project(v3(2.0, 0.0, -5.0)).unwrap().0;
        assert!((near - rect.center()).length() > (far - rect.center()).length());
    }
}
