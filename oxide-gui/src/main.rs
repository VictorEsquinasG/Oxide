mod tray;
mod ui;

use egui::IconData;
use local_ip_address::local_ip;
use oxide_core::app::AppState;
use oxide_core::room_manager::RoomManager;
use std::sync::Arc;
use ui::egui_ui::EguiApp;

fn load_icon() -> IconData {
    let (icon_rgba, icon_width, icon_height) = {
        // Nota: include_bytes! es relativo al archivo actual (main.rs)
        // En oxide-gui este archivo está en oxide-gui/src/, así que subimos dos niveles
        let image = image::load_from_memory(include_bytes!("../../assets/Icon.png"))
            .expect("No se pudo cargar el icono (assets/Icon.png)")
            .into_rgba8();

        // Resize to 32x32 if needed
        let resized =
            image::imageops::resize(&image, 32, 32, image::imageops::FilterType::Lanczos3);

        let (width, height) = resized.dimensions();
        let rgba = resized.into_raw();
        (rgba, width, height)
    };

    IconData {
        rgba: icon_rgba,
        width: icon_width,
        height: icon_height,
    }
}

#[tokio::main]
async fn main() -> eframe::Result<()> {
    let my_ip = local_ip().unwrap_or_else(|_| "0.0.0.0".parse().unwrap());
    // ===================== GLOBAL STATE =====================
    let state = Arc::new(AppState::new(my_ip.to_string(), "Player".to_string()));

    // ===================== SYSTEM INFO ====================
    // VPN LAN Emulation - Direct P2P tunnel via UDP
    state.log("🎮 Oxide - LAN Emulation for Legacy Games".into());
    state.log("📡 Arquitectura: TUN Virtual Interface + UDP P2P".into());
    state.log("✅ Sistema listo para conectar".into());

    // ===================== ROOM MANAGER =====================
    // Initialize RoomManager for room creation/joining
    let room_manager = match RoomManager::new("Player".to_string()).await {
        Ok(manager) => {
            state.log("✅ Room Manager initialized successfully".into());
            Some(Arc::new(tokio::sync::Mutex::new(manager)))
        }
        Err(e) => {
            state.log(format!("❌ Failed to initialize Room Manager: {}", e));
            None
        }
    };

    // ===================== SYSTEM TRAY =====================
    let _tray = tray::node::spawn_tray(state.clone());

    // ===================== GUI APP =====================
    let mut app = EguiApp::new(state.clone());
    app.ui.room_manager = room_manager;

    let create_native_options = || {
        let mut options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Oxide")
                .with_icon(load_icon()),
            ..Default::default()
        };
        // Use wgpu as primary renderer for better cross-platform support
        // wgpu provides Vulkan/DirectX/Metal abstraction, better Windows/Linux compatibility
        // Fallback to Glow only if wgpu is unavailable
        options.renderer = eframe::Renderer::Wgpu;
        options
    };

    let app_state = state.clone();
    let app_creator = Box::new(
        move |cc: &eframe::CreationContext<'_>| -> Box<dyn eframe::App> {
            Box::new(EguiApp::new(app_state.clone()))
        },
    );

    match eframe::run_native("Mini LAN Bridge", create_native_options(), app_creator) {
        Ok(result) => Ok(result),
        Err(err) => {
            eprintln!("Failed to start GUI: {}", err);
            eprintln!(
                "This may be due to missing graphics drivers or running in a headless environment."
            );
            eprintln!("Please ensure your system has proper graphics support.");
            Err(err)
        }
    }
}
