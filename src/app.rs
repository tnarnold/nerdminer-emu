//! Desktop "device" UI that mimics the NerdMiner TFT screens.

use crate::config::Config;
use crate::fmt;
use crate::miner::max_threads;
use crate::screens::{Lcd, Screen, Screens};
use crate::state::Shared;
use crate::tray::{Tray, TrayCmd};
use eframe::egui::{
    self, Color32, CornerRadius, Rect, Sense, Stroke, Vec2, ViewportCommand,
    WindowLevel,
};
use std::sync::Arc;
use std::sync::atomic::Ordering::*;
use std::time::{Duration, Instant};

pub const SCREEN: Vec2 = Vec2::new(320.0, 170.0);
const BEZEL: f32 = 12.0;
const TOOLBAR_H: f32 = 34.0;
pub const COMPACT: Vec2 = Vec2::new(SCREEN.x + BEZEL * 2.0, SCREEN.y + BEZEL * 2.0 + TOOLBAR_H);
const SETTINGS_H: f32 = 430.0;


pub struct NerdApp {
    shared: Arc<Shared>,
    tray: Option<Tray>,
    tray_rx: crossbeam_channel::Receiver<TrayCmd>,
    visible: bool,
    screen: Screen,
    screens: Option<Screens>,
    zoom: f32,
    shot_done: Option<usize>,
    settings_open: bool,
    draft: Config,
    last_tray_update: Instant,
}

impl NerdApp {
    pub fn new(cc: &eframe::CreationContext<'_>, shared: Arc<Shared>) -> Self {
        let (tx, tray_rx) = crossbeam_channel::unbounded();
        let ctx = cc.egui_ctx.clone();
        let tray = match Tray::new(move |cmd| {
            let _ = tx.send(cmd);
            ctx.request_repaint();
        }) {
            Ok(t) => Some(t),
            Err(e) => {
                shared.log(format!("Bandeja indisponível: {e}"));
                None
            }
        };
        let cfg = shared.config.read().unwrap().clone();
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let app = NerdApp {
            visible: !cfg.start_hidden || tray.is_none(),
            draft: cfg.clone(),
            shared,
            tray,
            tray_rx,
            screen: Screen::Miner,
            screens: None,
            shot_done: None,
            zoom: cfg.zoom.clamp(1.0, 3.0),
            settings_open: cfg.btc_address.trim().is_empty(),
            last_tray_update: Instant::now() - Duration::from_secs(10),
        };
        crate::screens::install_fonts(&cc.egui_ctx);
        cc.egui_ctx.send_viewport_cmd(ViewportCommand::InnerSize(app.window_size()));
        app
    }

    fn set_visible(&mut self, ctx: &egui::Context, v: bool) {
        self.visible = v;
        ctx.send_viewport_cmd(ViewportCommand::Visible(v));
        if v {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }
        if let Some(t) = &self.tray {
            t.toggle_item.set_text(if v { "Ocultar janela" } else { "Mostrar janela" });
        }
    }

    fn toggle_pause(&mut self) {
        let p = !self.shared.paused.load(Relaxed);
        self.shared.paused.store(p, Relaxed);
        self.draft.start_paused = p;
        self.persist_runtime();
    }

    fn quit(&self) -> ! {
        self.shared.config.read().unwrap().save();
        std::process::exit(0);
    }

    fn set_settings_open(&mut self, ctx: &egui::Context, open: bool) {
        self.settings_open = open;
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(self.window_size()));
    }

    /// Push the live (non-pool) settings to the miners and save.
    fn persist_runtime(&mut self) {
        let s = &self.shared;
        s.cpu_enabled.store(self.draft.cpu_enabled, Relaxed);
        s.cpu_threads.store(self.draft.cpu_threads.clamp(1, max_threads()), Relaxed);
        s.cpu_usage.store(self.draft.cpu_usage.clamp(1, 100), Relaxed);
        s.gpu_enabled.store(self.draft.gpu_enabled, Relaxed);
        s.gpu_usage.store(self.draft.gpu_usage.clamp(1, 100), Relaxed);
        let mut cfg = s.config.write().unwrap();
        cfg.cpu_enabled = self.draft.cpu_enabled;
        cfg.cpu_threads = self.draft.cpu_threads;
        cfg.cpu_usage = self.draft.cpu_usage;
        cfg.gpu_enabled = self.draft.gpu_enabled;
        cfg.gpu_usage = self.draft.gpu_usage;
        cfg.always_on_top = self.draft.always_on_top;
        cfg.start_hidden = self.draft.start_hidden;
        cfg.zoom = self.draft.zoom;
        cfg.start_paused = s.paused.load(Relaxed);
        cfg.save();
    }

    fn apply_pool(&mut self) {
        {
            let mut cfg = self.shared.config.write().unwrap();
            cfg.pool_host = self.draft.pool_host.trim().to_string();
            cfg.pool_port = self.draft.pool_port;
            cfg.btc_address = self.draft.btc_address.trim().to_string();
            cfg.worker_name = self.draft.worker_name.trim().to_string();
            cfg.pool_password = self.draft.pool_password.clone();
            cfg.suggest_diff = self.draft.suggest_diff;
            cfg.save();
        }
        self.shared.config_gen.fetch_add(1, Release);
    }

    fn update_tray(&mut self) {
        if self.last_tray_update.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.last_tray_update = Instant::now();
        let Some(t) = &self.tray else { return };
        let paused = self.shared.paused.load(Relaxed);
        let (v, u) = fmt::hashrate(self.shared.hashrate());
        let text = if paused { "Pausado".to_string() } else { format!("{v} {u}") };
        let _ = t.icon.set_tooltip(Some(format!("NerdMiner Emulator — {text}")));
        #[cfg(target_os = "macos")]
        t.icon.set_title(Some(format!(" {text}")));
        t.pause_item.set_text(if paused { "Retomar mineração" } else { "Pausar mineração" });
    }

    // ---------------------------------------------------------------- drawing

    fn draw_device(&mut self, ui: &mut egui::Ui) {
        let z = self.zoom;
        let size = Vec2::new(SCREEN.x * z + BEZEL * 2.0, SCREEN.y * z + BEZEL * 2.0);
        let (outer, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let p = ui.painter_at(outer);
        p.rect_filled(outer, CornerRadius::same(14), Color32::from_rgb(22, 22, 26));
        p.rect_stroke(outer.shrink(1.0), CornerRadius::same(13), Stroke::new(1.0, Color32::from_gray(55)), egui::StrokeKind::Inside);
        let screen = Rect::from_min_size(outer.min + Vec2::splat(BEZEL), SCREEN * z);
        let painter = ui.painter_at(screen);
        let screens = self.screens.get_or_insert_with(|| Screens::new(ui.ctx()));
        let lcd = Lcd { p: &painter, rect: screen, s: z };
        if self.shared.started.elapsed() < Duration::from_secs(3) {
            screens.loading(&lcd);
        } else {
            screens.draw(&lcd, self.screen, &self.shared);
        }

        if resp.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if resp.clicked() {
            self.next_screen(1);
        }
        resp.on_hover_text("Clique: próxima tela • Arraste: mover");
    }

    /// Debug: with NERDMINER_SHOTS=<dir>, save a raw RGBA screenshot of each screen and exit.
    fn debug_shots(&mut self, ctx: &egui::Context) {
        let Ok(dir) = std::env::var("NERDMINER_SHOTS") else { return };
        let t = self.shared.started.elapsed().as_secs_f32();
        let step = ((t - 8.0) / 1.5).floor();
        ctx.request_repaint_after(Duration::from_millis(100));
        if step < 0.0 {
            return;
        }
        let step = step as usize;
        if step >= Screen::ALL.len() + 1 {
            std::process::exit(0);
        }
        if self.shot_done == Some(step) {
            return;
        }
        self.shot_done = Some(step);
        if step < Screen::ALL.len() {
            self.screen = Screen::ALL[step];
        } else if !self.settings_open {
            self.set_settings_open(ctx, true);
        }
        let path = format!("{dir}/shot{step}");
        ctx.request_screenshot(move |img| {
            let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            let _ = std::fs::write(format!("{path}_{}x{}.rgba", img.width(), img.height()), raw);
        });
    }

    fn next_screen(&mut self, dir: i32) {
        let all = Screen::ALL;
        let i = all.iter().position(|s| *s == self.screen).unwrap() as i32;
        self.screen = all[((i + dir).rem_euclid(all.len() as i32)) as usize];
    }

    fn window_size(&self) -> Vec2 {
        let w = SCREEN.x * self.zoom + BEZEL * 2.0;
        let h = SCREEN.y * self.zoom + BEZEL * 2.0 + TOOLBAR_H;
        Vec2::new(w.max(COMPACT.x), if self.settings_open { h + SETTINGS_H } else { h })
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal(|ui| {
            ui.add_space(BEZEL - 4.0);
            if ui.small_button("◀").on_hover_text("Tela anterior").clicked() {
                self.next_screen(-1);
            }
            if ui.small_button("▶").on_hover_text("Próxima tela").clicked() {
                self.next_screen(1);
            }
            let paused = self.shared.paused.load(Relaxed);
            if ui.small_button(if paused { "▶ Minerar" } else { "⏸ Pausar" }).clicked() {
                self.toggle_pause();
            }
            if ui.selectable_label(self.settings_open, "⚙").on_hover_text("Configurações").clicked() {
                let o = !self.settings_open;
                self.set_settings_open(&ctx, o);
            }
            if ui.selectable_label(self.draft.always_on_top, "📌").on_hover_text("Sempre no topo").clicked() {
                self.draft.always_on_top = !self.draft.always_on_top;
                ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if self.draft.always_on_top {
                    WindowLevel::AlwaysOnTop
                } else {
                    WindowLevel::Normal
                }));
                self.persist_runtime();
            }
            if self.tray.is_some() && ui.small_button("Esconder").on_hover_text("Esconder na bandeja do sistema").clicked() {
                self.set_visible(&ctx, false);
            }
            if ui.small_button("Sair").on_hover_text("Encerrar o minerador").clicked() {
                self.quit();
            }
        });
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(4.0);
            ui.heading("Pool");
            egui::Grid::new("pool").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label("Endereço BTC");
                ui.add(egui::TextEdit::singleline(&mut self.draft.btc_address).hint_text("bc1q…").desired_width(220.0));
                ui.end_row();
                ui.label("Pool");
                ui.add(egui::TextEdit::singleline(&mut self.draft.pool_host).desired_width(220.0));
                ui.end_row();
                ui.label("Porta");
                ui.add(egui::DragValue::new(&mut self.draft.pool_port).range(1..=65535));
                ui.end_row();
                ui.label("Worker");
                ui.add(egui::TextEdit::singleline(&mut self.draft.worker_name).desired_width(220.0));
                ui.end_row();
                ui.label("Senha");
                ui.add(egui::TextEdit::singleline(&mut self.draft.pool_password).desired_width(220.0));
                ui.end_row();
                ui.label("Dif. sugerida");
                ui.add(egui::DragValue::new(&mut self.draft.suggest_diff).range(0.0..=1e9).speed(0.01))
                    .on_hover_text("Enviada via mining.suggest_difficulty (0 = não enviar)");
                ui.end_row();
            });
            ui.horizontal(|ui| {
                if ui.button("Salvar e reconectar").clicked() {
                    self.apply_pool();
                }
                for (name, host, port) in [("public-pool", "public-pool.io", 21496u16), ("ckpool", "solo.ckpool.org", 3333)] {
                    if ui.small_button(name).clicked() {
                        self.draft.pool_host = host.into();
                        self.draft.pool_port = port;
                    }
                }
            });

            ui.separator();
            ui.heading("Mineração");
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("Dispositivo");
                let (c, g) = (self.draft.cpu_enabled, self.draft.gpu_enabled);
                for (label, cpu, gpu) in [("CPU", true, false), ("GPU", false, true), ("CPU + GPU", true, true)] {
                    if ui.selectable_label(c == cpu && g == gpu, label).clicked() {
                        self.draft.cpu_enabled = cpu;
                        self.draft.gpu_enabled = gpu;
                        changed = true;
                    }
                }
            });

            ui.add_enabled_ui(self.draft.cpu_enabled, |ui| {
                ui.label(egui::RichText::new("CPU").strong());
                changed |= ui
                    .add(egui::Slider::new(&mut self.draft.cpu_threads, 1..=max_threads()).text("threads"))
                    .changed();
                changed |= ui.add(egui::Slider::new(&mut self.draft.cpu_usage, 1..=100).text("% uso por thread")).changed();
                let est = self.draft.cpu_threads as f64 * self.draft.cpu_usage as f64 / max_threads() as f64;
                ui.weak(format!("≈ {est:.0}% da CPU total ({} núcleos lógicos)", max_threads()));
            });

            ui.add_enabled_ui(self.draft.gpu_enabled, |ui| {
                ui.label(egui::RichText::new("GPU").strong());
                changed |= ui.add(egui::Slider::new(&mut self.draft.gpu_usage, 1..=100).text("% uso da GPU")).changed();
                ui.weak(format!("Status: {}", self.shared.gpu_status.lock().unwrap()));
            });

            ui.separator();
            ui.heading("Janela");
            changed |= ui.checkbox(&mut self.draft.start_hidden, "Iniciar escondido na bandeja").changed();
            ui.horizontal(|ui| {
                ui.label("Zoom da tela");
                for z in [1.0f32, 1.5, 2.0, 2.5] {
                    if ui.selectable_label(self.zoom == z, format!("{z}x")).clicked() {
                        self.zoom = z;
                        self.draft.zoom = z;
                        changed = true;
                        ui.ctx().send_viewport_cmd(ViewportCommand::InnerSize(self.window_size()));
                    }
                }
            });
            if changed {
                self.persist_runtime();
            }

            ui.separator();
            ui.heading("Log");
            egui::ScrollArea::vertical().id_salt("log").max_height(120.0).stick_to_bottom(true).show(ui, |ui| {
                for l in self.shared.log.lock().unwrap().iter() {
                    ui.monospace(egui::RichText::new(l).size(10.0));
                }
            });
            ui.add_space(6.0);
            ui.weak("Mineração solo é uma loteria: a chance de achar um bloco com CPU/GPU é minúscula.");
        });
    }
}

impl eframe::App for NerdApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(cmd) = self.tray_rx.try_recv() {
            match cmd {
                TrayCmd::Toggle => {
                    let v = !self.visible;
                    self.set_visible(ctx, v);
                }
                TrayCmd::Pause => self.toggle_pause(),
                TrayCmd::Quit => self.quit(),
            }
        }
        self.update_tray();
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Frame::new().fill(Color32::from_rgb(16, 16, 20)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            ui.set_min_size(ui.available_size());
            self.debug_shots(ui.ctx());
            self.draw_device(ui);
            self.toolbar(ui);
            if self.settings_open {
                egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| self.settings(ui));
            }
        });
    }
}
