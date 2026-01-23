use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;

use std::time::{SystemTime, UNIX_EPOCH};

pub struct AppState {
    pub my_ip: String,

    pub peer_ip: Arc<Mutex<String>>,
    pub peer_port: Arc<Mutex<String>>, // optional
    pub virtual_ip: String,            // IP inside VPN

    pub connected: Arc<AtomicBool>,
    pub shutdown: Arc<AtomicBool>,
    pub connection_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    pub last_seen: AtomicU64,
    pub logs: Arc<Mutex<Vec<String>>>,
    pub shared_socket: Arc<Mutex<Option<Arc<UdpSocket>>>>, // Shared UDP socket
}

impl AppState {
    pub fn new(my_ip: String, peer_port: String) -> Self {
        Self {
            my_ip: my_ip.trim().to_string(),
            peer_ip: Arc::new(Mutex::new(String::new())),
            peer_port: Arc::new(Mutex::new(peer_port)),
            virtual_ip: "10.0.0.1".to_string(),
            connected: Arc::new(AtomicBool::new(false)),
            shutdown: Arc::new(AtomicBool::new(false)),
            connection_handle: Arc::new(Mutex::new(None)),
            logs: Arc::new(Mutex::new(Vec::new())),
            last_seen: AtomicU64::new(0),
            shared_socket: Arc::new(Mutex::new(None)),
        }
    }

    pub fn update_last_seen(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_seen.store(now, Ordering::Relaxed);
    }

    pub fn log(&self, message: String) {
        let mut logs = self.logs.lock().unwrap();

        logs.push(format!("> {}", message));
        if logs.len() > 150 {
            logs.remove(0);
        }
    }
}
