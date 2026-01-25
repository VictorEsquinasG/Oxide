// This is TRAY ICON management code.
// Its function is secondary to main app logic and UI.
// This module spawns a separate thread
// Shows an icon in the system tray that indicates connection status.
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tray_icon::{
    menu::{Menu, MenuItem},
    Icon, TrayIconBuilder,
};

use crate::app::AppState;

/// Size of the tray icon
const ICON_SIZE: u32 = 32;

/// Base RGBA icon (raw pixels)
const BASE_ICON: &[u8] = include_bytes!("../../assets/icon.rgba");

/// Create a tray icon from RGBA bytes
fn icon_from_rgba(rgba: &[u8]) -> Icon {
    Icon::from_rgba(rgba.to_vec(), ICON_SIZE, ICON_SIZE).expect("Invalid RGBA icon")
}

/// Tint RGBA buffer with a solid color (preserving alpha)
fn tint_rgba(base: &[u8], r: u8, g: u8, b: u8) -> Vec<u8> {
    let mut out = base.to_vec();

    for px in out.chunks_mut(4) {
        if px[3] != 0 {
            px[0] = r;
            px[1] = g;
            px[2] = b;
        }
    }

    out
}

/// Spawn the system tray
pub fn spawn_tray(state: Arc<AppState>) {
    thread::spawn(move || {
        // ───── Icons ─────
        let icon_red = icon_from_rgba(&tint_rgba(BASE_ICON, 200, 0, 0));
        let icon_green = icon_from_rgba(&tint_rgba(BASE_ICON, 0, 200, 0));

        // ───── Menu ─────
        let menu = Menu::new();
        let quit_item = MenuItem::new("Quit", true, None);
        menu.append(&quit_item).unwrap();

        // ───── Tray ─────
        let _tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon_red.clone())
            .with_tooltip("Mini LAN Bridge")
            .build()
            .expect("Failed to create tray icon");

        // ───── Quit handling ─────
        // let state_quit = state.clone();
        // quit_item.set_callback(move || {
        //     state_quit.shutdown.store(true, Ordering::Relaxed);
        //     std::process::exit(0);
        // });

        // ───── Connection indicator loop ─────
        let mut last_connected = false;

        loop {
            let connected = state.connected.load(Ordering::Relaxed);

            if connected != last_connected {
                if connected {
                    let _ = _tray.set_icon(Some(icon_green.clone()));
                    let _ = _tray.set_tooltip(Some("Mini LAN Bridge — Connected"));
                } else {
                    let _ = _tray.set_icon(Some(icon_red.clone()));
                    let _ = _tray.set_tooltip(Some("Mini LAN Bridge — Disconnected"));
                }

                last_connected = connected;
            }

            thread::sleep(Duration::from_millis(500));
        }
    });
}
