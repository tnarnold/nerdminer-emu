//! System tray icon (macOS menu bar, Windows notification area, Linux SNI/ksni).
//!
//! The icon carries a status dot (mining / connecting / offline / paused); on macOS the
//! menu bar also shows the fields picked by the user. The menu is a native, text-only
//! dashboard: mining stats, network occupancy with the fee to use right now, and quick
//! controls (device, CPU/GPU usage, transaction size, menu bar fields).

use crate::config::Config;
use crate::fmt;
use crate::netinfo::{self, Tier};
use crate::state::Shared;
use std::sync::Arc;
use std::sync::atomic::Ordering::*;
use std::time::{SystemTime, UNIX_EPOCH};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub const CPU_STEPS: [u32; 5] = [10, 25, 50, 75, 100];
pub const GPU_STEPS: [u32; 5] = [10, 30, 50, 75, 100];
/// (vB, label) — 1 input / 2 outputs P2WPKH, P2TR, and a 5→1 P2WPKH consolidation.
pub const TX_SIZES: [(u32, &str); 3] = [
    (141, "Simples 1→2, SegWit · 141 vB"),
    (154, "Taproot 1→2 · 154 vB"),
    (382, "Consolidação 5→1 · 382 vB"),
];
pub const FIELDS: [(&str, &str); 5] = [
    ("hashrate", "Hashrate"),
    ("fee", "Taxa recomendada + ocupação"),
    ("last_block", "Tempo desde o último bloco"),
    ("best_diff", "Melhor dificuldade"),
    ("price", "Preço do BTC"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Device {
    Cpu,
    Gpu,
    Both,
}

pub enum TrayCmd {
    Toggle,
    Pause,
    Quit,
    Device(Device),
    CpuUsage(u32),
    GpuUsage(u32),
    TxSize(u32),
    Field(&'static str),
    OpenPool,
    OpenMempool,
}

fn parse_cmd(id: &str) -> Option<TrayCmd> {
    let (kind, arg) = id.split_once(':').unwrap_or((id, ""));
    Some(match kind {
        "toggle" => TrayCmd::Toggle,
        "pause" => TrayCmd::Pause,
        "quit" => TrayCmd::Quit,
        "pool" => TrayCmd::OpenPool,
        "mempool" => TrayCmd::OpenMempool,
        "dev" => TrayCmd::Device(match arg {
            "cpu" => Device::Cpu,
            "gpu" => Device::Gpu,
            _ => Device::Both,
        }),
        "cpu" => TrayCmd::CpuUsage(arg.parse().ok()?),
        "gpu" => TrayCmd::GpuUsage(arg.parse().ok()?),
        "tx" => TrayCmd::TxSize(arg.parse().ok()?),
        "field" => TrayCmd::Field(FIELDS.iter().find(|(k, _)| *k == arg)?.0),
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Status {
    Mining,
    Connecting,
    Offline,
    Paused,
}

impl Status {
    fn color(self) -> [u8; 3] {
        match self {
            Status::Mining => [52, 199, 89],
            Status::Connecting => [255, 204, 0],
            Status::Offline => [255, 59, 48],
            Status::Paused => [150, 150, 150],
        }
    }
    fn label(self) -> &'static str {
        match self {
            Status::Mining => "Minerando",
            Status::Connecting => "Conectando",
            Status::Offline => "Sem pool",
            Status::Paused => "Pausado",
        }
    }
}

/// Info lines of the menu, in order (each one is a disabled item).
#[derive(Clone, Copy)]
#[repr(usize)]
enum Row {
    Status,
    Hashrate,
    BestDiff,
    Shares,
    Chance,
    Block,
    Difficulty,
    Price,
    Network,
    Queue,
    Mempool,
    NextBlock,
    FeeTitle,
    Fee0,
    Fee1,
    Fee2,
    Fee3,
    Recommended,
}
const ROWS: usize = Row::Recommended as usize + 1;

pub struct Tray {
    pub icon: TrayIcon,
    pub toggle_item: MenuItem,
    pause_item: MenuItem,
    pool_item: MenuItem,
    rows: Vec<MenuItem>,
    texts: Vec<String>,
    dev_items: Vec<(Device, CheckMenuItem)>,
    cpu_items: Vec<(u32, CheckMenuItem)>,
    gpu_items: Vec<(u32, CheckMenuItem)>,
    tx_items: Vec<(u32, CheckMenuItem)>,
    field_items: Vec<(&'static str, CheckMenuItem)>,
    last_status: Option<Status>,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    last_title: String,
}

/// RGBA icon: orange circle with a dark "chip".
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let mut v = vec![0u8; (size * size * 4) as usize];
    let c = (size as f32 - 1.0) / 2.0;
    let r = size as f32 / 2.0 - 0.5;
    for y in 0..size {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            let (dx, dy) = (x as f32 - c, y as f32 - c);
            let d = (dx * dx + dy * dy).sqrt();
            if d <= r {
                let chip = dx.abs() < size as f32 * 0.22 && dy.abs() < size as f32 * 0.22;
                let pins = (dx.abs() < size as f32 * 0.32 && dy.abs() < size as f32 * 0.22 && (y % 4 < 2))
                    || (dy.abs() < size as f32 * 0.32 && dx.abs() < size as f32 * 0.22 && (x % 4 < 2));
                let (cr, cg, cb) = if chip || pins { (30, 22, 40) } else { (247, 147, 26) };
                let a = ((r - d).clamp(0.0, 1.0) * 255.0) as u8;
                v[i..i + 4].copy_from_slice(&[cr, cg, cb, a]);
            }
        }
    }
    v
}

/// Tray icon with a status dot in the bottom-right corner.
fn status_icon(size: u32, color: [u8; 3]) -> Vec<u8> {
    let mut v = icon_rgba(size);
    let s = size as f32;
    let (cx, cy, r) = (s * 0.76, s * 0.76, s * 0.22);
    let ring = s * 0.07;
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            if d > r + ring + 1.0 {
                continue;
            }
            let i = ((y * size + x) * 4) as usize;
            let (rgb, edge) = if d <= r { (color, r - d) } else { ([20, 20, 24], r + ring - d) };
            let a = (edge + 0.5).clamp(0.0, 1.0);
            let prev = v[i + 3] as f32 / 255.0;
            for k in 0..3 {
                v[i + k] = (rgb[k] as f32 * a + v[i + k] as f32 * (1.0 - a)) as u8;
            }
            v[i + 3] = ((a + prev * (1.0 - a)) * 255.0) as u8;
        }
    }
    v
}

fn make_icon(status: Status) -> Option<Icon> {
    Icon::from_rgba(status_icon(32, status.color()), 32, 32).ok()
}

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Text block shown in the menu bar / tooltip for one field.
fn field_text(key: &str, s: &Shared, n: &crate::state::NetInfo, paused: bool) -> Option<String> {
    match key {
        "hashrate" => Some(if paused {
            "Pausado".into()
        } else {
            let (v, u) = fmt::hashrate(s.hashrate());
            format!("{v} {u}")
        }),
        "fee" => {
            let (c, _) = netinfo::congestion(n)?;
            let fee = netinfo::recommendation(c).0.fee(n)?;
            Some(format!("⛽{} {} sat/vB", c.dot(), fmt::feerate(fee)))
        }
        "last_block" => {
            let b = n.last_block.as_ref()?;
            let m = now_unix().saturating_sub(b.time) / 60;
            Some(format!("⛓ {m}m"))
        }
        "best_diff" => Some(format!("★ {}", fmt::diff(s.best_diff.get()))),
        "price" => {
            if let Some(brl) = n.btc_price_brl {
                Some(format!("R$ {}k", fmt::br(brl / 1000.0, 0)))
            } else {
                n.btc_price_usd.map(|u| format!("${}k", fmt::br(u / 1000.0, 1)))
            }
        }
        _ => None,
    }
}

fn money(sats: f64, n: &crate::state::NetInfo) -> String {
    let btc = sats / 1e8;
    let mut parts = Vec::new();
    if let Some(u) = n.btc_price_usd {
        parts.push(format!("US$ {}", fmt::br(btc * u, 2)));
    }
    if let Some(b) = n.btc_price_brl {
        parts.push(format!("R$ {}", fmt::br(btc * b, 2)));
    }
    if parts.is_empty() { String::new() } else { format!("≈ {}", parts.join(" · ")) }
}

fn bar(frac: f64, width: usize) -> String {
    let full = (frac.clamp(0.0, 1.0) * width as f64).round() as usize;
    format!("▕{}{}▏", "█".repeat(full), "░".repeat(width - full))
}

/// URL of the miner's stats page on known solo pools.
pub fn pool_url(cfg: &Config) -> Option<String> {
    let addr = cfg.btc_address.trim();
    if addr.is_empty() {
        return None;
    }
    let host = cfg.pool_host.to_lowercase();
    if host.contains("public-pool") {
        Some(format!("https://web.public-pool.io/#/app/{addr}"))
    } else if host.contains("ckpool") {
        Some(format!("https://solo.ckpool.org/users/{addr}"))
    } else {
        None
    }
}

/// Open a URL in the default browser (works while the window is hidden).
pub fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    let _ = r;
}

impl Tray {
    /// Must be called on the main thread after the event loop has started.
    pub fn new(on_cmd: impl Fn(TrayCmd) + Send + Sync + 'static) -> Result<Tray, String> {
        let e = |e: tray_icon::menu::Error| e.to_string();
        let menu = Menu::new();
        let rows: Vec<MenuItem> = (0..ROWS).map(|_| MenuItem::new("…", false, None)).collect();
        let sep = PredefinedMenuItem::separator;
        let groups: [&[Row]; 4] = [
            &[Row::Status],
            &[Row::Hashrate, Row::BestDiff, Row::Shares, Row::Chance],
            &[Row::Block, Row::Difficulty, Row::Price],
            &[
                Row::Network, Row::Queue, Row::Mempool, Row::NextBlock, Row::FeeTitle,
                Row::Fee0, Row::Fee1, Row::Fee2, Row::Fee3, Row::Recommended,
            ],
        ];
        for g in groups {
            for r in g {
                menu.append(&rows[*r as usize]).map_err(e)?;
            }
            menu.append(&sep()).map_err(e)?;
        }

        let check = |id: String, text: &str, on: bool| CheckMenuItem::with_id(id, text, true, on, None);
        let dev_items: Vec<(Device, CheckMenuItem)> = [(Device::Cpu, "cpu", "CPU"), (Device::Gpu, "gpu", "GPU"), (Device::Both, "both", "CPU + GPU")]
            .into_iter()
            .map(|(d, k, t)| (d, check(format!("dev:{k}"), t, false)))
            .collect();
        let cpu_items: Vec<_> = CPU_STEPS.iter().map(|&p| (p, check(format!("cpu:{p}"), &format!("{p}% por thread"), false))).collect();
        let gpu_items: Vec<_> = GPU_STEPS.iter().map(|&p| (p, check(format!("gpu:{p}"), &format!("{p}%"), false))).collect();
        let tx_items: Vec<_> = TX_SIZES.iter().map(|&(vb, t)| (vb, check(format!("tx:{vb}"), t, false))).collect();
        let field_items: Vec<_> = FIELDS.iter().map(|&(k, t)| (k, check(format!("field:{k}"), t, false))).collect();

        let sub = |title: &str, items: Vec<&CheckMenuItem>| -> Result<Submenu, String> {
            let s = Submenu::new(title, true);
            for i in items {
                s.append(i).map_err(e)?;
            }
            Ok(s)
        };
        menu.append(&sub("Dispositivo", dev_items.iter().map(|x| &x.1).collect())?).map_err(e)?;
        menu.append(&sub("Uso da CPU", cpu_items.iter().map(|x| &x.1).collect())?).map_err(e)?;
        menu.append(&sub("Uso da GPU", gpu_items.iter().map(|x| &x.1).collect())?).map_err(e)?;
        menu.append(&sub("Tamanho da transação", tx_items.iter().map(|x| &x.1).collect())?).map_err(e)?;
        // Only macOS shows text next to the tray icon.
        if cfg!(target_os = "macos") {
            menu.append(&sub("Barra de menus mostra", field_items.iter().map(|x| &x.1).collect())?).map_err(e)?;
        }
        menu.append(&sep()).map_err(e)?;

        let pause_item = MenuItem::with_id("pause", "Pausar mineração", true, None);
        let pool_item = MenuItem::with_id("pool", "Ver na pool…", true, None);
        let mempool_item = MenuItem::with_id("mempool", "Ver no mempool.space…", true, None);
        let toggle_item = MenuItem::with_id("toggle", "Ocultar janela", true, None);
        let quit_item = MenuItem::with_id("quit", "Sair", true, None);
        for i in [&pause_item, &pool_item, &mempool_item, &toggle_item] {
            menu.append(i).map_err(e)?;
        }
        menu.append(&sep()).map_err(e)?;
        menu.append(&quit_item).map_err(e)?;

        let icon = make_icon(Status::Connecting).ok_or("ícone inválido")?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("NerdMiner Emulator")
            .with_icon(icon)
            .build()
            .map_err(|e| e.to_string())?;

        let on_cmd = Arc::new(on_cmd);
        let h = on_cmd.clone();
        MenuEvent::set_event_handler(Some(move |ev: MenuEvent| {
            if let Some(cmd) = parse_cmd(ev.id.as_ref()) {
                h(cmd)
            }
        }));
        // Double-click on the icon (Windows/Linux) toggles the window.
        TrayIconEvent::set_event_handler(Some(move |ev: TrayIconEvent| {
            if let TrayIconEvent::DoubleClick { .. } = ev {
                on_cmd(TrayCmd::Toggle)
            }
        }));

        Ok(Tray {
            icon: tray,
            toggle_item,
            pause_item,
            pool_item,
            rows,
            texts: vec![String::new(); ROWS],
            dev_items,
            cpu_items,
            gpu_items,
            tx_items,
            field_items,
            last_status: None,
            last_title: String::new(),
        })
    }

    fn set_row(&mut self, r: Row, text: String) {
        let i = r as usize;
        if self.texts[i] != text {
            self.rows[i].set_text(&text);
            self.texts[i] = text;
        }
    }

    /// Refresh icon, title, tooltip and menu texts. Call about once per second.
    pub fn refresh(&mut self, s: &Shared) {
        let cfg = s.config.read().unwrap().clone();
        let n = s.net.lock().unwrap().clone();
        let paused = s.paused.load(Relaxed);
        let connected = s.pool_connected.load(Relaxed);
        let status = if paused {
            Status::Paused
        } else if connected {
            Status::Mining
        } else if cfg.btc_address.trim().is_empty() || s.pool_status.lock().unwrap().starts_with("Erro") {
            Status::Offline
        } else {
            Status::Connecting
        };
        if self.last_status != Some(status) {
            let _ = self.icon.set_icon(make_icon(status));
            self.last_status = Some(status);
        }

        // ---- menu bar title (macOS) and tooltip
        let parts: Vec<String> = cfg.tray_fields.iter().filter_map(|k| field_text(k, s, &n, paused)).collect();
        #[cfg(target_os = "macos")]
        {
            let title = if parts.is_empty() { String::new() } else { format!(" {}", parts.join(" · ")) };
            if title != self.last_title {
                self.icon.set_title(Some(&title));
                self.last_title = title;
            }
        }
        let hr = field_text("hashrate", s, &n, paused).unwrap_or_default();
        let mut tip = format!("NerdMiner — {}\n{hr} · best {}", status.label().to_lowercase(), fmt::diff(s.best_diff.get()));
        if let Some(b) = &n.last_block {
            tip += &format!("\nBloco #{} · há {}", b.height, fmt::duration(now_unix().saturating_sub(b.time) as f64));
        }
        if let (Some((c, _)), Some(fee)) = (netinfo::congestion(&n), netinfo::congestion(&n).and_then(|(c, _)| netinfo::recommendation(c).0.fee(&n))) {
            let cost = money(fee * cfg.tx_vbytes as f64, &n);
            tip += &format!("\nRede {} {} · enviar: {} sat/vB {}", c.dot(), c.label().to_lowercase(), fmt::feerate(fee), cost);
        }
        if cfg!(target_os = "windows") && tip.chars().count() > 127 {
            tip = tip.chars().take(127).collect(); // Windows tooltip limit
        }
        let _ = self.icon.set_tooltip(Some(tip.as_str()));

        // ---- mining
        let mut head = format!("● {} — {}", status.label(), cfg.pool_host);
        if let Some(t) = *s.connected_since.lock().unwrap() {
            head += &format!(" · {}", fmt::duration(t.elapsed().as_secs_f64()));
        }
        self.set_row(Row::Status, head);
        let mut devs = Vec::new();
        for (on, name, h) in [
            (s.cpu_enabled.load(Relaxed), "CPU", s.hashrate_cpu.get()),
            (s.gpu_enabled.load(Relaxed), "GPU", s.hashrate_gpu.get()),
        ] {
            if on {
                let (v, u) = fmt::hashrate(h);
                devs.push(format!("{name} {v} {u}"));
            }
        }
        let split = if devs.len() > 1 { format!("   ({})", devs.join(" · ")) } else if devs.is_empty() { "   (nenhum dispositivo)".into() } else { format!("   ({})", devs[0].split(' ').next().unwrap_or("")) };
        self.set_row(Row::Hashrate, format!("Hashrate   {hr}{split}"));
        self.set_row(Row::BestDiff, format!("Melhor dificuldade   {}", fmt::diff(s.best_diff.get())));
        self.set_row(
            Row::Shares,
            format!(
                "Shares   {} (32 bits) · aceitas {} · blocos {}",
                s.shares32.load(Relaxed),
                s.accepted.load(Relaxed),
                s.blocks.load(Relaxed)
            ),
        );
        let chance = match n.net_difficulty {
            Some(d) if s.hashrate() > 0.0 => {
                let per_block = d * 4_294_967_296.0;
                let p = -(-s.hashrate() * 86400.0 / per_block).exp_m1();
                format!("Chance hoje   {} · bloco a cada ~{}", fmt::odds(p), fmt::duration(per_block / s.hashrate()))
            }
            _ => "Chance hoje   —".into(),
        };
        self.set_row(Row::Chance, chance);

        // ---- network
        self.set_row(
            Row::Block,
            match &n.last_block {
                Some(b) => format!(
                    "Bloco   #{} · {} · há {}",
                    fmt::br(b.height as f64, 0),
                    b.pool,
                    fmt::duration(now_unix().saturating_sub(b.time) as f64)
                ),
                None => "Bloco   —".into(),
            },
        );
        self.set_row(
            Row::Difficulty,
            match &n.diff_adj {
                Some(a) => format!(
                    "Ajuste de dificuldade   {}% · {}{}% em ~{} ({} blocos)",
                    fmt::br(a.progress_pct, 0),
                    if a.change_pct >= 0.0 { "+" } else { "" },
                    fmt::br(a.change_pct, 2),
                    fmt::duration(a.remaining_secs),
                    fmt::br(a.remaining_blocks as f64, 0)
                ),
                None => "Ajuste de dificuldade   —".into(),
            },
        );
        let mut price = Vec::new();
        if let Some(u) = n.btc_price_usd {
            price.push(format!("US$ {}", fmt::br(u, 0)));
        }
        if let Some(b) = n.btc_price_brl {
            price.push(format!("R$ {}", fmt::br(b, 0)));
        }
        self.set_row(Row::Price, format!("BTC   {}", if price.is_empty() { "—".into() } else { price.join(" · ") }));

        // ---- occupancy and fees
        match (netinfo::congestion(&n), &n.mempool) {
            (Some((c, blocks)), Some(m)) => {
                self.set_row(Row::Network, format!("REDE BITCOIN   {} {}", c.dot(), c.label()));
                let clear = (blocks.ceil().max(1.0)) * 600.0;
                self.set_row(
                    Row::Queue,
                    format!(
                        "Fila pagante   {} {} {} · ~{} para limpar",
                        bar(netinfo::congestion_fraction(blocks), 16),
                        fmt::br(blocks, if blocks < 10.0 { 2 } else { 0 }),
                        if blocks <= 1.0 { "bloco" } else { "blocos" },
                        fmt::duration(clear)
                    ),
                );
                let cheap = m.vsize - m.backlog_vsize;
                let mut mp = format!("Mempool   {} tx · {} vMB", fmt::br_big(m.count as f64), fmt::br(m.vsize / 1e6, 1));
                if cheap > m.vsize * 0.5 {
                    mp += &format!(" (maioria abaixo de {} sat/vB)", fmt::feerate(n.fee_minimum.unwrap_or(1.0)));
                }
                self.set_row(Row::Mempool, mp);
                self.set_row(
                    Row::NextBlock,
                    match m.next_block {
                        Some((lo, med, hi, ntx)) => format!(
                            "Próximo bloco   {} – {} sat/vB · mediana {} · {} tx",
                            fmt::feerate(lo),
                            fmt::feerate(hi),
                            fmt::feerate(med),
                            fmt::br_big(ntx as f64)
                        ),
                        None => "Próximo bloco   —".into(),
                    },
                );
            }
            _ => {
                self.set_row(Row::Network, "REDE BITCOIN   carregando…".into());
                for r in [Row::Queue, Row::Mempool, Row::NextBlock] {
                    self.set_row(r, "—".into());
                }
            }
        }
        let size_label = TX_SIZES
            .iter()
            .find(|x| x.0 == cfg.tx_vbytes)
            .map(|x| x.1.to_string())
            .unwrap_or_else(|| format!("{} vB", cfg.tx_vbytes));
        self.set_row(Row::FeeTitle, format!("TAXA PARA ENVIAR AGORA   ({size_label})"));
        for (i, t) in Tier::ALL.iter().enumerate() {
            let row = [Row::Fee0, Row::Fee1, Row::Fee2, Row::Fee3][i];
            let text = match t.fee(&n) {
                Some(f) => {
                    let sats = (f * cfg.tx_vbytes as f64).round();
                    format!("{}   {} sat/vB · {} sats {}", t.label(), fmt::feerate(f), fmt::br(sats, 0), money(sats, &n))
                }
                None => format!("{}   —", t.label()),
            };
            self.set_row(row, text);
        }
        let rec = netinfo::congestion(&n).and_then(|(c, _)| {
            let (tier, why) = netinfo::recommendation(c);
            Some(format!("✅ Recomendado: {} sat/vB — {why}", fmt::feerate(tier.fee(&n)?)))
        });
        self.set_row(Row::Recommended, rec.unwrap_or_else(|| "✅ Recomendado: —".into()));

        // ---- controls
        self.pause_item.set_text(if paused { "Retomar mineração" } else { "Pausar mineração" });
        self.pool_item.set_enabled(pool_url(&cfg).is_some());
        let cpu_on = s.cpu_enabled.load(Relaxed);
        let gpu_on = s.gpu_enabled.load(Relaxed);
        let dev = match (cpu_on, gpu_on) {
            (true, true) => Some(Device::Both),
            (true, false) => Some(Device::Cpu),
            (false, true) => Some(Device::Gpu),
            _ => None,
        };
        for (d, it) in &self.dev_items {
            it.set_checked(Some(*d) == dev);
        }
        let cu = s.cpu_usage.load(Relaxed);
        for (p, it) in &self.cpu_items {
            it.set_checked(*p == cu);
            it.set_enabled(cpu_on);
        }
        let gu = s.gpu_usage.load(Relaxed);
        for (p, it) in &self.gpu_items {
            it.set_checked(*p == gu);
            it.set_enabled(gpu_on);
        }
        for (vb, it) in &self.tx_items {
            it.set_checked(*vb == cfg.tx_vbytes);
        }
        for (k, it) in &self.field_items {
            it.set_checked(cfg.tray_fields.iter().any(|f| f == k));
        }

        // Debug: NERDMINER_TRAY_DUMP=1 prints the menu to stderr (screenshots of the
        // menu bar are not always possible).
        if std::env::var_os("NERDMINER_TRAY_DUMP").is_some() {
            let title: Vec<String> = cfg.tray_fields.iter().filter_map(|k| field_text(k, s, &n, paused)).collect();
            eprintln!("=== [{status:?}] title: {}\n--- tooltip:\n{tip}\n--- menu:\n{}\n", title.join(" · "), self.texts.join("\n"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_ids_round_trip() {
        assert!(matches!(parse_cmd("cpu:25"), Some(TrayCmd::CpuUsage(25))));
        assert!(matches!(parse_cmd("dev:both"), Some(TrayCmd::Device(Device::Both))));
        assert!(matches!(parse_cmd("tx:154"), Some(TrayCmd::TxSize(154))));
        assert!(matches!(parse_cmd("field:fee"), Some(TrayCmd::Field("fee"))));
        assert!(parse_cmd("field:nope").is_none());
        assert!(parse_cmd("cpu:x").is_none());
    }

    #[test]
    fn pool_urls() {
        let mut c = Config { btc_address: "bc1qtest".into(), ..Config::default() };
        assert_eq!(pool_url(&c).unwrap(), "https://web.public-pool.io/#/app/bc1qtest");
        c.pool_host = "solo.ckpool.org".into();
        assert_eq!(pool_url(&c).unwrap(), "https://solo.ckpool.org/users/bc1qtest");
        c.pool_host = "my.pool".into();
        assert!(pool_url(&c).is_none());
    }
}
