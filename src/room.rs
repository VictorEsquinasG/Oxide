// Room and Peer management structures
// Handles multi-player room configuration and player state

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a single peer/player in a room
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Peer {
    /// Unique identifier for the peer (e.g., "player1", "dad", etc.)
    pub id: String,
    /// Human-readable alias (e.g., "Player 1", "Dad", "Alice")
    pub alias: String,
    /// Virtual IP assigned in the room subnet (e.g., 10.0.0.1)
    pub virtual_ip: String,
    /// Real external IP address
    pub real_ip: String,
    /// UDP port for communication
    pub port: u16,
    /// Connection status (online/offline/connecting)
    pub status: PeerStatus,
    /// Last seen timestamp (for timeout detection)
    pub last_seen: u64,
}

/// Status of a peer in the room
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PeerStatus {
    Online,
    Offline,
    Connecting,
}

impl Peer {
    /// Create a new peer with default status
    pub fn new(
        id: String,
        alias: String,
        virtual_ip: String,
        real_ip: String,
        port: u16,
    ) -> Self {
        Self {
            id,
            alias,
            virtual_ip,
            real_ip,
            port,
            status: PeerStatus::Offline,
            last_seen: 0,
        }
    }

    /// Get the socket address (real_ip:port)
    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.real_ip, self.port)
    }
}

/// Represents a virtual LAN room
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Room {
    /// Unique room identifier (e.g., "HEXAGON-2024")
    pub id: String,
    /// Human-readable room name (e.g., "Family Gaming Night")
    pub name: String,
    /// ID of the peer who created the room
    pub creator_id: String,
    /// Virtual network subnet (e.g., "10.0.0.0/24")
    pub virtual_network: String,
    /// All peers in the room (key: peer id, value: peer data)
    pub peers: HashMap<String, Peer>,
    /// Maximum number of players allowed (1-10)
    pub max_players: u32,
    /// Whether the room is currently active
    pub active: bool,
    /// Timestamp of room creation
    pub created_at: u64,
    /// Timestamp of last activity
    pub last_activity: u64,
}

impl Room {
    /// Create a new room
    pub fn new(
        id: String,
        name: String,
        creator_id: String,
        max_players: u32,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id,
            name,
            creator_id,
            virtual_network: "10.0.0.0/24".to_string(),
            peers: HashMap::new(),
            max_players: max_players.min(10), // Cap at 10 players
            active: true,
            created_at: now,
            last_activity: now,
        }
    }

    /// Add a peer to the room (if space available)
    pub fn add_peer(&mut self, peer: Peer) -> Result<(), String> {
        if self.peers.len() >= self.max_players as usize {
            return Err("Room is full".to_string());
        }

        if self.peers.contains_key(&peer.id) {
            return Err("Peer already in room".to_string());
        }

        self.peers.insert(peer.id.clone(), peer);
        self.update_activity();
        Ok(())
    }

    /// Remove a peer from the room
    pub fn remove_peer(&mut self, peer_id: &str) -> Result<(), String> {
        if self.peers.remove(peer_id).is_some() {
            self.update_activity();
            Ok(())
        } else {
            Err("Peer not found".to_string())
        }
    }

    /// Get next available virtual IP in the subnet
    /// Returns IPs like 10.0.0.1, 10.0.0.2, etc.
    pub fn next_virtual_ip(&self) -> Result<String, String> {
        // Simple approach: iterate from 1 to 254 (leaving .0 and .255)
        for i in 1..=254 {
            let ip = format!("10.0.0.{}", i);
            if !self.peers.values().any(|p| p.virtual_ip == ip) {
                return Ok(ip);
            }
        }
        Err("No available IPs in subnet".to_string())
    }

    /// Update last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    /// Check if room has space for new peer
    pub fn has_space(&self) -> bool {
        self.peers.len() < self.max_players as usize
    }

    /// Get list of online peers
    pub fn online_peers(&self) -> Vec<Peer> {
        self.peers
            .values()
            .filter(|p| p.status == PeerStatus::Online)
            .cloned()
            .collect()
    }

    /// Get peer by ID
    pub fn get_peer(&self, peer_id: &str) -> Option<&Peer> {
        self.peers.get(peer_id)
    }

    /// Get peer by ID (mutable)
    pub fn get_peer_mut(&mut self, peer_id: &str) -> Option<&mut Peer> {
        self.peers.get_mut(peer_id)
    }
}

/// Configuration for a set of rooms (persistence)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoomConfig {
    /// Version of the config format (for future migrations)
    pub version: u32,
    /// List of all rooms
    pub rooms: HashMap<String, Room>,
    /// Default player alias preference
    pub default_alias: String,
}

impl RoomConfig {
    /// Create new empty room configuration
    pub fn new(default_alias: String) -> Self {
        Self {
            version: 1,
            rooms: HashMap::new(),
            default_alias,
        }
    }

    /// Get room by ID
    pub fn get_room(&self, room_id: &str) -> Option<&Room> {
        self.rooms.get(room_id)
    }

    /// Get room by ID (mutable)
    pub fn get_room_mut(&mut self, room_id: &str) -> Option<&mut Room> {
        self.rooms.get_mut(room_id)
    }

    /// Add or update a room
    pub fn save_room(&mut self, room: Room) {
        self.rooms.insert(room.id.clone(), room);
    }

    /// Remove a room
    pub fn delete_room(&mut self, room_id: &str) -> Result<(), String> {
        if self.rooms.remove(room_id).is_some() {
            Ok(())
        } else {
            Err("Room not found".to_string())
        }
    }

    /// List all room IDs
    pub fn list_rooms(&self) -> Vec<String> {
        self.rooms.keys().cloned().collect()
    }
}

/// Generates a simple room code (e.g., "HEXAGON-2024")
pub fn generate_room_code() -> String {
    use rand::Rng;

    const ADJECTIVES: &[&str] = &[
        "Alpha", "Beta", "Gamma", "Delta", "Epic", "Inferno", "Phantom", "Shadow",
        "Swift", "Thunder", "Titan", "Viper", "Nexus", "Cyber", "Mystic", "Cosmic",
    ];

    const NOUNS: &[&str] = &[
        "Fox", "Wolf", "Eagle", "Dragon", "Phoenix", "Raven", "Tiger", "Cobra",
        "Hydra", "Sphinx", "Demon", "Kraken", "Beast", "Knight", "Warrior", "Legend",
    ];

    let mut rng = rand::thread_rng();
    let adj = ADJECTIVES[rng.gen_range(0..ADJECTIVES.len())];
    let noun = NOUNS[rng.gen_range(0..NOUNS.len())];
    let number = rng.gen_range(2020..2030);

    format!("{}-{}-{}", adj, noun, number)
}
