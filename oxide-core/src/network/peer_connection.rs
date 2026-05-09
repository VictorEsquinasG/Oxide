// Peer Connection Handler
// Manages individual peer connections in the mesh

use crate::room::Peer;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

/// Maximum time without hearing from peer before disconnection
const PEER_TIMEOUT_SECS: u64 = 30;

/// Interval for checking peer health
const HEALTH_CHECK_INTERVAL_SECS: u64 = 5;

/// Individual peer connection state
#[derive(Debug, Clone)]
pub struct PeerConnectionState {
    pub peer: Peer,
    pub last_ping_time: u64,
    pub last_pong_time: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub latency_ms: u32,
    pub is_connected: bool,
    pub connection_time: u64,
}

impl PeerConnectionState {
    pub fn new(peer: Peer) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            peer,
            last_ping_time: now,
            last_pong_time: now,
            packets_sent: 0,
            packets_received: 0,
            latency_ms: 0,
            is_connected: false,
            connection_time: now,
        }
    }

    /// Check if peer has timed out
    pub fn is_timed_out(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now - self.last_pong_time > PEER_TIMEOUT_SECS
    }

    /// Get time since last packet
    pub fn time_since_last_packet(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now - self.last_pong_time.max(self.last_ping_time)
    }

    /// Get uptime in seconds
    pub fn uptime_secs(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now - self.connection_time
    }
}

/// Manages connection to a single peer
pub struct PeerConnection {
    pub state: Arc<Mutex<PeerConnectionState>>,
    pub socket: Arc<UdpSocket>,
    pub incoming_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    pub incoming_tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
}

impl PeerConnection {
    /// Create new peer connection
    pub async fn new(peer: Peer, socket: Arc<UdpSocket>) -> Result<Self, String> {
        let state = PeerConnectionState::new(peer);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            socket,
            incoming_rx: rx,
            incoming_tx: tx,
        })
    }

    /// Send data to peer
    pub async fn send_data(&self, data: &[u8]) -> Result<(), String> {
        let peer = {
            let state = self.state.lock().await;
            state.peer.clone()
        };

        match self.socket.send_to(data, peer.real_ip).await {
            Ok(_) => {
                let mut state = self.state.lock().await;
                state.packets_sent += 1;
                Ok(())
            }
            Err(e) => Err(format!("Send failed: {}", e)),
        }
    }

    /// Send PING packet and measure latency
    pub async fn send_ping(&self) -> Result<(), String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("Time error: {}", e))?
            .as_millis() as u64;

        let mut state = self.state.lock().await;
        state.last_ping_time = now;
        drop(state);

        let packet = format!("PING:{}", now);
        self.send_data(packet.as_bytes()).await
    }

    /// Handle received PING packet
    pub async fn handle_ping(&self, timestamp: u64) -> Result<(), String> {
        let packet = format!("PONG:{}", timestamp);
        self.send_data(packet.as_bytes()).await
    }

    /// Handle received PONG packet
    pub async fn handle_pong(&self, timestamp: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let latency = (now - timestamp) as u32;

        let mut state = self.state.lock().await;
        state.last_pong_time = now as u64;
        state.latency_ms = latency;
    }

    /// Receive and process packet from peer
    pub async fn receive_packet(&self, data: &[u8]) -> Result<(), String> {
        let mut state = self.state.lock().await;
        state.packets_received += 1;
        state.last_pong_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        drop(state);

        // Route packet to incoming channel
        if let Err(e) = self.incoming_tx.send(data.to_vec()) {
            return Err(format!("Channel error: {}", e));
        }

        Ok(())
    }

    /// Start health check routine
    pub async fn start_health_check(
        self: Arc<Self>,
        shutdown: Arc<Mutex<bool>>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(HEALTH_CHECK_INTERVAL_SECS));

            loop {
                ticker.tick().await;

                // Check if shutdown requested
                {
                    let is_shutdown = *shutdown.lock().await;
                    if is_shutdown {
                        break;
                    }
                }

                // Check timeout
                {
                    let state = self.state.lock().await;
                    if state.is_timed_out() {
                        eprintln!(
                            "Peer {} timed out after {} seconds",
                            state.peer.id, PEER_TIMEOUT_SECS
                        );
                        // Connection will be cleaned up by main mesh handler
                        break;
                    }
                }

                // Send ping
                if let Err(e) = self.send_ping().await {
                    eprintln!("Failed to send ping: {}", e);
                }
            }
        })
    }

    /// Get current state snapshot
    pub async fn get_state(&self) -> PeerConnectionState {
        self.state.lock().await.clone()
    }

    /// Mark as connected
    pub async fn mark_connected(&self) {
        let mut state = self.state.lock().await;
        state.is_connected = true;
        state.connection_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// Mark as disconnected
    pub async fn mark_disconnected(&self) {
        let mut state = self.state.lock().await;
        state.is_connected = false;
    }
}

/// Connection metrics for monitoring
#[derive(Debug, Clone)]
pub struct ConnectionMetrics {
    pub peer_id: String,
    pub is_connected: bool,
    pub latency_ms: u32,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub time_since_last_packet_secs: u64,
    pub uptime_secs: u64,
}

impl ConnectionMetrics {
    pub async fn from_connection(conn: &PeerConnection) -> Self {
        let state = conn.state.lock().await;
        Self {
            peer_id: state.peer.id.clone(),
            is_connected: state.is_connected,
            latency_ms: state.latency_ms,
            packets_sent: state.packets_sent,
            packets_received: state.packets_received,
            time_since_last_packet_secs: state.time_since_last_packet(),
            uptime_secs: state.uptime_secs(),
        }
    }
}

/// Connection quality rating
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionQuality {
    Excellent, // < 10ms
    Good,      // < 50ms
    Fair,      // < 100ms
    Poor,      // < 200ms
    VeryPoor,  // >= 200ms or disconnected
}

impl ConnectionQuality {
    pub fn from_latency(latency_ms: u32) -> Self {
        match latency_ms {
            0..=10 => ConnectionQuality::Excellent,
            11..=50 => ConnectionQuality::Good,
            51..=100 => ConnectionQuality::Fair,
            101..=200 => ConnectionQuality::Poor,
            _ => ConnectionQuality::VeryPoor,
        }
    }

    pub fn emoji(&self) -> &str {
        match self {
            ConnectionQuality::Excellent => "🟢",
            ConnectionQuality::Good => "🟢",
            ConnectionQuality::Fair => "🟡",
            ConnectionQuality::Poor => "🟠",
            ConnectionQuality::VeryPoor => "🔴",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room::{Peer, PeerStatus};

    #[test]
    fn test_connection_quality_levels() {
        assert_eq!(
            ConnectionQuality::from_latency(5),
            ConnectionQuality::Excellent
        );
        assert_eq!(ConnectionQuality::from_latency(30), ConnectionQuality::Good);
        assert_eq!(ConnectionQuality::from_latency(75), ConnectionQuality::Fair);
        assert_eq!(
            ConnectionQuality::from_latency(150),
            ConnectionQuality::Poor
        );
        assert_eq!(
            ConnectionQuality::from_latency(300),
            ConnectionQuality::VeryPoor
        );
    }

    #[test]
    fn test_connection_quality_emoji() {
        assert_eq!(ConnectionQuality::Excellent.emoji(), "🟢");
        assert_eq!(ConnectionQuality::Poor.emoji(), "🟠");
        assert_eq!(ConnectionQuality::VeryPoor.emoji(), "🔴");
    }

    #[test]
    fn test_peer_connection_state() {
        let peer = Peer {
            id: "test-peer".to_string(),
            alias: "TestPeer".to_string(),
            virtual_ip: "10.0.0.2".to_string(),
            real_ip: "192.168.1.100:9000".parse().unwrap(),
            port: 9000,
            status: PeerStatus::Online,
            last_seen: 0,
        };

        let state = PeerConnectionState::new(peer);
        assert!(!state.is_connected);
        assert!(!state.is_timed_out());
    }
}
