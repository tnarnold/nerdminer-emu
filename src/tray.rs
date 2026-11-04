//! System tray icon (macOS menu bar, Windows notification area, Linux SNI/ksni).
//!
//! The icon carries a status dot (mining / connecting / offline / paused); on macOS the
//! menu bar also shows the fields picked by the user. The menu is a native, text-only
//! dashboard: mining stats, network occupancy with the fee to use right now, and quick
//! controls (device, CPU/GPU usage, transaction size, currency, language, menu bar fields).
//!
//! All texts come from `i18n`; when the language changes the menu is rebuilt.

use crate::config::Config;
use crate::fmt;
use crate::i18n::{self, Lang, fill, t};
use crate::netinfo::{self, Tier};
use crate::state::{PoolStatus, Shared};
use std::sync::Arc;
use std::sync::atomic::Ordering::*;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub const CPU_STEPS: [u32; 5] = [10, 25, 50, 75, 100];
pub const GPU_STEPS: [u32; 5] = [10, 30, 50, 75, 100];
/// vB of a 1-in/2-out P2WPKH, a 1-in/2-out P2TR and a 5→1 P2WPKH consolidation
/// (labels in `i18n::Tr::tx_labels`).
pub const TX_SIZES: [u32; 3] = [141, 154, 382];
/// Menu bar fields (labels in `i18n::Tr::field_labels`).
pub const FIELDS: [&str; 5] = ["hashrate", "fee", "last_block", "best_diff", "price"];

/// macOS re-lays out the status item (and closes an open tooltip) whenever the title or
/// tooltip is set, so these are refreshed sparingly.
const TITLE_EVERY: Duration = Duration::from_secs(5);
const TOOLTIP_EVERY: Duration = Duration::from_secs(60);

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
    Currency(&'static str),
    /// "auto" or a `Lang` code.
    Language(&'static str),
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
        "cur" => TrayCmd::Currency(crate::currency::CURRENCIES.iter().find(|c| c.0 == arg)?.0),
        "lang" if arg == "auto" => TrayCmd::Language("auto"),
        "lang" => TrayCmd::Language(Lang::ALL.into_iter().find(|l| l.code() == arg)?.code()),
        "field" => TrayCmd::Field(FIELDS.iter().find(|k| **k == arg)?),
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
        let t = t();
        match self {
            Status::Mining => t.st_mining,
            Status::Connecting => t.st_connecting,
            Status::Offline => t.st_offline,
            Status::Paused => t.st_paused,
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

/// Menu items that are updated after the menu is built (rebuilt on language change).
struct Parts {
    toggle_item: MenuItem,
    pause_item: MenuItem,
    pool_item: MenuItem,
    rows: Vec<MenuItem>,
    dev_items: Vec<(Device, CheckMenuItem)>,
    cpu_items: Vec<(u32, CheckMenuItem)>,
    gpu_items: Vec<(u32, CheckMenuItem)>,
    tx_items: Vec<(u32, CheckMenuItem)>,
    cur_items: Vec<(&'static str, CheckMenuItem)>,
    lang_items: Vec<(&'static str, CheckMenuItem)>,
    field_items: Vec<(&'static str, CheckMenuItem)>,
}

pub struct Tray {
    icon: TrayIcon,
    parts: Parts,
    texts: Vec<String>,
    lang: Lang,
    window_visible: bool,
    last_status: Option<Status>,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    last_title: (String, Option<Instant>),
    last_tip: (String, Option<Instant>),
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
            t().st_paused.into()
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
            let cur = &s.config.read().unwrap().currency;
            n.price(cur).map(|p| crate::currency::price_short(p, cur))
        }
        _ => None,
    }
}

/// "≈ R$ 1,21" in the chosen currency (empty while the price is unknown).
fn money(sats: f64, n: &crate::state::NetInfo, cur: &str) -> String {
    n.price(cur).map(|p| format!("≈ {}", crate::currency::amount(sats / 1e8 * p, cur))).unwrap_or_default()
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

fn build_menu() -> Result<(Menu, Parts), String> {
    let t = t();
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

    let check = |id: String, text: &str| CheckMenuItem::with_id(id, text, true, false, None);
    let dev_items: Vec<_> = [(Device::Cpu, "cpu", "CPU"), (Device::Gpu, "gpu", "GPU"), (Device::Both, "both", "CPU + GPU")]
        .into_iter()
        .map(|(d, k, txt)| (d, check(format!("dev:{k}"), txt)))
        .collect();
    let cpu_items: Vec<_> = CPU_STEPS.iter().map(|&p| (p, check(format!("cpu:{p}"), &fill(t.cpu_step, &[&p])))).collect();
    let gpu_items: Vec<_> = GPU_STEPS.iter().map(|&p| (p, check(format!("gpu:{p}"), &format!("{p}%")))).collect();
    let tx_items: Vec<_> = TX_SIZES.iter().zip(t.tx_labels).map(|(&vb, l)| (vb, check(format!("tx:{vb}"), l))).collect();
    let cur_items: Vec<_> = crate::currency::CURRENCIES
        .iter()
        .map(|&(c, sym)| (c, check(format!("cur:{c}"), &format!("{} ({sym})", crate::currency::name(c)))))
        .collect();
    let mut lang_items = vec![("auto", check("lang:auto".into(), t.language_auto))];
    lang_items.extend(Lang::ALL.into_iter().map(|l| (l.code(), check(format!("lang:{}", l.code()), l.native_name()))));
    let field_items: Vec<_> = FIELDS.iter().zip(t.field_labels).map(|(&k, l)| (k, check(format!("field:{k}"), l))).collect();

    let sub = |title: &str, items: Vec<&CheckMenuItem>| -> Result<Submenu, String> {
        let s = Submenu::new(title, true);
        for i in items {
            s.append(i).map_err(e)?;
        }
        Ok(s)
    };
    menu.append(&sub(t.sub_device, dev_items.iter().map(|x| &x.1).collect())?).map_err(e)?;
    menu.append(&sub(t.sub_cpu, cpu_items.iter().map(|x| &x.1).collect())?).map_err(e)?;
    menu.append(&sub(t.sub_gpu, gpu_items.iter().map(|x| &x.1).collect())?).map_err(e)?;
    menu.append(&sub(t.sub_tx, tx_items.iter().map(|x| &x.1).collect())?).map_err(e)?;
    menu.append(&sub(t.sub_currency, cur_items.iter().map(|x| &x.1).collect())?).map_err(e)?;
    menu.append(&sub(t.sub_language, lang_items.iter().map(|x| &x.1).collect())?).map_err(e)?;
    // Only macOS shows text next to the tray icon.
    if cfg!(target_os = "macos") {
        menu.append(&sub(t.sub_fields, field_items.iter().map(|x| &x.1).collect())?).map_err(e)?;
    }
    menu.append(&sep()).map_err(e)?;

    let pause_item = MenuItem::with_id("pause", t.pause_mining, true, None);
    let pool_item = MenuItem::with_id("pool", t.open_pool, true, None);
    let mempool_item = MenuItem::with_id("mempool", t.open_mempool, true, None);
    let toggle_item = MenuItem::with_id("toggle", t.hide_window, true, None);
    let quit_item = MenuItem::with_id("quit", t.quit, true, None);
    for i in [&pause_item, &pool_item, &mempool_item, &toggle_item] {
        menu.append(i).map_err(e)?;
    }
    menu.append(&sep()).map_err(e)?;
    menu.append(&quit_item).map_err(e)?;

    Ok((
        menu,
        Parts { toggle_item, pause_item, pool_item, rows, dev_items, cpu_items, gpu_items, tx_items, cur_items, lang_items, field_items },
    ))
}

/// True when `last` was never set or is older than `every`.
fn due(last: Option<Instant>, every: Duration) -> bool {
    last.is_none_or(|t| t.elapsed() >= every)
}

impl Tray {
    /// Must be called on the main thread after the event loop has started.
    pub fn new(on_cmd: impl Fn(TrayCmd) + Send + Sync + 'static) -> Result<Tray, String> {
        let (menu, parts) = build_menu()?;
        let icon = make_icon(Status::Connecting).ok_or("invalid icon")?;
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
            parts,
            texts: vec![String::new(); ROWS],
            lang: i18n::lang(),
            window_visible: true,
            last_status: None,
            last_title: (String::new(), None),
            last_tip: (String::new(), None),
        })
    }

    pub fn set_window_visible(&mut self, v: bool) {
        self.window_visible = v;
        self.parts.toggle_item.set_text(if v { t().hide_window } else { t().show_window });
    }

    fn set_row(&mut self, r: Row, text: String) {
        let i = r as usize;
        if self.texts[i] != text {
            self.parts.rows[i].set_text(&text);
            self.texts[i] = text;
        }
    }

    /// Refresh icon, title, tooltip and menu texts. Call about once per second.
    pub fn refresh(&mut self, s: &Shared) {
        // ---- language change: rebuild the whole menu
        if i18n::lang() != self.lang {
            if let Ok((menu, parts)) = build_menu() {
                self.icon.set_menu(Some(Box::new(menu)));
                self.parts = parts;
                self.texts = vec![String::new(); ROWS];
                self.lang = i18n::lang();
                self.last_title.1 = None;
                self.last_tip.1 = None;
                let v = self.window_visible;
                self.set_window_visible(v);
            }
        }
        let t = t();
        let cfg = s.config.read().unwrap().clone();
        let n = s.net.lock().unwrap().clone();
        let pool = s.pool_status.lock().unwrap().clone();
        let paused = s.paused.load(Relaxed);
        let status = if paused {
            Status::Paused
        } else if s.pool_connected.load(Relaxed) {
            Status::Mining
        } else if matches!(pool, PoolStatus::NoAddress | PoolStatus::Error(_)) {
            Status::Offline
        } else {
            Status::Connecting
        };
        let status_changed = self.last_status != Some(status);
        if status_changed {
            let _ = self.icon.set_icon(make_icon(status));
            self.last_status = Some(status);
        }

        // ---- menu bar title (macOS): at most every TITLE_EVERY unless the status changed
        let parts: Vec<String> = cfg.tray_fields.iter().filter_map(|k| field_text(k, s, &n, paused)).collect();
        #[cfg(target_os = "macos")]
        {
            let title = if parts.is_empty() { String::new() } else { format!(" {}", parts.join(" · ")) };
            if title != self.last_title.0 && (status_changed || due(self.last_title.1, TITLE_EVERY)) {
                self.icon.set_title(Some(&title));
                self.last_title = (title, Some(Instant::now()));
            }
        }

        // ---- tooltip: at most every TOOLTIP_EVERY unless the status changed
        let hr = field_text("hashrate", s, &n, paused).unwrap_or_default();
        let mut tip = format!("NerdMiner — {}\n{hr} · best {}", status.label().to_lowercase(), fmt::diff(s.best_diff.get()));
        if let Some(b) = &n.last_block {
            tip += "\n";
            tip += &fill(t.tip_block, &[&b.height, &fmt::duration(now_unix().saturating_sub(b.time) as f64)]);
        }
        if let Some((c, _)) = netinfo::congestion(&n) {
            if let Some(fee) = netinfo::recommendation(c).0.fee(&n) {
                let cost = money(fee * cfg.tx_vbytes as f64, &n, &cfg.currency);
                tip += "\n";
                tip += &fill(t.tip_network, &[&c.dot(), &c.label().to_lowercase(), &fmt::feerate(fee), &cost]);
            }
        }
        if cfg!(target_os = "windows") && tip.chars().count() > 127 {
            tip = tip.chars().take(127).collect(); // Windows tooltip limit
        }
        if tip != self.last_tip.0 && (status_changed || due(self.last_tip.1, TOOLTIP_EVERY)) {
            let _ = self.icon.set_tooltip(Some(tip.as_str()));
            self.last_tip = (tip.clone(), Some(Instant::now()));
        }

        // ---- mining
        let mut head = format!("● {} — {}", status.label(), cfg.pool_host);
        match (&status, &pool) {
            (Status::Offline, PoolStatus::NoAddress) => head = format!("● {} — {}", status.label(), t.set_address),
            (Status::Offline, PoolStatus::Error(e)) => {
                let e: String = e.chars().take(70).collect();
                head = format!("● {} — {}", status.label(), e);
            }
            _ => {}
        }
        if let Some(since) = *s.connected_since.lock().unwrap() {
            head += &format!(" · {}", fmt::duration(since.elapsed().as_secs_f64()));
        }
        self.set_row(Row::Status, head);
        let mut devs = Vec::new();
        for (on, name, h) in [
            (s.cpu_enabled.load(Relaxed), "CPU", s.hashrate_cpu.get()),
            (s.gpu_enabled.load(Relaxed), "GPU", s.hashrate_gpu.get()),
        ] {
            if on {
                let (v, u) = fmt::hashrate(h);
                devs.push((name, format!("{name} {v} {u}")));
            }
        }
        let split = match devs.len() {
            0 => format!("   ({})", t.r_no_device),
            1 => format!("   ({})", devs[0].0),
            _ => format!("   ({})", devs.iter().map(|d| d.1.as_str()).collect::<Vec<_>>().join(" · ")),
        };
        self.set_row(Row::Hashrate, format!("Hashrate   {hr}{split}"));
        self.set_row(Row::BestDiff, format!("{}   {}", t.r_best, fmt::diff(s.best_diff.get())));
        self.set_row(
            Row::Shares,
            fill(t.r_shares, &[&s.shares32.load(Relaxed), &s.accepted.load(Relaxed), &s.blocks.load(Relaxed)]),
        );
        let chance = match n.net_difficulty {
            Some(d) if s.hashrate() > 0.0 => {
                let per_block = d * 4_294_967_296.0;
                let p = -(-s.hashrate() * 86400.0 / per_block).exp_m1();
                let every = fill(t.r_block_every, &[&fmt::duration(per_block / s.hashrate())]);
                format!("{}   {} · {every}", t.r_chance, fmt::odds(p))
            }
            _ => format!("{}   —", t.r_chance),
        };
        self.set_row(Row::Chance, chance);

        // ---- network
        self.set_row(
            Row::Block,
            match &n.last_block {
                Some(b) => {
                    let ago = fill(t.ago, &[&fmt::duration(now_unix().saturating_sub(b.time) as f64)]);
                    format!("{}   #{} · {} · {ago}", t.r_block, fmt::num(b.height as f64, 0), b.pool)
                }
                None => format!("{}   —", t.r_block),
            },
        );
        self.set_row(
            Row::Difficulty,
            match &n.diff_adj {
                Some(a) => fill(
                    t.r_diff_adj,
                    &[
                        &fmt::num(a.progress_pct, 0),
                        &format!("{}{}", if a.change_pct >= 0.0 { "+" } else { "" }, fmt::num(a.change_pct, 2)),
                        &fmt::duration(a.remaining_secs),
                        &fmt::num(a.remaining_blocks as f64, 0),
                    ],
                ),
                None => format!("{}   —", t.r_diff_adj.split("   ").next().unwrap_or("")),
            },
        );
        let price = n.price(&cfg.currency).map(|p| crate::currency::price(p, &cfg.currency));
        self.set_row(Row::Price, format!("BTC   {}", price.unwrap_or_else(|| "—".into())));

        // ---- occupancy and fees
        match (netinfo::congestion(&n), &n.mempool) {
            (Some((c, blocks)), Some(m)) => {
                self.set_row(Row::Network, format!("{}   {} {}", t.r_network, c.dot(), c.label()));
                let clear = (blocks.ceil().max(1.0)) * 600.0;
                self.set_row(
                    Row::Queue,
                    format!(
                        "{}   {} {} {} · {}",
                        t.r_queue,
                        bar(netinfo::congestion_fraction(blocks), 16),
                        fmt::num(blocks, if blocks < 10.0 { 2 } else { 0 }),
                        if blocks <= 1.0 { t.block_one } else { t.block_many },
                        fill(t.to_clear, &[&fmt::duration(clear)])
                    ),
                );
                let cheap = m.vsize - m.backlog_vsize;
                let mut mp = format!("Mempool   {} tx · {} vMB", fmt::big(m.count as f64), fmt::num(m.vsize / 1e6, 1));
                if cheap > m.vsize * 0.5 {
                    mp += " ";
                    mp += &fill(t.mostly_below, &[&fmt::feerate(n.fee_minimum.unwrap_or(1.0))]);
                }
                self.set_row(Row::Mempool, mp);
                self.set_row(
                    Row::NextBlock,
                    match m.next_block {
                        Some((lo, med, hi, ntx)) => format!(
                            "{}   {} – {} sat/vB · {} {} · {} tx",
                            t.r_next_block,
                            fmt::feerate(lo),
                            fmt::feerate(hi),
                            t.median,
                            fmt::feerate(med),
                            fmt::big(ntx as f64)
                        ),
                        None => format!("{}   —", t.r_next_block),
                    },
                );
            }
            _ => {
                self.set_row(Row::Network, format!("{}   {}", t.r_network, t.loading));
                for r in [Row::Queue, Row::Mempool, Row::NextBlock] {
                    self.set_row(r, "—".into());
                }
            }
        }
        let size_label = TX_SIZES
            .iter()
            .position(|&vb| vb == cfg.tx_vbytes)
            .map(|i| t.tx_labels[i].to_string())
            .unwrap_or_else(|| format!("{} vB", cfg.tx_vbytes));
        self.set_row(Row::FeeTitle, format!("{}   ({size_label})", t.fee_title));
        for (i, tier) in Tier::ALL.iter().enumerate() {
            let row = [Row::Fee0, Row::Fee1, Row::Fee2, Row::Fee3][i];
            let text = match tier.fee(&n) {
                Some(f) => {
                    let sats = (f * cfg.tx_vbytes as f64).round();
                    format!(
                        "{}   {} sat/vB · {} sats {}",
                        tier.label(),
                        fmt::feerate(f),
                        fmt::num(sats, 0),
                        money(sats, &n, &cfg.currency)
                    )
                }
                None => format!("{}   —", tier.label()),
            };
            self.set_row(row, text);
        }
        let rec = netinfo::congestion(&n).and_then(|(c, _)| {
            let (tier, why) = netinfo::recommendation(c);
            Some(format!("{}: {} sat/vB — {why}", t.recommended, fmt::feerate(tier.fee(&n)?)))
        });
        self.set_row(Row::Recommended, rec.unwrap_or_else(|| format!("{}: —", t.recommended)));

        // ---- controls
        let p = &self.parts;
        p.pause_item.set_text(if paused { t.resume_mining } else { t.pause_mining });
        p.pool_item.set_enabled(pool_url(&cfg).is_some());
        let cpu_on = s.cpu_enabled.load(Relaxed);
        let gpu_on = s.gpu_enabled.load(Relaxed);
        let dev = match (cpu_on, gpu_on) {
            (true, true) => Some(Device::Both),
            (true, false) => Some(Device::Cpu),
            (false, true) => Some(Device::Gpu),
            _ => None,
        };
        for (d, it) in &p.dev_items {
            it.set_checked(Some(*d) == dev);
        }
        let cu = s.cpu_usage.load(Relaxed);
        for (v, it) in &p.cpu_items {
            it.set_checked(*v == cu);
            it.set_enabled(cpu_on);
        }
        let gu = s.gpu_usage.load(Relaxed);
        for (v, it) in &p.gpu_items {
            it.set_checked(*v == gu);
            it.set_enabled(gpu_on);
        }
        for (vb, it) in &p.tx_items {
            it.set_checked(*vb == cfg.tx_vbytes);
        }
        for (k, it) in &p.field_items {
            it.set_checked(cfg.tray_fields.iter().any(|f| f == k));
        }
        let cur = crate::currency::code(&cfg.currency);
        for (c, it) in &p.cur_items {
            it.set_checked(*c == cur);
        }
        let auto = cfg.language.trim().eq_ignore_ascii_case("auto");
        for (code, it) in &p.lang_items {
            it.set_checked(if *code == "auto" { auto } else { !auto && i18n::resolve(&cfg.language).code() == *code });
        }

        // Debug: NERDMINER_TRAY_DUMP=1 prints the menu to stderr (screenshots of the
        // menu bar are not always possible).
        if std::env::var_os("NERDMINER_TRAY_DUMP").is_some() {
            eprintln!(
                "=== [{status:?}] title: {}\n--- tooltip:\n{tip}\n--- menu:\n{}\n",
                parts.join(" · "),
                self.texts.join("\n")
            );
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
        assert!(matches!(parse_cmd("cur:EUR"), Some(TrayCmd::Currency("EUR"))));
        assert!(matches!(parse_cmd("lang:en"), Some(TrayCmd::Language("en"))));
        assert!(matches!(parse_cmd("lang:auto"), Some(TrayCmd::Language("auto"))));
        assert!(parse_cmd("lang:xx").is_none());
        assert!(parse_cmd("cur:XYZ").is_none());
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
