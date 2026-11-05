//! Desktop "device" UI that mimics the NerdMiner TFT screens.

use crate::config::Config;
use crate::i18n::{self, fill, t};
use crate::lab::Handle;
use crate::miner::max_threads;
use crate::screens::{Lcd, Screen, Screens};
use crate::state::Shared;
use crate::tray::{Device, Tray, TrayCmd};
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
const HEADER_H: f32 = 24.0;
const LOGO: f32 = 16.0;
const BRAND_URL: &str = "https://gobtc.com.br";
pub const COMPACT: Vec2 = Vec2::new(SCREEN.x + BEZEL * 2.0, SCREEN.y + BEZEL * 2.0 + TOOLBAR_H + HEADER_H);
const SETTINGS_H: f32 = 458.0;


pub struct NerdApp {
    shared: Arc<Shared>,
    tray: Option<Tray>,
    tray_rx: crossbeam_channel::Receiver<TrayCmd>,
    visible: bool,
    screen: Screen,
    screens: Option<Screens>,
    zoom: f32,
    logo: Option<egui::TextureHandle>,
    shot_done: Option<usize>,
    settings_open: bool,
    draft: Config,
    last_tray_update: Instant,
    /// `None` until the lab window is opened, and again once it is closed: the lab uses no
    /// memory unless it is on screen.
    lab: Option<Handle>,
    /// Surface format, needed by the lab's studio renderer; a plain value, costs nothing.
    lab_format: Option<eframe::wgpu::TextureFormat>,
}

impl NerdApp {
    pub fn new(cc: &eframe::CreationContext<'_>, shared: Arc<Shared>) -> Self {
        Self::debug_rate_log(&shared);
        let (tx, tray_rx) = crossbeam_channel::unbounded();
        let ctx = cc.egui_ctx.clone();
        let tray = match Tray::new(move |cmd| {
            let _ = tx.send(cmd);
            ctx.request_repaint();
        }) {
            Ok(t) => Some(t),
            Err(e) => {
                shared.log(fill(t().log_tray_unavailable, &[&e]));
                None
            }
        };
        let cfg = shared.config.read().unwrap().clone();
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut app = NerdApp {
            visible: !cfg.start_hidden || tray.is_none(),
            draft: cfg.clone(),
            shared,
            tray,
            tray_rx,
            screen: Screen::Miner,
            screens: None,
            logo: None,
            shot_done: None,
            zoom: cfg.zoom.clamp(1.0, 3.0),
            settings_open: cfg.btc_address.trim().is_empty(),
            last_tray_update: Instant::now() - Duration::from_secs(10),
            lab: None,
            lab_format: cc.wgpu_render_state.as_ref().map(|s| s.target_format),
        };
        crate::screens::install_fonts(&cc.egui_ctx);
        if std::env::var_os("NERDMINER_LAB").is_some() {
            app.open_lab();
        }
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
        if let Some(tray) = &mut self.tray {
            tray.set_window_visible(v);
        }
    }

    /// Debug: NERDMINER_RATE_LOG=1 prints the hashrate every 5 s, lab open or not.
    fn debug_rate_log(shared: &Arc<Shared>) {
        if std::env::var_os("NERDMINER_RATE_LOG").is_none() {
            return;
        }
        let shared = shared.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            eprintln!("rate {:.0}", shared.hashrate());
        });
    }

    fn open_lab(&mut self) {
        if self.lab.is_none() {
            self.lab = Some(crate::lab::open(&self.shared, self.lab_format));
        }
    }

    /// Debug: NERDMINER_LAB_CYCLE=1 opens and closes the lab every 5 s, to check that closing
    /// gives the memory back.
    fn debug_lab_cycle(&mut self) {
        let Ok(mode) = std::env::var("NERDMINER_LAB_CYCLE") else { return };
        let secs = self.shared.started.elapsed().as_secs();
        // "once" opens it a single time, to watch what memory does long after closing.
        let open = if mode == "once" { (5..15).contains(&secs) } else { secs / 5 % 2 == 1 };
        if open {
            self.open_lab();
        } else {
            self.lab = None;
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
        cfg.tray_fields = self.draft.tray_fields.clone();
        cfg.tx_vbytes = self.draft.tx_vbytes;
        cfg.currency = self.draft.currency.clone();
        cfg.language = self.draft.language.clone();
        i18n::set(i18n::resolve(&cfg.language));
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
        if let Some(t) = &mut self.tray {
            t.refresh(&self.shared);
        }
    }

    fn handle_tray(&mut self, ctx: &egui::Context, cmd: TrayCmd) {
        match cmd {
            TrayCmd::Toggle => {
                let v = !self.visible;
                self.set_visible(ctx, v);
            }
            TrayCmd::Pause => self.toggle_pause(),
            TrayCmd::Quit => self.quit(),
            TrayCmd::Device(d) => {
                self.draft.cpu_enabled = matches!(d, Device::Cpu | Device::Both);
                self.draft.gpu_enabled = matches!(d, Device::Gpu | Device::Both);
                self.persist_runtime();
            }
            TrayCmd::CpuUsage(p) => {
                self.draft.cpu_usage = p;
                self.persist_runtime();
            }
            TrayCmd::GpuUsage(p) => {
                self.draft.gpu_usage = p;
                self.persist_runtime();
            }
            TrayCmd::Language(l) => {
                self.draft.language = l.to_string();
                self.persist_runtime();
            }
            TrayCmd::Currency(c) => {
                self.draft.currency = c.to_string();
                self.persist_runtime();
            }
            TrayCmd::TxSize(vb) => {
                self.draft.tx_vbytes = vb;
                self.persist_runtime();
            }
            TrayCmd::Field(k) => {
                let f = &mut self.draft.tray_fields;
                if let Some(i) = f.iter().position(|x| x == k) {
                    f.remove(i);
                } else {
                    // Keep the order of the menu, not the order of clicks.
                    f.push(k.to_string());
                    f.sort_by_key(|x| crate::tray::FIELDS.iter().position(|key| key == x));
                }
                self.persist_runtime();
            }
            TrayCmd::OpenLab => self.open_lab(),
            TrayCmd::OpenPool => {
                if let Some(u) = crate::tray::pool_url(&self.shared.config.read().unwrap()) {
                    crate::tray::open_url(&u);
                }
            }
            TrayCmd::OpenMempool => {
                let hash = self.shared.net.lock().unwrap().last_block.as_ref().map(|b| b.hash.clone());
                crate::tray::open_url(&match hash {
                    Some(h) => format!("https://mempool.space/block/{h}"),
                    None => "https://mempool.space/".into(),
                });
            }
        }
        // Reflect the change in the menu right away.
        self.last_tray_update = Instant::now() - Duration::from_secs(10);
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
        resp.on_hover_text(t().hover_screen);
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

    /// Top strip: discreet brand logo (top-left, aligned with the screen) + window drag area.
    fn draw_header(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        let (bar, bar_resp) = ui.allocate_exact_size(Vec2::new(width, HEADER_H), Sense::click_and_drag());
        if bar_resp.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
        let logo = self.logo.get_or_insert_with(|| load_logo(ui.ctx()));
        let rect = Rect::from_min_size(
            egui::pos2(bar.left() + BEZEL, bar.center().y - LOGO / 2.0 + 2.0),
            Vec2::splat(LOGO),
        );
        let resp = ui.interact(rect, ui.scope_id().with("brand_logo"), Sense::click());
        let alpha = if resp.hovered() { 1.0 } else { 0.55 };
        ui.painter().image(
            logo.id(),
            rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE.gamma_multiply(alpha),
        );
        if resp.clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(BRAND_URL));
        }
        resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("gobtc.com.br");
    }

    fn next_screen(&mut self, dir: i32) {
        let all = Screen::ALL;
        let i = all.iter().position(|s| *s == self.screen).unwrap() as i32;
        self.screen = all[((i + dir).rem_euclid(all.len() as i32)) as usize];
    }

    fn window_size(&self) -> Vec2 {
        let w = SCREEN.x * self.zoom + BEZEL * 2.0;
        let h = SCREEN.y * self.zoom + BEZEL * 2.0 + TOOLBAR_H + HEADER_H;
        Vec2::new(w.max(COMPACT.x), if self.settings_open { h + SETTINGS_H } else { h })
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.horizontal(|ui| {
            ui.add_space(BEZEL - 4.0);
            if ui.small_button("◀").on_hover_text(t().prev_screen).clicked() {
                self.next_screen(-1);
            }
            if ui.small_button("▶").on_hover_text(t().next_screen).clicked() {
                self.next_screen(1);
            }
            let paused = self.shared.paused.load(Relaxed);
            if ui.small_button(if paused { t().btn_mine } else { t().btn_pause }).clicked() {
                self.toggle_pause();
            }
            if ui.selectable_label(self.lab.is_some(), "🎓").on_hover_text(t().open_lab).clicked() {
                self.open_lab();
            }
            if ui.selectable_label(self.settings_open, "⚙").on_hover_text(t().settings).clicked() {
                let o = !self.settings_open;
                self.set_settings_open(&ctx, o);
            }
            if ui.selectable_label(self.draft.always_on_top, "📌").on_hover_text(t().always_on_top).clicked() {
                self.draft.always_on_top = !self.draft.always_on_top;
                ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if self.draft.always_on_top {
                    WindowLevel::AlwaysOnTop
                } else {
                    WindowLevel::Normal
                }));
                self.persist_runtime();
            }
            if self.tray.is_some() && ui.small_button(t().hide).on_hover_text(t().hide_tip).clicked() {
                self.set_visible(&ctx, false);
            }
            if ui.small_button(t().quit).on_hover_text(t().quit_tip).clicked() {
                self.quit();
            }
        });
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(4.0);
            ui.heading("Pool");
            egui::Grid::new("pool").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label(t().btc_address);
                ui.add(egui::TextEdit::singleline(&mut self.draft.btc_address).hint_text("bc1q…").desired_width(220.0));
                ui.end_row();
                ui.label(t().pool);
                ui.add(egui::TextEdit::singleline(&mut self.draft.pool_host).desired_width(220.0));
                ui.end_row();
                ui.label(t().port);
                ui.add(egui::DragValue::new(&mut self.draft.pool_port).range(1..=65535));
                ui.end_row();
                ui.label(t().worker);
                ui.add(egui::TextEdit::singleline(&mut self.draft.worker_name).desired_width(220.0));
                ui.end_row();
                ui.label(t().password);
                ui.add(egui::TextEdit::singleline(&mut self.draft.pool_password).desired_width(220.0));
                ui.end_row();
                ui.label(t().suggest_diff);
                ui.add(egui::DragValue::new(&mut self.draft.suggest_diff).range(0.0..=1e9).speed(0.01))
                    .on_hover_text(t().suggest_diff_tip);
                ui.end_row();
            });
            ui.horizontal(|ui| {
                if ui.button(t().save_reconnect).clicked() {
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
            ui.heading(t().h_mining);
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label(t().device);
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
                    .add(egui::Slider::new(&mut self.draft.cpu_threads, 1..=max_threads()).text(t().threads))
                    .changed();
                changed |= ui.add(egui::Slider::new(&mut self.draft.cpu_usage, 1..=100).text(t().cpu_usage_slider)).changed();
                let est = self.draft.cpu_threads as f64 * self.draft.cpu_usage as f64 / max_threads() as f64;
                ui.weak(fill(t().cpu_total, &[&format!("{est:.0}"), &max_threads()]));
            });

            ui.add_enabled_ui(self.draft.gpu_enabled, |ui| {
                ui.label(egui::RichText::new("GPU").strong());
                changed |= ui.add(egui::Slider::new(&mut self.draft.gpu_usage, 1..=100).text(t().gpu_usage_slider)).changed();
                ui.weak(fill(t().status, &[&self.shared.gpu_status.lock().unwrap().text()]));
            });

            ui.separator();
            ui.heading(t().h_window);
            changed |= ui.checkbox(&mut self.draft.start_hidden, t().start_hidden).changed();
            ui.horizontal(|ui| {
                ui.label(t().zoom);
                for z in [1.0f32, 1.5, 2.0, 2.5] {
                    if ui.selectable_label(self.zoom == z, format!("{z}x")).clicked() {
                        self.zoom = z;
                        self.draft.zoom = z;
                        changed = true;
                        ui.ctx().send_viewport_cmd(ViewportCommand::InnerSize(self.window_size()));
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label(t().currency);
                let cur = crate::currency::code(&self.draft.currency);
                let label = |c: &str| format!("{c} · {} · {}", crate::currency::symbol(c), crate::currency::name(c));
                egui::ComboBox::from_id_salt("currency").selected_text(label(cur)).width(220.0).show_ui(ui, |ui| {
                    for (code, _) in crate::currency::CURRENCIES {
                        if ui.selectable_label(cur == code, label(code)).clicked() {
                            self.draft.currency = code.to_string();
                            changed = true;
                        }
                    }
                });
            })
            .response
            .on_hover_text(t().currency_tip);
            ui.horizontal(|ui| {
                ui.label(t().language);
                let auto = self.draft.language.trim().eq_ignore_ascii_case("auto");
                let current = if auto { t().language_auto.to_string() } else { i18n::resolve(&self.draft.language).native_name().to_string() };
                egui::ComboBox::from_id_salt("language").selected_text(current).width(220.0).show_ui(ui, |ui| {
                    if ui.selectable_label(auto, t().language_auto).clicked() {
                        self.draft.language = "auto".into();
                        changed = true;
                    }
                    for l in i18n::Lang::ALL {
                        if ui.selectable_label(!auto && i18n::resolve(&self.draft.language) == l, l.native_name()).clicked() {
                            self.draft.language = l.code().into();
                            changed = true;
                        }
                    }
                });
            });
            if changed {
                self.persist_runtime();
            }

            ui.separator();
            ui.heading(t().h_log);
            egui::ScrollArea::vertical().id_salt("log").max_height(120.0).stick_to_bottom(true).show(ui, |ui| {
                for l in self.shared.log.lock().unwrap().iter() {
                    ui.monospace(egui::RichText::new(l).size(10.0));
                }
            });
            ui.add_space(6.0);
            ui.weak(t().lottery_note);
        });
    }
}

impl eframe::App for NerdApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(cmd) = self.tray_rx.try_recv() {
            self.handle_tray(ctx, cmd);
        }
        self.update_tray();
        self.debug_lab_cycle();
        // Deferred viewports stay open only while shown from every root pass.
        if self.lab.as_ref().is_some_and(crate::lab::closed) {
            self.lab = None;
        }
        if let Some(lab) = &self.lab {
            crate::lab::show(ctx, &self.shared, lab);
        }
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Frame::new().fill(Color32::from_rgb(16, 16, 20)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            ui.set_min_size(ui.available_size());
            self.debug_shots(ui.ctx());
            self.draw_header(ui);
            self.draw_device(ui);
            self.toolbar(ui);
            if self.settings_open {
                egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 4)).show(ui, |ui| self.settings(ui));
            }
        });
    }
}

fn load_logo(ctx: &egui::Context) -> egui::TextureHandle {
    let img = image::load_from_memory(include_bytes!("../assets/brand/gobtc-64.png"))
        .expect("embedded logo")
        .to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    ctx.load_texture("gobtc-logo", color, egui::TextureOptions::LINEAR)
}
