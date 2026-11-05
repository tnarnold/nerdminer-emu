#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod config;
mod currency;
mod fmt;
mod gpu;
mod i18n;
mod lab;
mod miner;
mod netinfo;
mod screens;
mod state;
mod stratum;
mod tray;
mod work;

use eframe::egui;

fn main() -> eframe::Result {
    let cfg = config::Config::load();
    i18n::set(i18n::resolve(&cfg.language));
    let shared = state::Shared::new(cfg.clone());

    stratum::spawn(shared.clone());
    miner::spawn_cpu(shared.clone());
    miner::spawn_stats(shared.clone());
    gpu::spawn(shared.clone());
    netinfo::spawn(shared.clone());

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("NerdMiner Emulator")
        .with_inner_size(app::COMPACT * 1.4)
        .with_resizable(false)
        .with_decorations(false)
        .with_visible(!cfg.start_hidden);
    // Inside a macOS .app bundle the Dock uses AppIcon.icns; elsewhere set the runtime icon.
    if !in_app_bundle() {
        viewport = viewport.with_icon(egui::IconData { rgba: tray::icon_rgba(64), width: 64, height: 64 });
    }
    if cfg.always_on_top {
        viewport = viewport.with_always_on_top();
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "NerdMiner Emulator",
        options,
        Box::new(move |cc| Ok(Box::new(app::NerdApp::new(cc, shared)))),
    )
}

fn in_app_bundle() -> bool {
    cfg!(target_os = "macos")
        && std::env::current_exe()
            .map(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
            .unwrap_or(false)
}
