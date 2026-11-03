//! System tray icon (macOS menu bar, Windows notification area, Linux SNI/ksni).

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub const ID_TOGGLE: &str = "toggle";
pub const ID_PAUSE: &str = "pause";
pub const ID_QUIT: &str = "quit";

pub enum TrayCmd {
    Toggle,
    Pause,
    Quit,
}

pub struct Tray {
    pub icon: TrayIcon,
    pub toggle_item: MenuItem,
    pub pause_item: MenuItem,
}

/// 32x32 RGBA icon: orange circle with a dark "chip".
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

impl Tray {
    /// Must be called on the main thread after the event loop has started.
    pub fn new(on_cmd: impl Fn(TrayCmd) + Send + Sync + 'static) -> Result<Tray, String> {
        let toggle_item = MenuItem::with_id(ID_TOGGLE, "Ocultar janela", true, None);
        let pause_item = MenuItem::with_id(ID_PAUSE, "Pausar mineração", true, None);
        let quit_item = MenuItem::with_id(ID_QUIT, "Sair", true, None);
        let menu = Menu::new();
        menu.append(&toggle_item).map_err(|e| e.to_string())?;
        menu.append(&pause_item).map_err(|e| e.to_string())?;
        menu.append(&PredefinedMenuItem::separator()).map_err(|e| e.to_string())?;
        menu.append(&quit_item).map_err(|e| e.to_string())?;

        let icon = Icon::from_rgba(icon_rgba(32), 32, 32).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("NerdMiner Emulator")
            .with_icon(icon)
            .build()
            .map_err(|e| e.to_string())?;

        let on_cmd = std::sync::Arc::new(on_cmd);
        let h = on_cmd.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| match e.id.as_ref() {
            ID_TOGGLE => h(TrayCmd::Toggle),
            ID_PAUSE => h(TrayCmd::Pause),
            ID_QUIT => h(TrayCmd::Quit),
            _ => {}
        }));
        // Double-click on the icon (Windows/Linux) toggles the window.
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::DoubleClick { .. } = e {
                on_cmd(TrayCmd::Toggle)
            }
        }));

        Ok(Tray { icon: tray, toggle_item, pause_item })
    }
}
