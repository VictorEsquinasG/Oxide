use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use crate::room::Room;
use crate::network::P2PNetwork;

use std::time::{SystemTime, UNIX_EPOCH};

/// Global application state shared across UI and network layers
/// Implements thread-safe atomic variables for state coordination
pub struct AppState {
    /// Local machine IP address
    pub my_ip: String,
    /// Flag indicating active connection status
    pub connected: Arc<AtomicBool>,
    /// Flag to signal shutdown to all async tasks
    pub shutdown: Arc<AtomicBool>,
    /// Timestamp of last activity (NAT keep-alive tracking)
    pub last_seen: AtomicU64,
    /// Application log messages for UI display
    pub logs: Arc<Mutex<Vec<String>>>,
    /// Shared UDP socket for P2P communication
    pub shared_socket: Arc<Mutex<Option<Arc<UdpSocket>>>>,
    /// Currently active room (None if not in a room)
    pub current_room: Arc<Mutex<Option<Room>>>,
    /// Current player ID/alias
    pub player_id: String,
    /// P2P mesh network manager (None if not active)
    pub p2p_network: Arc<Mutex<Option<P2PNetwork>>>,
}

impl AppState {
    /// Factory method: Create new application state
    /// 
    /// # Arguments
    /// * `my_ip` - Local machine IP address
    /// * `player_id` - Player identifier/alias
    pub fn new(my_ip: String, player_id: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        Self {
            my_ip: my_ip.trim().to_string(),
            player_id,
            connected: Arc::new(AtomicBool::new(false)),
            shutdown: Arc::new(AtomicBool::new(false)),
            logs: Arc::new(Mutex::new(Vec::new())),
            last_seen: AtomicU64::new(now),
            shared_socket: Arc::new(Mutex::new(None)),
            current_room: Arc::new(Mutex::new(None)),
            p2p_network: Arc::new(Mutex::new(None)),
        }
    }

    /// Update the last activity timestamp
    /// Used for NAT keep-alive and connection timeout tracking
    pub fn update_last_seen(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_seen.store(now, Ordering::Relaxed);
    }

    /// Add a log message to the internal log buffer
    /// Messages are displayed in the UI and automatically trimmed to 150 items
    pub fn log(&self, message: String) {
        let mut logs = self.logs.lock().unwrap();

        logs.push(format!("> {}", message));
        if logs.len() > 150 {
            logs.remove(0);
        }
    }
}
