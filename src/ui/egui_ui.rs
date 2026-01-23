use eframe::egui; // Import egui to use TextureOptions and ColorImage
use eframe::App;
use std::sync::atomic::Ordering;

use crate::app::AppState;
use std::sync::Arc;
use crate::network::node::NetworkNode;

#[cfg(windows)]
use crate::system::wintun;

#[derive(Default)]
pub struct UiState {
    pub peer_ip: String,
    pub peer_port: String,
}

pub struct EguiApp {
    pub state: Arc<AppState>,
    pub ui: UiState,
    pub power_on_texture: Option<egui::TextureHandle>,
    pub power_off_texture: Option<egui::TextureHandle>,
    pub wintun_install_attempted: bool,
}

impl EguiApp {
    // Constructor to initialize the app with empty textures
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            ui: UiState::default(),
            power_on_texture: None,
            power_off_texture: None,
            wintun_install_attempted: false,
        }
    }

    fn load_texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> egui::TextureHandle {
        let image = image::load_from_memory(bytes).expect("Invalid PNG");
        let size = [image.width() as _, image.height() as _];
        let rgba = image.to_rgba8();
        let pixels = rgba.as_flat_samples();

        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());

        ctx.load_texture(name, color_image, egui::TextureOptions::default())
    }

    fn ensure_textures_loaded(&mut self, ctx: &egui::Context) {
        if self.power_on_texture.is_none() {
            self.power_on_texture = Some(Self::load_texture(
                ctx,
                "power_on",
                include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/power_on.png")),
            ));
        }

        if self.power_off_texture.is_none() {
            self.power_off_texture = Some(Self::load_texture(
                ctx,
                "power_off",
                include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/power_off.png")),
            ));
        }
    }

    fn toggle_connection(&mut self) {
        if self.state.connected.load(Ordering::Relaxed) {
            // Disconnect
            self.state.shutdown.store(true, Ordering::Relaxed);
            self.state.connected.store(false, Ordering::Relaxed);
            self.state.log("🛑 Disconnecting...".into());
        } else {
            // Check if wintun is installed (only on Windows)
            #[cfg(windows)]
            {
                if !wintun::is_wintun_installed() {
                    // Solo intenta instalar UNA VEZ, no en cada intento
                    if !self.wintun_install_attempted {
                        self.wintun_install_attempted = true;
                        self.state.log("⚠️ Wintun no encontrado. Iniciando instalación...".into());
                        
                        let state_for_progress = self.state.clone();
                        let on_progress = Arc::new(tokio::sync::Mutex::new(Box::new(move |msg: String| {
                            state_for_progress.log(msg);
                        }) as Box<dyn Fn(String) + Send>));
                        
                        let state_for_install = self.state.clone();
                        let _install_handle = tokio::spawn(async move {
                            match wintun::install_wintun(Some(on_progress)).await {
                                Ok(_) => {
                                    state_for_install.log("✅ Instalación completada. Por favor, reinicia HecateVPN.".into());
                                }
                                Err(e) => {
                                    state_for_install.log(format!("❌ Error en instalación: {}", e));
                                }
                            }
                        });
                        return;
                    } else {
                        self.state.log("⚠️ Instalación en progreso o completada. Por favor, reinicia la app.".into());
                        return;
                    }
                }
            }

            // Connect
            if self.ui.peer_ip.is_empty() {
                self.state.log("❌ IP del peer no puede estar vacía".into());
                return;
            }

            let peer_ip = self.ui.peer_ip.clone();
            let peer_port = self.ui.peer_port.clone();
            let state = self.state.clone();

            self.state.log(format!("🔌 Conectando a {}:{}", peer_ip, peer_port));

            // Spawn connection task
            tokio::spawn(async move {
                let peer_addr: String = format!("{}:{}", peer_ip, peer_port);
                let bind_addr = "0.0.0.0:9000";

                match peer_addr.parse::<std::net::SocketAddr>() {
                    Ok(peer_socket) => {
                        match bind_addr.parse::<std::net::SocketAddr>() {
                            Ok(bind_socket) => {
                                match NetworkNode::new(bind_socket, peer_socket, state.clone(), "10.0.0.1")
                                    .await
                                {
                                    Ok(node) => {
                                        state.log("✅ NetworkNode creado".into());
                                        let shutdown = state.shutdown.clone();
                                        state.shutdown.store(false, Ordering::Relaxed);
                                        if let Err(e) = node.run(shutdown).await {
                                            state.log(format!("❌ Error de conexión: {}", e));
                                        }
                                    }
                                    Err(e) => {
                                        state.log(format!("❌ Error al crear NetworkNode: {}", e));
                                    }
                                }
                            }
                            Err(e) => {
                                state.log(format!("❌ Dirección de bind inválida: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        state.log(format!("❌ Dirección del peer inválida: {}", e));
                    }
                }
            });
        }
    }
}

impl App for EguiApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_textures_loaded(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.state.connected.load(Ordering::Relaxed) {
                ui.colored_label(egui::Color32::GREEN, "● Connected");
            } else {
                ui.colored_label(egui::Color32::RED, "● Disconnected");
            }

            let icon = if self.state.connected.load(Ordering::Relaxed) {
                self.power_on_texture.as_ref().unwrap()
            } else {
                self.power_off_texture.as_ref().unwrap()
            };

            if ui
                .add(egui::ImageButton::new(icon).rounding(8.0).frame(true))
                .clicked()
            {
                self.toggle_connection();
            }

            // ===================== HEADER =====================
            ui.heading("Mini LAN Bridge");

            // ===================== CONTEXT INFO =====================
            ui.horizontal(|ui| {
                ui.label("My IP:");
                ui.label(&self.state.my_ip);
            });

            // ===================== DATA FORM =====================
            ui.horizontal(|ui| {
                ui.label("Peer IP:");
                ui.text_edit_singleline(&mut self.ui.peer_ip);

                ui.label("Port:");
                ui.text_edit_singleline(&mut self.ui.peer_port);
            });       

            if ui.button("Exit").clicked() {
                std::process::exit(0);
            }

            // ===================== LOGS =====================
            ui.separator();
            ui.label("Logs:");
            eframe::egui::ScrollArea::vertical().show(ui, |ui| {
                for log in self.state.logs.lock().unwrap().iter() {
                    ui.label(log);
                }
            });
        });
    }
}
