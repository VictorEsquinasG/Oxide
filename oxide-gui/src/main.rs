#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod tray;
mod ui;

use egui::IconData;
use local_ip_address::local_ip;
use oxide_core::app::AppState;
use oxide_core::room_manager::RoomManager;
use std::sync::Arc;
use std::thread;
use ui::egui_ui::EguiApp;

/// The GUI has no console on Windows, so errors would be invisible.
/// Write them to a log file and show a dialog (rfd needs no GPU).
fn report_fatal(msg: &str) {
    let dir = oxide_core::vpn_config::config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let log = dir.join("oxide-gui.log");
    let _ = std::fs::write(&log, msg);
    rfd::MessageDialog::new()
        .set_title("Oxide")
        .set_level(rfd::MessageLevel::Error)
        .set_description(format!("{}\n\n(Detalles en {})", msg, log.display()))
        .show();
}

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

    // ===================== SYSTEM TRAY (Non-blocking) =====================
    // Spawn tray in background, silently fail if GTK/system tray unavailable
    let state_tray = state.clone();
    thread::spawn(move || {
        if let Err(e) = tray::node::spawn_tray_safe(state_tray) {
            // Tray is optional - log but don't panic
            eprintln!("⚠️ Tray icon unavailable: {}", e);
        }
    });

    // ===================== GUI APP =====================
    // Store room_manager in Arc to pass to app_creator closure
    let room_mgr_arc = room_manager.clone();

    std::panic::set_hook(Box::new(|info| {
        report_fatal(&format!("Oxide se cerró por un error interno:\n{}", info));
    }));

    // Try the modern renderer first, then OpenGL. OXIDE_RENDERER=glow|wgpu forces one.
    let order: Vec<eframe::Renderer> = match std::env::var("OXIDE_RENDERER").as_deref() {
        Ok("glow") => vec![eframe::Renderer::Glow],
        Ok("wgpu") => vec![eframe::Renderer::Wgpu],
        _ => vec![eframe::Renderer::Wgpu, eframe::Renderer::Glow],
    };

    let mut errors = Vec::new();
    for renderer in order {
        let name = format!("{:?}", renderer);
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Oxide")
                .with_icon(load_icon()),
            renderer,
            ..Default::default()
        };
        let app_state = state.clone();
        let room_mgr_arc = room_manager.clone();
        let app_creator = Box::new(
            move |_cc: &eframe::CreationContext<'_>| -> Box<dyn eframe::App> {
                let mut app = EguiApp::new(app_state.clone());
                app.ui.room_manager = room_mgr_arc.clone();
                Box::new(app)
            },
        );
        match eframe::run_native("Mini LAN Bridge", options, app_creator) {
            Ok(()) => return Ok(()),
            Err(err) => {
                eprintln!("Renderer {} failed: {}", name, err);
                errors.push(format!("{}: {}", name, err));
            }
        }
    }

    report_fatal(&format!(
        "No se pudo abrir la ventana: no hay gráficos compatibles (¿máquina virtual sin GPU?).\n\n{}\n\nAlternativa sin ventana: oxide-cli.exe connect HOST --key CLAVE (como Administrador).",
        errors.join("\n")
    ));
    Ok(())
}
