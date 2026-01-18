mod app;
mod config;
mod network;
mod packet;
mod tray;
mod ui;

use egui::IconData;
use local_ip_address::local_ip;
use std::sync::Arc;
use ui::egui_ui::{EguiApp, UiState};

fn load_icon() -> IconData {
    let (icon_rgba, icon_width, icon_height) = {
        // Nota: include_bytes! es relativo al archivo actual (main.rs)
        // main.rs está en src/, así que subimos uno (..) y entramos a assets/
        let image = image::load_from_memory(include_bytes!("../assets/Icon.png"))
            .expect("No se pudo cargar el icono (assets/Icon.png)")
            .into_rgba8();
        let (width, height) = image.dimensions();
        (image.into_raw(), width, height)
    };

    IconData {
        rgba: icon_rgba,
        width: icon_width,
        height: icon_height,
    }
}

fn main() -> eframe::Result<()> {
    let my_ip = local_ip().unwrap_or_else(|_| "0.0.0.0".parse().unwrap());
    // ===================== GLOBAL STATE =====================
    let state = Arc::new(app::AppState::new(my_ip.to_string(), "9000".to_string()));
    // ===================== SYSTEM TRAY =====================
    let _tray = tray::spawn_tray(state.clone());
    // ===================== UI STATE =====================
    let ui_state = UiState {
        peer_ip: String::new(),
        peer_port: "9000".to_string(),
        virtual_ip: "10.0.0.1".to_string(),
    };
    // ===================== APP =====================
    let app = EguiApp {
        state: state.clone(),
        // state: app::AppState::new(my_ip.to_string()),
        ui: ui_state,
        power_on_texture: None,
        power_off_texture: None,
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("HecateVPN")
            .with_icon(load_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "Mini LAN Bridge",
        native_options,
        Box::new(|_| Box::new(app)),
    )
}
