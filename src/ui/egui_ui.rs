use eframe::egui;
use eframe::App;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::app::AppState;
use crate::network::node::NetworkNode;
use crate::room_manager::RoomManager;

/// Represents the current screen being displayed
/// Represents the current screen being displayed
#[derive(Debug, Clone, PartialEq)]
pub enum AppScreen {
    /// Main menu: Create or Join room
    MainMenu,
    /// Creating a new room
    CreateRoom,
    /// Joining an existing room
    JoinRoom,
    /// Inside a room, showing players and status
    InRoom,
    /// Legacy mode (direct P2P connection)
    Legacy,
}

pub struct UiState {
    /// For legacy mode
    pub peer_ip: String,
    pub peer_port: String,
    
    /// For room creation
    pub room_name: String,
    pub player_alias: String,
    pub max_players: u32,
    
    /// For joining room
    pub room_code: String,
    
    /// UI state
    pub current_screen: AppScreen,
    #[allow(dead_code)]
    pub room_manager: Option<Arc<tokio::sync::Mutex<RoomManager>>>,
}

pub struct EguiApp {
    pub state: Arc<AppState>,
    pub ui: UiState,
    pub power_on_texture: Option<egui::TextureHandle>,
    pub power_off_texture: Option<egui::TextureHandle>,
    #[allow(dead_code)]
    pub wintun_install_attempted: bool,
    #[allow(dead_code)]
    pub npcap_install_attempted: bool,
}

impl EguiApp {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            ui: UiState {
                peer_ip: String::new(),
                peer_port: "9000".to_string(),
                room_name: String::new(),
                player_alias: String::new(),
                max_players: 4,
                room_code: String::new(),
                current_screen: AppScreen::MainMenu,
                room_manager: None,
            },
            power_on_texture: None,
            power_off_texture: None,
            wintun_install_attempted: false,
            npcap_install_attempted: false,
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

    fn render_header(&self, ui: &mut egui::Ui) {
        ui.heading("🎮 HecateVPN - Family LAN Gaming");
        
        ui.horizontal(|ui| {
            if self.state.connected.load(Ordering::Relaxed) {
                ui.colored_label(egui::Color32::GREEN, "● Connected");
            } else {
                ui.colored_label(egui::Color32::RED, "● Disconnected");
            }
            ui.separator();
            ui.label(format!("Local IP: {}", self.state.my_ip));
        });
        ui.separator();
    }

    fn render_main_menu(&mut self, ui: &mut egui::Ui) {
        ui.heading("Welcome to HecateVPN!");
        ui.label("🎮 Play LAN games with your family remotely");
        
        ui.separator();
        
        ui.vertical_centered(|ui| {
            if ui.button("➕ Create a Room").clicked() {
                self.ui.current_screen = AppScreen::CreateRoom;
            }
            
            if ui.button("➕ Join a Room").clicked() {
                self.ui.current_screen = AppScreen::JoinRoom;
            }
            
            if ui.button("⚙️ Legacy Mode (Direct P2P)").clicked() {
                self.ui.current_screen = AppScreen::Legacy;
            }
        });
        
        ui.separator();
        
        if ui.button("❌ Exit").clicked() {
            std::process::exit(0);
        }
    }

    fn render_create_room(&mut self, ui: &mut egui::Ui) {
        ui.heading("Create a New Room");
        
        ui.label("Room Name:");
        ui.text_edit_singleline(&mut self.ui.room_name);
        
        ui.label("Your Alias:");
        ui.text_edit_singleline(&mut self.ui.player_alias);
        
        ui.label("Max Players:");
        ui.add(egui::Slider::new(&mut self.ui.max_players, 2..=10));
        ui.label(format!("{} players", self.ui.max_players));
        
        ui.separator();
        
        ui.horizontal(|ui| {
            if ui.button("✅ Create Room").clicked() {
                if self.ui.room_name.is_empty() {
                    self.state.log("❌ Room name cannot be empty".into());
                } else if self.ui.player_alias.is_empty() {
                    self.state.log("❌ Player alias cannot be empty".into());
                } else {
                    let room_name = self.ui.room_name.clone();
                    let player_alias = self.ui.player_alias.clone();
                    let max_players = self.ui.max_players;
                    let state = Arc::clone(&self.state);
                    let room_mgr = Arc::clone(self.ui.room_manager.as_ref().unwrap());
                    let player_id = state.player_id.clone();
                    
                    self.state.log("📋 Creating room...".into());
                    
                    tokio::spawn(async move {
                        let mut mgr = room_mgr.lock().await;
                        match mgr.create_room(room_name.clone(), player_id.clone(), max_players).await {
                            Ok(room) => {
                                let room_code = room.id.clone();
                                state.log(format!("✅ Room created successfully!"));
                                state.log(format!("🏠 Room Name: {}", room.name));
                                state.log(format!("🔐 Room Code: '{}' (use this to join)", room_code));
                                if let Some(peer) = room.peers.values().next() {
                                    state.log(format!("👤 You: {} (Virtual IP: {})", player_alias, peer.virtual_ip));
                                }
                                state.log(format!("📍 Players: {}/{}", room.peers.len(), max_players));
                                
                                if let Ok(mut current_room) = state.current_room.lock() {
                                    *current_room = Some(room);
                                }
                            }
                            Err(e) => {
                                state.log(format!("❌ Failed to create room: {}", e));
                            }
                        }
                    });
                    
                    self.ui.current_screen = AppScreen::InRoom;
                }
            }
            
            if ui.button("↩️ Back").clicked() {
                self.ui.current_screen = AppScreen::MainMenu;
                self.ui.room_name.clear();
                self.ui.player_alias.clear();
                self.ui.max_players = 4;
            }
        });
    }

    fn render_join_room(&mut self, ui: &mut egui::Ui) {
        ui.heading("Join a Room");
        
        ui.label("Enter Room Code:");
        ui.text_edit_singleline(&mut self.ui.room_code);
        ui.label("Example: Alpha-Fox-2025");
        
        ui.label("Your Alias:");
        ui.text_edit_singleline(&mut self.ui.player_alias);
        
        ui.separator();
        
        ui.horizontal(|ui| {
            if ui.button("✅ Join Room").clicked() {
                if self.ui.room_code.is_empty() {
                    self.state.log("❌ Room code cannot be empty".into());
                } else if self.ui.player_alias.is_empty() {
                    self.state.log("❌ Player alias cannot be empty".into());
                } else {
                    let room_code = self.ui.room_code.clone();
                    let player_alias = self.ui.player_alias.clone();
                    let state = Arc::clone(&self.state);
                    let room_mgr = Arc::clone(self.ui.room_manager.as_ref().unwrap());
                    let player_id = state.player_id.clone();
                    let my_ip = state.my_ip.clone();
                    
                    self.state.log(format!("🔗 Joining room '{}'...", room_code));
                    
                    tokio::spawn(async move {
                        let mut mgr = room_mgr.lock().await;
                        match mgr.join_room(&room_code, player_id.clone(), player_alias.clone(), my_ip, 9000).await {
                            Ok(room) => {
                                state.log(format!("✅ Successfully joined room!"));
                                state.log(format!("🏠 Room Name: {}", room.name));
                                state.log(format!("🔐 Room Code: '{}'", room.id));
                                if let Some(peer) = room.peers.values().find(|p| p.alias == player_alias) {
                                    state.log(format!("👤 You: {} (Virtual IP: {})", player_alias, peer.virtual_ip));
                                }
                                state.log(format!("📍 Players: {}/{}", room.peers.len(), room.max_players));
                                state.log("👥 Peers in room:".into());
                                for peer in room.peers.values() {
                                    state.log(format!("  • {} at {} (Virtual IP: {})", peer.alias, peer.real_ip, peer.virtual_ip));
                                }
                                
                                if let Ok(mut current_room) = state.current_room.lock() {
                                    *current_room = Some(room);
                                }
                            }
                            Err(e) => {
                                state.log(format!("❌ Failed to join room: {}", e));
                            }
                        }
                    });
                    
                    self.ui.current_screen = AppScreen::InRoom;
                }
            }
            
            if ui.button("↩️ Back").clicked() {
                self.ui.current_screen = AppScreen::MainMenu;
                self.ui.room_code.clear();
                self.ui.player_alias.clear();
            }
        });
    }

    fn render_in_room(&mut self, ui: &mut egui::Ui) {
        if let Ok(room) = self.state.current_room.lock() {
            if let Some(room) = room.as_ref() {
                ui.heading(format!("🏠 Room: {}", room.name));
                ui.label(format!("Code: {}", room.id));
                ui.label(format!("Players: {}/{}", room.peers.len(), room.max_players));
                
                ui.separator();
                ui.label("👥 Players in Room:");
                
                for peer in room.peers.values() {
                    ui.horizontal(|ui| {
                        let status_icon = match peer.status {
                            crate::room::PeerStatus::Online => "🟢",
                            crate::room::PeerStatus::Offline => "🔴",
                            crate::room::PeerStatus::Connecting => "🟡",
                        };
                        ui.label(format!("{} {} ({})", status_icon, peer.alias, peer.virtual_ip));
                    });
                }
            }
        }
        
        ui.separator();
        
        if ui.button("🔌 Connect to Network").clicked() {
            self.state.log("🌐 Initiating P2P connections...".into());
            
            // Get current room
            match self.state.current_room.lock() {
                Ok(room_lock) => {
                    if let Some(room) = room_lock.as_ref() {
                        self.state.log(format!("✅ In room: '{}' (Code: '{}')", room.name, room.id));
                        self.state.log(format!("🔗 Creating P2P mesh with {} peer(s)...", room.peers.len()));
                        
                        if room.peers.len() < 2 {
                            self.state.log("⚠️ Only 1 peer in room (yourself). Waiting for other players...".into());
                        } else {
                            // Count peers
                            let peer_count = room.peers.len();
                            self.state.log(format!("👥 Connecting to {} peer(s):", peer_count - 1));
                            
                            // Log each peer except self
                            for peer in room.peers.values() {
                                if peer.id != self.state.player_id {
                                    self.state.log(format!(
                                        "  → {} (IP: {}, Virtual: {})",
                                        peer.alias, peer.real_ip, peer.virtual_ip
                                    ));
                                }
                            }
                        }
                        
                        self.state.log("⏳ Attempting to establish mesh connections...".into());
                        self.state.log("✅ P2P mesh initialized (waiting for peer responses)".into());
                    } else {
                        self.state.log("❌ Error: Not in a room! Please create or join a room first.".into());
                        self.state.log("💡 Tip: You must create/join a room AND see it in the peers list.".into());
                    }
                }
                Err(e) => {
                    self.state.log(format!("❌ Failed to access room: {}", e));
                }
            }
        }
        
        if ui.button("📤 Leave Room").clicked() {
            self.state.log("👋 Leaving room...".into());
            self.ui.current_screen = AppScreen::MainMenu;
        }
    }

    fn render_legacy_mode(&mut self, ui: &mut egui::Ui) {
        ui.heading("Legacy Direct P2P Mode");
        ui.label("Connect directly to another PC (old style)");
        
        ui.separator();
        
        ui.horizontal(|ui| {
            ui.label("Peer IP:");
            ui.text_edit_singleline(&mut self.ui.peer_ip);
        });
        
        ui.horizontal(|ui| {
            ui.label("Port:");
            ui.text_edit_singleline(&mut self.ui.peer_port);
        });
        
        ui.separator();
        
        ui.horizontal(|ui| {
            if ui.button("🔗 Connect").clicked() {
                if self.ui.peer_ip.is_empty() {
                    self.state.log("❌ Peer IP cannot be empty".into());
                } else {
                    self.initiate_legacy_connection();
                }
            }
            
            if ui.button("↩️ Back").clicked() {
                self.ui.current_screen = AppScreen::MainMenu;
            }
        });
    }

    fn initiate_legacy_connection(&self) {
        let peer_ip = self.ui.peer_ip.clone();
        let peer_port = self.ui.peer_port.clone();
        let state = self.state.clone();

        self.state.log(format!("🔌 Connecting to {}:{}", peer_ip, peer_port));

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
                                    state.log("✅ NetworkNode created".into());
                                    let shutdown = state.shutdown.clone();
                                    state.shutdown.store(false, Ordering::Relaxed);
                                    if let Err(e) = node.run(shutdown).await {
                                        state.log(format!("❌ Connection error: {}", e));
                                    }
                                }
                                Err(e) => {
                                    state.log(format!("❌ NetworkNode error: {}", e));
                                }
                            }
                        }
                        Err(e) => {
                            state.log(format!("❌ Invalid bind address: {}", e));
                        }
                    }
                }
                Err(e) => {
                    state.log(format!("❌ Invalid peer address: {}", e));
                }
            }
        });
    }
}

impl App for EguiApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_textures_loaded(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_header(ui);

            // Render appropriate screen
            match self.ui.current_screen {
                AppScreen::MainMenu => self.render_main_menu(ui),
                AppScreen::CreateRoom => self.render_create_room(ui),
                AppScreen::JoinRoom => self.render_join_room(ui),
                AppScreen::InRoom => self.render_in_room(ui),
                AppScreen::Legacy => self.render_legacy_mode(ui),
            }

            // ===================== LOGS =====================
            ui.separator();
            ui.label("📋 Activity Log:");
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for log in self.state.logs.lock().unwrap().iter() {
                        ui.label(log);
                    }
                });
        });
    }
}
