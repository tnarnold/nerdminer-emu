//! NerdMiner v2 T-Display (320x170) screens, rendered with the original
//! backgrounds, fonts and text coordinates from `tDisplayDriver.cpp`
//! (BitMaker-hub/NerdMiner_v2, MIT License).

use crate::fmt;
use crate::state::{NetInfo, Shared};
use eframe::egui::{
    self, Align2, Color32, ColorImage, FontData, FontDefinitions, FontFamily, FontId, Painter, Pos2,
    Rect, TextureHandle, TextureOptions, Vec2,
};
use std::sync::Arc;
use std::sync::atomic::Ordering::*;

/// TFT_eSPI RGB565 colours used by the original firmware.
const TFT_BLACK: Color32 = Color32::BLACK;
const TFT_WHITE: Color32 = Color32::WHITE;
const C_DEDB: Color32 = Color32::from_rgb(222, 219, 222);
const C_9C92: Color32 = Color32::from_rgb(156, 146, 148);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Miner,
    Clock,
    GlobalHash,
    BtcPrice,
}

impl Screen {
    pub const ALL: [Screen; 4] = [Screen::Miner, Screen::Clock, Screen::GlobalHash, Screen::BtcPrice];
}

pub struct Screens {
    miner: TextureHandle,
    clock: TextureHandle,
    global: TextureHandle,
    price: TextureHandle,
    pub init: TextureHandle,
}

/// Pixel box (inclusive) of the "KH/s" label baked into a background.
type LabelBox = (usize, usize, usize, usize);
const MINER_KHS: LabelBox = (135, 146, 160, 153);
const SMALL_KHS: LabelBox = (102, 150, 128, 157);

fn load(ctx: &egui::Context, name: &str, raw: &[u8], erase: Option<LabelBox>) -> TextureHandle {
    let mut rgb: Vec<u8> = raw
        .chunks_exact(2)
        .flat_map(|c| {
            let v = u16::from_le_bytes([c[0], c[1]]);
            let r = ((v >> 11) & 31) as u32 * 255 / 31;
            let g = ((v >> 5) & 63) as u32 * 255 / 63;
            let b = (v & 31) as u32 * 255 / 31;
            [r as u8, g as u8, b as u8]
        })
        .collect();
    // Erase the fixed "KH/s" so the unit can follow the actual hashrate.
    if let Some((x0, y0, x1, y1)) = erase {
        for y in y0 - 1..=y1 + 1 {
            let src = (y * 320 + x0 - 3) * 3;
            let px = [rgb[src], rgb[src + 1], rgb[src + 2]];
            for x in x0 - 2..=x1 + 2 {
                let i = (y * 320 + x) * 3;
                rgb[i..i + 3].copy_from_slice(&px);
            }
        }
    }
    let img = ColorImage::from_rgb([320, 170], &rgb);
    ctx.load_texture(name, img, TextureOptions::NEAREST)
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "digital".into(),
        Arc::new(FontData::from_static(include_bytes!("../assets/fonts/DigitalNumbers.ttf"))),
    );
    fonts.font_data.insert(
        "nerdsans".into(),
        Arc::new(FontData::from_static(include_bytes!("../assets/fonts/NotoSans_Bold.ttf"))),
    );
    let fallback = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut digital = vec!["digital".to_string()];
    digital.extend(fallback.clone());
    let mut sans = vec!["nerdsans".to_string()];
    sans.extend(fallback);
    fonts.families.insert(FontFamily::Name("digital".into()), digital);
    fonts.families.insert(FontFamily::Name("nerdsans".into()), sans);
    ctx.set_fonts(fonts);
}

impl Screens {
    pub fn new(ctx: &egui::Context) -> Self {
        Screens {
            miner: load(ctx, "miner", include_bytes!("../assets/screens/miner.rgb565"), Some(MINER_KHS)),
            clock: load(ctx, "clock", include_bytes!("../assets/screens/clock.rgb565"), Some(SMALL_KHS)),
            global: load(ctx, "global", include_bytes!("../assets/screens/global.rgb565"), None),
            price: load(ctx, "price", include_bytes!("../assets/screens/price.rgb565"), Some(SMALL_KHS)),
            init: load(ctx, "init", include_bytes!("../assets/screens/init.rgb565"), None),
        }
    }
}

/// Maps 320x170 device pixels onto the on-screen rect.
pub struct Lcd<'a> {
    pub p: &'a Painter,
    pub rect: Rect,
    pub s: f32,
}

impl Lcd<'_> {
    fn pos(&self, x: f32, y: f32) -> Pos2 {
        self.rect.min + Vec2::new(x, y) * self.s
    }
    fn bg(&self, tex: &TextureHandle) {
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        self.p.image(tex.id(), self.rect, uv, Color32::WHITE);
    }
    /// OpenFontRender with the DigitalNumbers font. `size` is the font size in device px.
    fn digital(&self, text: &str, x: f32, y: f32, size: f32, color: Color32, anchor: Align2) {
        let font = FontId::new(size * self.s, FontFamily::Name("digital".into()));
        self.p.text(self.pos(x, y), anchor, text, font, color);
    }
    /// TFT_eSPI FreeSans fonts (approximated by the bundled Noto Sans Bold).
    fn sans(&self, text: &str, x: f32, y: f32, size: f32, color: Color32, anchor: Align2) {
        let font = FontId::new(size * self.s, FontFamily::Name("nerdsans".into()));
        self.p.text(self.pos(x, y), anchor, text, font, color);
    }
    /// Like `sans`, shrinking the font so the text is at most `max_w` device px wide
    /// (other currencies have longer prices than the firmware's "$12345").
    fn sans_fit(&self, text: &str, x: f32, y: f32, size: f32, max_w: f32, color: Color32, anchor: Align2) {
        let font = |sz: f32| FontId::new(sz * self.s, FontFamily::Name("nerdsans".into()));
        let w = self.p.layout_no_wrap(text.to_string(), font(size), color).size().x / self.s;
        let fit = if w > max_w { size * max_w / w } else { size };
        // Top-anchored text: move down so the smaller text stays vertically centered.
        let dy = if anchor.y() == egui::Align::Min { (size - fit) / 2.0 } else { 0.0 };
        self.p.text(self.pos(x, y + dy), anchor, text, font(fit), color);
    }
    fn fill(&self, x: f32, y: f32, w: f32, h: f32, color: Color32) {
        let r = Rect::from_min_size(self.pos(x, y), Vec2::new(w, h) * self.s);
        self.p.rect_filled(r.intersect(self.rect), 0.0, color);
    }
    fn unit(&self, unit: &str, b: LabelBox) {
        let (x0, y0, x1, y1) = b;
        let cx = (x0 + x1) as f32 / 2.0 + 0.5;
        let cy = (y0 + y1) as f32 / 2.0 + 0.5;
        self.sans(unit, cx, cy, 11.5, TFT_BLACK, Align2::CENTER_CENTER);
    }
}

/// Hashrate in the firmware style: a number plus the unit drawn next to it.
fn hashrate_parts(h: f64) -> (String, &'static str) {
    let k = h / 1000.0;
    if k < 1000.0 {
        (format!("{k:.2}"), "KH/s")
    } else if k < 1e6 {
        (format!("{:.2}", k / 1e3), "MH/s")
    } else {
        (format!("{:.2}", k / 1e6), "GH/s")
    }
}

fn now_hm() -> String {
    chrono::Local::now().format("%H:%M").to_string()
}

fn price(net: &NetInfo, currency: &str) -> String {
    net.price(currency)
        .map(|p| crate::currency::price_screen(p, currency))
        .unwrap_or_else(|| format!("{}--", crate::currency::screen_symbol(currency)))
}

fn height(net: &NetInfo) -> String {
    net.block_height.map(|h| h.to_string()).unwrap_or_else(|| "--".into())
}

impl Screens {
    pub fn draw(&self, lcd: &Lcd, screen: Screen, s: &Shared) {
        let net = s.net.lock().unwrap().clone();
        let cur = s.config.read().unwrap().currency.clone();
        match screen {
            Screen::Miner => self.miner(lcd, s),
            Screen::Clock => self.clock(lcd, s, &net, &cur),
            Screen::GlobalHash => self.global(lcd, &net, &cur),
            Screen::BtcPrice => self.price(lcd, s, &net, &cur),
        }
    }

    fn miner(&self, l: &Lcd, s: &Shared) {
        l.bg(&self.miner);
        let (hr, unit) = hashrate_parts(s.hashrate());
        l.digital(&hr, 118.0, 114.0, 35.0, TFT_BLACK, Align2::RIGHT_TOP);
        l.unit(unit, MINER_KHS);
        let mhashes = s.total_hashes() as f64 / 1e6;
        let mh = if mhashes < 1e5 { format!("{mhashes:.0}") } else { fmt::si(mhashes) };
        l.digital(&mh, 268.0, 138.0, 18.0, TFT_BLACK, Align2::RIGHT_TOP);
        l.digital(&s.templates.load(Relaxed).to_string(), 186.0, 20.0, 18.0, C_DEDB, Align2::LEFT_TOP);
        l.digital(&best_diff(s.best_diff.get()), 186.0, 48.0, 18.0, C_DEDB, Align2::LEFT_TOP);
        l.digital(&s.shares32.load(Relaxed).to_string(), 186.0, 76.0, 18.0, C_DEDB, Align2::LEFT_TOP);
        let up = s.started.elapsed().as_secs();
        let t = format!("{}  {:02}:{:02}:{:02}", up / 86400, (up % 86400) / 3600, (up % 3600) / 60, up % 60);
        l.digital(&t, 315.0, 104.0, 14.0, C_DEDB, Align2::RIGHT_TOP);
        l.digital(&s.blocks.load(Relaxed).to_string(), 285.0, 56.0, 24.0, C_DEDB, Align2::LEFT_TOP);
        l.digital(&now_hm(), 286.0, 1.0, 10.0, TFT_BLACK, Align2::RIGHT_TOP);
        // pool connection indicator next to the wifi icon
        let ok = s.pool_connected.load(Relaxed);
        let c = if ok { Color32::from_rgb(40, 200, 90) } else { Color32::from_rgb(220, 50, 50) };
        l.p.circle_filled(l.pos(296.0, 6.0), 2.2 * l.s, c);
        if s.paused.load(Relaxed) {
            l.sans("PAUSADO", 64.0, 124.0, 11.0, TFT_BLACK, Align2::CENTER_CENTER);
        }
    }

    fn clock(&self, l: &Lcd, s: &Shared, net: &NetInfo, cur: &str) {
        l.bg(&self.clock);
        let (hr, unit) = hashrate_parts(s.hashrate());
        l.digital(&hr, 94.0, 129.0, 25.0, TFT_BLACK, Align2::RIGHT_TOP);
        l.unit(unit, SMALL_KHS);
        l.sans_fit(&price(net, cur), 202.0, 3.0, 13.0, 82.0, TFT_BLACK, Align2::LEFT_TOP);
        l.digital(&height(net), 254.0, 140.0, 18.0, TFT_BLACK, Align2::RIGHT_TOP);
        // FreeSansBold18pt at text size 2
        l.sans(&now_hm(), 130.0, 50.0, 50.0, C_DEDB, Align2::LEFT_TOP);
    }

    fn global(&self, l: &Lcd, net: &NetInfo, cur: &str) {
        l.bg(&self.global);
        l.sans_fit(&price(net, cur), 198.0, 3.0, 13.0, 62.0, TFT_BLACK, Align2::LEFT_TOP);
        l.sans(&now_hm(), 268.0, 3.0, 13.0, TFT_BLACK, Align2::LEFT_TOP);
        let fee = net.fee_half_hour.map(|f| format!("{f:.0} sat/vB")).unwrap_or_else(|| "-- sat/vB".into());
        l.sans(&fee, 302.0, 52.0, 13.0, C_9C92, Align2::RIGHT_TOP);
        let diff = net.net_difficulty.map(diff_short).unwrap_or_else(|| "--".into());
        l.sans(&diff, 302.0, 88.0, 13.0, C_9C92, Align2::RIGHT_TOP);
        let ghr = net.net_hashrate.map(|h| format!("{:.2}", h / 1e18)).unwrap_or_else(|| "--".into());
        l.digital(&ghr, 274.0, 145.0, 17.0, TFT_BLACK, Align2::RIGHT_TOP);
        l.digital(&height(net), 140.0, 104.0, 28.0, C_DEDB, Align2::RIGHT_TOP);

        const HALVING_BLOCKS: u64 = 210_000;
        let (pct, remaining) = match net.block_height {
            Some(h) => {
                let rem = HALVING_BLOCKS - h % HALVING_BLOCKS;
                ((HALVING_BLOCKS - rem) * 100 / HALVING_BLOCKS, rem.to_string())
            }
            None => (0, "--".into()),
        };
        let x2 = 2.0 + 138.0 * pct as f32 / 100.0;
        l.fill(2.0, 149.0, x2, 168.0, C_DEDB);
        l.sans(&format!("{remaining} BLOCKS"), 72.0, 159.0, 11.0, TFT_BLACK, Align2::CENTER_CENTER);
    }

    fn price(&self, l: &Lcd, s: &Shared, net: &NetInfo, cur: &str) {
        l.bg(&self.price);
        let (hr, unit) = hashrate_parts(s.hashrate());
        l.digital(&hr, 94.0, 129.0, 25.0, TFT_BLACK, Align2::RIGHT_TOP);
        l.unit(unit, SMALL_KHS);
        l.digital(&height(net), 254.0, 138.0, 18.0, TFT_WHITE, Align2::RIGHT_TOP);
        l.sans(&now_hm(), 222.0, 3.0, 13.0, TFT_BLACK, Align2::LEFT_TOP);
        // FreeSansBold24pt
        l.sans_fit(&price(net, cur), 300.0, 58.0, 33.0, 190.0, C_DEDB, Align2::RIGHT_TOP);
    }

    pub fn loading(&self, l: &Lcd) {
        l.bg(&self.init);
        l.sans(concat!("v", env!("CARGO_PKG_VERSION"), " emu"), 24.0, 147.0, 11.0, TFT_BLACK, Align2::LEFT_TOP);
    }
}

/// Best difficulty like the firmware's `suffix_string` (e.g. "12.3K", "0.004").
fn best_diff(d: f64) -> String {
    if d < 1000.0 {
        if d < 10.0 { format!("{d:.3}") } else { format!("{d:.1}") }
    } else {
        diff_short(d)
    }
}

fn diff_short(d: f64) -> String {
    const S: [(f64, &str); 6] = [(1e18, "E"), (1e15, "P"), (1e12, "T"), (1e9, "G"), (1e6, "M"), (1e3, "K")];
    for (v, suf) in S {
        if d >= v {
            let x = d / v;
            return if x >= 100.0 { format!("{x:.0}{suf}") } else if x >= 10.0 { format!("{x:.1}{suf}") } else { format!("{x:.2}{suf}") };
        }
    }
    format!("{d:.0}")
}
