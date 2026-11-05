//! "How mining works": an on-demand window that teaches the mining process in 3D, driven by
//! what the miners are actually doing.
//!
//! Nothing here exists until the window opens: `NerdApp` holds an `Option<Handle>` that is
//! `None` before that and after the window closes, which frees all of it.

mod live;
mod mesh;
mod model;
mod scene;
mod stations;
mod studio;
mod text;

use crate::fmt;
use crate::i18n::fill;
use crate::state::Shared;
use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, UiBuilder, Vec2, ViewportBuilder, ViewportId,
};
use eframe::wgpu;
use live::{Live, diff_bits, rev_hex};
use model::{FLOOR, INK, Kit, at, build_meshes};
use scene::{Camera, V3, View, v3};
use stations::{Board, COUNT, Ctx, ORANGE, SPACING, active_field, origin};
use studio::{Frame, Look, Slot};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use text::{LabText, text};

const PANEL_W: f32 = 372.0;
const FRAME: Duration = Duration::from_millis(33); // ~30 fps: the miners come first
const AUTOPLAY: Duration = Duration::from_secs(25);
const PANEL_BG: Color32 = Color32::from_rgb(19, 22, 31);
const MUTED: Color32 = Color32::from_rgb(150, 160, 180);
const GOLD: Color32 = Color32::from_rgb(255, 208, 92);
const BODY: Color32 = Color32::from_rgb(212, 217, 228);

/// Colors of the text around the scene, which follow the studio backdrop.
struct Pal {
    panel: Color32,
    body: Color32,
    muted: Color32,
    fact: Color32,
    accent: Color32,
    event: Color32,
    ink: Color32,
}

const DARK: Pal = Pal { panel: PANEL_BG, body: BODY, muted: MUTED, fact: Color32::LIGHT_GRAY, accent: ORANGE, event: GOLD, ink: BODY };
const LIGHT: Pal = Pal {
    panel: Color32::from_rgb(246, 246, 247),
    body: Color32::from_rgb(44, 47, 54),
    muted: Color32::from_rgb(92, 97, 108),
    fact: Color32::from_rgb(58, 62, 70),
    accent: Color32::from_rgb(196, 98, 0),
    event: Color32::from_rgb(160, 90, 0),
    ink: Color32::from_rgb(38, 41, 48),
};

pub type Handle = Arc<Mutex<LabState>>;

pub struct LabState {
    live: Live,
    step: usize,
    live_mode: bool,
    technical: bool,
    playing: bool,
    step_at: Instant,
    cam: Camera,
    orbit: (f32, f32),
    zoom: f32,
    started: Instant,
    last_frame: Instant,
    close: bool,
    shots: Option<Shots>,
    /// Target format of the window; `None` when eframe is not running on wgpu.
    format: Option<wgpu::TextureFormat>,
    /// GPU objects of the studio renderer, created on the first frame.
    gpu: Slot,
    board: Board,
    look: Look,
    bench: Option<Bench>,
}

/// Debug: NERDMINER_LAB_BENCH=1 stays on station 5 with no frame cap and logs what it costs.
struct Bench {
    since: Instant,
    frames: u32,
    bytes: Arc<AtomicU64>,
}

/// Debug: NERDMINER_LAB_SHOTS=<dir> saves one RGBA screenshot per station and exits.
struct Shots {
    dir: String,
    next: usize,
    at: Instant,
}

/// `t` drives the live mode's slow travel along the line.
fn goal(step: usize, live_mode: bool, t: f32) -> Camera {
    if live_mode {
        let mid = (COUNT - 1) as f32 * SPACING / 2.0;
        let x = mid - (t * 0.05).cos() * mid * 0.8;
        return Camera { target: v3(x, 3.0, 0.0), yaw: 0.34, pitch: 0.32, dist: 98.0 };
    }
    // Target offset, yaw, pitch and distance of each station, framed as a product shot.
    let (x, y, yaw, pitch, dist) = [
        (0.0, 1.8, -0.30, 0.44, 45.0),
        (0.0, 3.6, 0.26, 0.22, 42.0),
        (0.0, 0.8, -0.32, 0.62, 42.0),
        (0.0, 1.8, 0.30, 0.38, 42.0),
        (0.0, 1.2, -0.36, 0.56, 51.0),
        (-0.2, 3.8, -0.32, 0.22, 44.0),
        (0.8, 3.8, 0.30, 0.24, 44.0),
        (0.0, 2.6, -0.30, 0.34, 40.0),
    ][step];
    Camera { target: origin(step) + v3(x, y, 0.0), yaw, pitch, dist }
}

pub fn open(shared: &Arc<Shared>, format: Option<wgpu::TextureFormat>) -> Handle {
    let now = Instant::now();
    let shots = std::env::var("NERDMINER_LAB_SHOTS").ok().map(|dir| Shots { dir, next: 0, at: now + Duration::from_secs(7) });
    let bench = std::env::var_os("NERDMINER_LAB_BENCH").map(|_| Bench { since: now, frames: 0, bytes: Arc::new(AtomicU64::new(0)) });
    let look = if std::env::var("NERDMINER_LAB_LOOK").as_deref() == Ok("dark") { Look::Dark } else { Look::Light };
    Arc::new(Mutex::new(LabState {
        live: Live::new(shared),
        step: if bench.is_some() { 4 } else { 0 },
        live_mode: std::env::var("NERDMINER_LAB_BENCH").as_deref() == Ok("live"),
        technical: false,
        playing: false,
        step_at: now,
        cam: goal(0, false, 0.0),
        orbit: (0.0, 0.0),
        zoom: 1.0,
        started: now,
        last_frame: now,
        close: false,
        shots,
        format,
        gpu: Arc::new(Mutex::new(None)),
        board: Board::new(),
        look,
        bench,
    }))
}

pub fn closed(lab: &Handle) -> bool {
    lab.lock().unwrap().close
}

/// Called on every pass of the root viewport while the lab is open.
pub fn show(ctx: &egui::Context, shared: &Arc<Shared>, lab: &Handle) {
    let (lab, shared) = (lab.clone(), shared.clone());
    let builder = ViewportBuilder::default()
        .with_title(text().title)
        .with_inner_size([1120.0, 700.0])
        .with_min_inner_size([820.0, 520.0]);
    ctx.show_viewport_deferred(ViewportId::from_hash_of("mining-lab"), builder, move |ui, _| {
        lab.lock().unwrap().frame(ui, &shared);
    });
}

impl LabState {
    fn pal(&self) -> &'static Pal {
        if self.look == Look::Light { &LIGHT } else { &DARK }
    }

    fn go(&mut self, step: usize) {
        self.step = step % COUNT;
        self.step_at = Instant::now();
        self.live_mode = false;
        self.orbit = (0.0, 0.0);
        self.zoom = 1.0;
    }

    fn frame(&mut self, ui: &mut egui::Ui, shared: &Shared) {
        let ctx = ui.ctx().clone();
        if ui.input(|i| i.viewport().close_requested()) {
            self.close = true;
            ctx.request_repaint_of(ViewportId::ROOT);
        }
        // Minimized: no animation and no sampling.
        if ui.input(|i| i.viewport().minimized) == Some(true) {
            ctx.request_repaint_after(Duration::from_millis(500));
            return;
        }
        if let Some(b) = &mut self.bench {
            if std::env::var("NERDMINER_LAB_BENCH").as_deref() == Ok("cap") {
                ctx.request_repaint_after(FRAME);
            } else {
                ctx.request_repaint();
            }
            b.frames += 1;
            if b.since.elapsed() >= Duration::from_secs(5) {
                let fps = b.frames as f32 / b.since.elapsed().as_secs_f32();
                eprintln!("lab bench: {fps:.1} fps, render targets {} MB", b.bytes.load(Ordering::Relaxed) / 1_000_000);
                (b.since, b.frames) = (Instant::now(), 0);
            }
        } else {
            ctx.request_repaint_after(FRAME);
        }

        let tx = text();
        self.live.tick(shared, tx);
        if self.playing && !self.live_mode && self.step_at.elapsed() >= AUTOPLAY {
            self.go(self.step + 1);
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            self.go(self.step + 1);
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            self.go(self.step + COUNT - 1);
        }

        let full = ui.available_rect_before_wrap();
        let view_rect = Rect::from_min_max(full.min, Pos2::new(full.max.x - PANEL_W, full.max.y));
        let panel = Rect::from_min_max(Pos2::new(view_rect.max.x, full.min.y), full.max);
        self.scene(ui, view_rect, tx);
        ui.painter().rect_filled(panel, 0.0, self.pal().panel);
        let light = std::ptr::eq(self.pal(), &LIGHT);
        ui.scope_builder(UiBuilder::new().max_rect(panel.shrink2(Vec2::new(16.0, 14.0))), |ui| {
            if light {
                ui.style_mut().visuals = egui::Visuals::light();
            }
            self.panel(ui, tx);
        });
        self.debug_shots(&ctx);
    }

    fn scene(&mut self, ui: &mut egui::Ui, rect: Rect, tx: &LabText) {
        let response = ui.allocate_rect(rect, Sense::drag());
        let d = response.drag_delta();
        self.orbit.0 -= d.x * 0.006;
        self.orbit.1 = (self.orbit.1 + d.y * 0.005).clamp(-0.25, 0.9);
        if response.hovered() {
            let s = ui.input(|i| i.smooth_scroll_delta.y);
            self.zoom = (self.zoom * (1.0 - s * 0.002)).clamp(0.45, 2.2);
        }

        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        let k = 1.0 - (-dt * 2.6).exp();
        let t = self.started.elapsed().as_secs_f32();
        let g = goal(self.step, self.live_mode, t);
        self.cam.target = self.cam.target.lerp(g.target, k);
        self.cam.yaw += (g.yaw + self.orbit.0 - self.cam.yaw) * k;
        self.cam.pitch += (g.pitch + self.orbit.1 - self.cam.pitch) * k;
        self.cam.dist += (g.dist * self.zoom - self.cam.dist) * k;

        let painter = ui.painter_at(rect);
        let Some(format) = self.format else { return };
        let mut kit = Kit::default();
        kit.insts.reserve(2048);
        kit.add(FLOOR, at(v3(self.cam.target.x, -0.02, 0.0)), INK);
        // Only the stations around the camera are built.
        let reach = if self.live_mode { 3.4 } else { 1.3 };
        for i in 0..COUNT {
            if (self.cam.target.x - origin(i).x).abs() > SPACING * reach {
                continue;
            }
            let detail = !self.live_mode && i == self.step;
            let mut c = Ctx { k: &mut kit, t, dt, live: &self.live, tx, look: self.look, board: &mut self.board, detail };
            stations::floor(&mut c, i);
            stations::draw(i, &mut c);
            if self.live_mode {
                c.k.tag(origin(i) + v3(0.0, 13.0, 0.0), format!("{}. {}", i + 1, tx.stations[i].name), true);
            }
        }
        let Kit { mut insts, notes } = kit;
        insts.sort_by_key(|i| i.0);
        let frame = Frame {
            slot: self.gpu.clone(),
            format,
            rect,
            cam: self.cam,
            look: self.look,
            center: v3(self.cam.target.x, 0.0, 0.0),
            radius: if self.live_mode { 110.0 } else { 24.0 },
            insts,
            build: build_meshes,
            report: self.bench.as_ref().map(|b| b.bytes.clone()),
        };
        painter.add(Shape::Callback(frame.shape()));
        let view = self.cam.view(rect);
        let pal = self.pal();
        for n in &notes {
            let ink = if n.accent { pal.accent } else { pal.ink };
            match n.leader {
                Some(leader) => callout(&painter, &view, n.at, leader, &n.text, ink, pal),
                None => {
                    if let Some((p, _)) = view.project(n.at) {
                        plated(&painter, p, Align2::CENTER_CENTER, &n.text, ink, pal);
                    }
                }
            }
        }
        self.overlays(&painter, rect, tx);
    }

    fn overlays(&self, painter: &egui::Painter, rect: Rect, tx: &LabText) {
        let pal = self.pal();
        let font = FontId::proportional(13.0);
        let left = rect.min.x + 16.0;
        let mut y = rect.min.y + 14.0;
        if self.live.demo {
            painter.text(Pos2::new(left, y), Align2::LEFT_TOP, tx.demo_banner, font.clone(), pal.accent);
            y += 22.0;
        } else if self.live.hashes_per_sample() >= 1.0 {
            let n = fmt::big(self.live.hashes_per_sample());
            painter.text(Pos2::new(left, y), Align2::LEFT_TOP, fill(tx.sample_note, &[&n]), font.clone(), pal.muted);
            y += 22.0;
        }
        for (at, ev) in self.live.events.iter().rev() {
            let alpha = (1.0 - (at.elapsed().as_secs_f32() - 12.0) / 3.0).clamp(0.0, 1.0);
            if alpha > 0.0 {
                painter.text(Pos2::new(left, y), Align2::LEFT_TOP, format!("● {ev}"), font.clone(), pal.event.gamma_multiply(alpha));
                y += 20.0;
            }
        }
        painter.text(Pos2::new(rect.center().x, rect.max.y - 14.0), Align2::CENTER_BOTTOM, tx.hint_orbit, FontId::proportional(11.5), pal.muted);
    }

    fn panel(&mut self, ui: &mut egui::Ui, tx: &LabText) {
        let pal = self.pal();
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        ui.horizontal(|ui| {
            if ui.selectable_label(!self.live_mode, tx.tour).clicked() {
                self.live_mode = false;
                self.orbit = (0.0, 0.0);
            }
            if ui.selectable_label(self.live_mode, tx.live).clicked() {
                self.live_mode = true;
                self.orbit = (0.0, 0.0);
                self.zoom = 1.0;
            }
            ui.separator();
            if ui.selectable_label(!self.technical, tx.basic).clicked() {
                self.technical = false;
            }
            if ui.selectable_label(self.technical, tx.technical).clicked() {
                self.technical = true;
            }
        });
        ui.horizontal(|ui| {
            if ui.small_button("◀").clicked() {
                self.go(self.step + COUNT - 1);
            }
            for i in 0..COUNT {
                let on = !self.live_mode && i == self.step;
                let label = RichText::new((i + 1).to_string()).color(if on { pal.accent } else { pal.muted });
                if ui.selectable_label(on, label).on_hover_text(tx.stations[i].name).clicked() {
                    self.go(i);
                }
            }
            if ui.small_button("▶").clicked() {
                self.go(self.step + 1);
            }
            let icon = if self.playing { "⏸" } else { "⏵" };
            if ui.selectable_label(self.playing, icon).on_hover_text(tx.play_tip).clicked() {
                self.playing = !self.playing;
                self.step_at = Instant::now();
            }
        });
        ui.horizontal(|ui| {
            if ui.selectable_label(self.look == Look::Light, tx.look_light).clicked() {
                self.look = Look::Light;
            }
            if ui.selectable_label(self.look == Look::Dark, tx.look_dark).clicked() {
                self.look = Look::Dark;
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if self.live_mode {
                ui.label(RichText::new(tx.live_title).size(21.0).strong());
                ui.add_space(4.0);
                ui.label(RichText::new(tx.live_body).size(14.0).color(pal.body));
            } else {
                self.lesson(ui, tx);
            }
        });
    }

    fn lesson(&mut self, ui: &mut egui::Ui, tx: &LabText) {
        let st = &tx.stations[self.step];
        let pal = self.pal();
        ui.label(RichText::new(format!("{}. {}", self.step + 1, st.name)).size(21.0).strong());
        ui.label(RichText::new(st.question).size(15.0).color(pal.accent));
        ui.add_space(4.0);
        ui.label(RichText::new(if self.technical { st.tech } else { st.basic }).size(14.0).color(pal.body));
        if self.step == 4 {
            ui.add_space(6.0);
            let button = egui::Button::new(RichText::new(tx.nonce_plus).size(14.0).color(Color32::BLACK)).fill(ORANGE);
            if ui.add(button).clicked() {
                self.live.press_avalanche();
            }
            if let Some(a) = &self.live.avalanche {
                ui.label(RichText::new(fill(tx.avalanche_result, &[&a.flipped])).color(pal.accent));
                ui.label(RichText::new(format!("{:08x}: {}…", a.nonce, &rev_hex(&a.a)[..20])).monospace().size(11.0));
                ui.label(RichText::new(format!("{:08x}: {}…", a.nonce.wrapping_add(1), &rev_hex(&a.b)[..20])).monospace().size(11.0));
            }
        }
        ui.add_space(8.0);
        ui.label(RichText::new(tx.watch).size(11.0).strong().color(pal.muted));
        ui.label(RichText::new(st.watch).size(13.0).color(pal.muted));
        ui.add_space(8.0);
        ui.label(RichText::new(tx.real_data).size(11.0).strong().color(pal.muted));
        let values = facts(self.step, &self.live);
        egui::Grid::new("facts").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            for (i, (label, value)) in st.facts.iter().zip(values).enumerate() {
                let color = if self.step == 2 && i == active_field(self.started.elapsed().as_secs_f32()) { pal.accent } else { pal.fact };
                ui.label(RichText::new(*label).size(12.0).color(color));
                ui.label(RichText::new(value).size(12.0).monospace());
                ui.end_row();
            }
        });
    }

    fn debug_shots(&mut self, ctx: &egui::Context) {
        let Some(s) = &mut self.shots else { return };
        if Instant::now() < s.at {
            return;
        }
        // Eight stations, the technical text of the Target station, then the live overview.
        let n = s.next;
        s.next += 1;
        s.at = Instant::now() + Duration::from_secs(3);
        if n % 2 == 1 {
            let path = format!("{}/lab{:02}", s.dir, n / 2);
            ctx.request_screenshot(move |img| {
                let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                let _ = std::fs::write(format!("{path}_{}x{}.rgba", img.width(), img.height()), raw);
            });
            return;
        }
        match n / 2 {
            k if k < COUNT => {
                self.go(k);
                if k == 4 {
                    self.live.press_avalanche();
                }
            }
            k if k == COUNT => {
                self.go(5);
                self.technical = true;
            }
            k if k == COUNT + 1 => {
                self.technical = false;
                self.live_mode = true;
            }
            _ => std::process::exit(0),
        }
    }
}

/// Label on a small plate, readable over any surface of the model.
fn plated(painter: &egui::Painter, at: Pos2, align: Align2, text: &str, ink: Color32, pal: &Pal) {
    let galley = painter.layout_no_wrap(text.to_string(), FontId::monospace(11.5), ink);
    let rect = align.anchor_size(at, galley.size());
    painter.rect_filled(rect.expand2(Vec2::new(4.0, 2.0)), 3.0, pal.panel.gamma_multiply(0.85));
    painter.galley(rect.min, galley, ink);
}

/// Technical label tied to a point of the model by a thin leader.
fn callout(painter: &egui::Painter, view: &View, anchor: V3, offset: Vec2, label: &str, ink: Color32, pal: &Pal) {
    let Some((p, _)) = view.project(anchor) else { return };
    let end = p + offset;
    let shelf = end + Vec2::new(if offset.x < 0.0 { -16.0 } else { 16.0 }, 0.0);
    painter.line_segment([p, end], Stroke::new(1.0, ink.gamma_multiply(0.7)));
    painter.line_segment([end, shelf], Stroke::new(1.0, ink.gamma_multiply(0.7)));
    painter.circle_filled(p, 2.5, pal.accent);
    let (align, at) = if offset.x < 0.0 { (Align2::RIGHT_CENTER, shelf - Vec2::new(5.0, 0.0)) } else { (Align2::LEFT_CENTER, shelf + Vec2::new(5.0, 0.0)) };
    plated(painter, at, align, label, ink, pal);
}

fn short(hex: &str) -> String {
    format!("{}…{}", &hex[..8], &hex[hex.len() - 6..])
}

fn dash<T>(v: Option<T>, f: impl Fn(T) -> String) -> String {
    v.map_or_else(|| "—".to_string(), f)
}

/// The network is far beyond the units of the device screens.
fn rate(h: f64) -> String {
    if h >= 1e21 {
        format!("{:.2} ZH/s", h / 1e21)
    } else if h >= 1e18 {
        format!("{:.1} EH/s", h / 1e18)
    } else {
        let (v, u) = fmt::hashrate(h);
        format!("{v} {u}")
    }
}

/// Block subsidy in BTC at a height.
fn subsidy(height: u64) -> f64 {
    50.0 / 2f64.powi((height / 210_000).min(63) as i32)
}

/// Live values of a station, in the order of `StationText::facts`.
fn facts(step: usize, l: &Live) -> Vec<String> {
    const TWO32: f64 = 4_294_967_296.0;
    let h = &l.sample.header;
    let u32_at = |i: usize| u32::from_le_bytes(h[i..i + 4].try_into().expect("4 bytes"));
    let net_diff = (l.net_diff > 0.0).then_some(l.net_diff).or(l.net.net_difficulty);
    let mining = l.hashrate > 0.0;
    match step {
        0 => vec![
            dash(l.net.mempool.as_ref(), |m| fmt::big(m.count as f64)),
            dash(l.net.mempool.as_ref(), |m| format!("{:.1} MvB", m.vsize / 1e6)),
            dash(l.net.fee_fastest, |f| format!("{} sat/vB", fmt::feerate(f))),
            dash(l.net.fee_half_hour, |f| format!("{} sat/vB", fmt::feerate(f))),
        ],
        1 => vec![
            l.branches.to_string(),
            fmt::big(2f64.powi(l.branches as i32)),
            format!("{:x}", l.en2),
            short(&rev_hex(&h[36..68])),
        ],
        2 => vec![
            format!("{:08x}", u32_at(0)),
            short(&rev_hex(&h[4..36])),
            short(&rev_hex(&h[36..68])),
            chrono::DateTime::from_timestamp(u32_at(68) as i64, 0)
                .map_or_else(String::new, |d| d.format("%Y-%m-%d %H:%M UTC").to_string()),
            format!("{:08x}", u32_at(72)),
            format!("{:08x}", u32_at(76)),
        ],
        3 => vec![
            fmt::num(l.sample.nonce as f64, 0),
            rate(l.hashrate),
            dash(mining.then_some(l.hashrate), |r| fmt::duration(TWO32 / r)),
            fmt::big(l.total as f64),
        ],
        4 => vec![
            short(&rev_hex(&l.sample.hash)),
            dash(l.avalanche.as_ref(), |a| format!("{} / 256", a.flipped)),
            dash(mining.then(|| l.hashes_per_sample()), fmt::big),
        ],
        5 => vec![
            l.sample.bits.to_string(),
            dash((l.best > 0.0).then_some(l.best), |d| format!("{:.0}", diff_bits(d))),
            dash((l.pool_diff > 0.0).then_some(l.pool_diff), |d| format!("{:.0}", diff_bits(d))),
            dash(net_diff, |d| format!("{:.0}", diff_bits(d))),
            dash(net_diff, fmt::diff),
        ],
        6 => {
            let diff = net_diff.filter(|_| mining);
            vec![
                rate(l.hashrate),
                dash(l.net.net_hashrate, rate),
                dash(l.net.net_hashrate.filter(|_| mining), |n| format!("1 / {}", fmt::big(n / l.hashrate))),
                dash(diff, |d| fmt::odds(1.0 - (-l.hashrate * 86_400.0 / (d * TWO32)).exp())),
                dash(diff, |d| fmt::duration(d * TWO32 / l.hashrate)),
            ]
        }
        _ => {
            let b = l.net.last_block.as_ref();
            let now = chrono::Utc::now().timestamp() as f64;
            vec![
                dash(b, |b| fmt::num(b.height as f64, 0)),
                dash(b, |b| b.pool.clone()),
                dash(b, |b| fmt::duration((now - b.time as f64).max(0.0))),
                dash(b, |b| format!("{} BTC", subsidy(b.height))),
                dash(l.net.diff_adj.as_ref(), |d| format!("{:+.1}% · {}", d.change_pct, fmt::duration(d.remaining_secs))),
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_rate_uses_big_units() {
        assert_eq!(rate(1.03e21), "1.03 ZH/s");
        assert_eq!(rate(650e18), "650.0 EH/s");
        assert_eq!(rate(250_000.0), "250.0 KH/s");
    }

    #[test]
    fn subsidy_halves() {
        assert_eq!(subsidy(0), 50.0);
        assert_eq!(subsidy(840_000), 3.125);
        assert_eq!(subsidy(1_050_000), 1.5625);
    }
}
