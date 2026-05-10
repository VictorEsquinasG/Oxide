// This is TRAY ICON management code.
// Its function is secondary to main app logic and UI.
// This module spawns a separate thread
// Shows an icon in the system tray that indicates connection status.
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use lazy_static::lazy_static;
use tray_icon::{
    menu::{Menu, MenuItem},
    Icon, TrayIconBuilder,
};

use oxide_core::app::AppState;

/// Size of the tray icon
const ICON_SIZE: u32 = 32;

// Base RGBA icon (raw pixels) - generated from PNG at runtime
lazy_static! {
    static ref BASE_ICON: Vec<u8> = {
        image::load_from_memory(include_bytes!("../../../assets/Icon.png"))
            .expect("Failed to load icon.png")
            .resize_exact(ICON_SIZE, ICON_SIZE, image::imageops::FilterType::Lanczos3)
            .into_rgba8()
            .into_raw()
    };
}

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

/// Spawn the system tray (silently fails if GTK/system tray unavailable)
/// Returns error if tray icon creation fails - caller should handle gracefully
pub fn spawn_tray_safe(state: Arc<AppState>) -> Result<(), String> {
    thread::spawn(move || {
        // Try to create tray - if it fails, just exit gracefully
        if let Err(e) = create_tray_icon(&state) {
            eprintln!("⚠️ Tray icon creation failed: {}", e);
        }
    });
    Ok(())
}

/// Create and manage the system tray icon
/// Returns early if GTK/tray is unavailable (e.g., headless environment)
fn create_tray_icon(state: &Arc<AppState>) -> Result<(), String> {
    // ───── Icons ─────
    let icon_red = icon_from_rgba(&tint_rgba(&BASE_ICON, 200, 0, 0));
    let icon_green = icon_from_rgba(&tint_rgba(&BASE_ICON, 0, 200, 0));

    // ───── Menu ─────
    let menu = Menu::new();
    let quit_item = MenuItem::new("Quit", true, None);
    menu.append(&quit_item).map_err(|e| format!("Failed to append quit menu: {}", e))?;

    // ───── Tray ─────
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon_red.clone())
        .with_tooltip("Mini LAN Bridge")
        .build()
        .map_err(|e| format!("Failed to build tray: {}", e))?;

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
}

/// Legacy spawn_tray - kept for compatibility
#[allow(dead_code)]
pub fn spawn_tray(state: Arc<AppState>) {
    let _ = spawn_tray_safe(state);
}
