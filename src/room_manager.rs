// Room Manager - Handles persistence and room operations
// Saves/loads rooms from JSON file

#![allow(dead_code)]

use crate::room::{Room, RoomConfig, Peer, PeerStatus, generate_room_code};
use std::path::PathBuf;
use tokio::fs;

/// Manages room persistence and operations
pub struct RoomManager {
    config_path: PathBuf,
    config: RoomConfig,
}

impl RoomManager {
    /// Create a new RoomManager with default config path
    /// Path: ~/.Oxide/rooms.json
    pub async fn new(default_alias: String) -> Result<Self, String> {
        let config_dir = Self::get_config_dir()?;
        
        // Create directory if it doesn't exist
        fs::create_dir_all(&config_dir)
            .await
            .map_err(|e| format!("Failed to create config dir: {}", e))?;

        let config_path = config_dir.join("rooms.json");

        // Load existing config or create new
        let config = if config_path.exists() {
            Self::load_config_from_file(&config_path).await?
        } else {
            RoomConfig::new(default_alias.clone())
        };

        Ok(Self { config_path, config })
    }

    /// Get the configuration directory path
    fn get_config_dir() -> Result<PathBuf, String> {
        #[cfg(target_os = "windows")]
        {
            let app_data = std::env::var("APPDATA")
                .map_err(|_| "Failed to get APPDATA".to_string())?;
            Ok(PathBuf::from(app_data).join("Oxide"))
        }

        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME")
                .map_err(|_| "Failed to get HOME".to_string())?;
            Ok(PathBuf::from(home).join("Library/Application Support/Oxide"))
        }

        #[cfg(target_os = "linux")]
        {
            let home = std::env::var("HOME")
                .map_err(|_| "Failed to get HOME".to_string())?;
            Ok(PathBuf::from(home).join(".config/Oxide"))
        }
    }

    /// Load config from JSON file
    async fn load_config_from_file(path: &PathBuf) -> Result<RoomConfig, String> {
        let content = fs::read_to_string(path)
            .await
            .map_err(|e| format!("Failed to read config file: {}", e))?;

        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse config JSON: {}", e))
    }

    /// Save config to JSON file
    async fn save_config_to_file(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        fs::write(&self.config_path, json)
            .await
            .map_err(|e| format!("Failed to write config file: {}", e))?;

        Ok(())
    }

    /// Create a new room
    pub async fn create_room(
        &mut self,
        room_name: String,
        creator_id: String,
        max_players: u32,
    ) -> Result<Room, String> {
        let room_id = generate_room_code();
        let mut room = Room::new(room_id, room_name, creator_id.clone(), max_players);

        // Add creator as first peer with virtual IP 10.0.0.1
        let creator_peer = Peer::new(
            creator_id.clone(),
            self.config.default_alias.clone(),
            "10.0.0.1".to_string(),
            "0.0.0.0".to_string(), // Will be updated with real IP
            9000,
        );

        room.add_peer(creator_peer)?;

        self.config.save_room(room.clone());
        self.save_config_to_file().await?;

        Ok(room)
    }

    /// Join an existing room
    pub async fn join_room(
        &mut self,
        room_id: &str,
        peer_id: String,
        peer_alias: String,
        peer_real_ip: String,
        peer_port: u16,
    ) -> Result<Room, String> {
        let room = self
            .config
            .get_room_mut(room_id)
            .ok_or("Room not found")?
            .clone();

        if !room.has_space() {
            return Err("Room is full".to_string());
        }

        let virtual_ip = room.next_virtual_ip()?;

        let peer = Peer::new(peer_id, peer_alias, virtual_ip, peer_real_ip, peer_port);

        let mut updated_room = room;
        updated_room.add_peer(peer)?;

        self.config.save_room(updated_room.clone());
        self.save_config_to_file().await?;

        Ok(updated_room)
    }

    /// Leave a room (remove peer)
    pub async fn leave_room(&mut self, room_id: &str, peer_id: &str) -> Result<(), String> {
        let room = self
            .config
            .get_room_mut(room_id)
            .ok_or("Room not found")?;

        room.remove_peer(peer_id)?;

        // If no peers left, mark room as inactive
        if room.peers.is_empty() {
            room.active = false;
        }

        self.save_config_to_file().await?;
        Ok(())
    }

    /// Get a room by ID
    pub fn get_room(&self, room_id: &str) -> Option<Room> {
        self.config.get_room(room_id).cloned()
    }

    /// List all available rooms
    pub fn list_rooms(&self) -> Vec<Room> {
        self.config
            .rooms
            .values()
            .filter(|r| r.active)
            .cloned()
            .collect()
    }

    /// Reload config from file (for multi-instance support)
    pub async fn reload(&mut self) -> Result<(), String> {
        match Self::load_config_from_file(&self.config_path).await {
            Ok(config) => {
                self.config = config;
                Ok(())
            }
            Err(e) => {
                // If file doesn't exist, it's fine - just keep current config
                if self.config_path.exists() {
                    Err(e)
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Delete a room
    pub async fn delete_room(&mut self, room_id: &str) -> Result<(), String> {
        self.config.delete_room(room_id)?;
        self.save_config_to_file().await?;
        Ok(())
    }

    /// Update a peer's status
    pub async fn update_peer_status(
        &mut self,
        room_id: &str,
        peer_id: &str,
        status: PeerStatus,
    ) -> Result<(), String> {
        let room = self
            .config
            .get_room_mut(room_id)
            .ok_or("Room not found")?;

        if let Some(peer) = room.get_peer_mut(peer_id) {
            peer.status = status;
            room.update_activity();
        } else {
            return Err("Peer not found".to_string());
        }

        self.save_config_to_file().await?;
        Ok(())
    }

    /// Get current config
    pub fn config(&self) -> &RoomConfig {
        &self.config
    }

    /// Get mutable config
    pub fn config_mut(&mut self) -> &mut RoomConfig {
        &mut self.config
    }
}
