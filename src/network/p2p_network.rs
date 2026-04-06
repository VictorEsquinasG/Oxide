// P2P Mesh Network Manager
// Handles multi-peer UDP connections, discovery, and keep-alive

use crate::room::{Room, Peer, PeerStatus};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents a connection to a single peer
#[derive(Clone)]
pub struct PeerConnection {
    pub peer: Peer,
    pub socket: Arc<UdpSocket>,
    pub last_seen: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub latency_ms: u32,
}

/// Manages P2P mesh network for a room
pub struct P2PNetwork {
    /// Current room being networked
    pub current_room: Arc<Mutex<Option<Room>>>,
    
    /// All peer connections in the room
    pub connections: Arc<Mutex<HashMap<String, PeerConnection>>>,
    
    /// This peer's ID
    pub my_peer_id: String,
    
    /// Local UDP socket bound to 0.0.0.0:9000
    pub local_socket: Arc<UdpSocket>,
    
    /// Whether the mesh is active
    pub active: Arc<AtomicBool>,
    
    /// Shutdown signal
    pub shutdown: Arc<AtomicBool>,
}

impl P2PNetwork {
    /// Create a new P2P network manager
    pub async fn new(
        my_peer_id: String,
        room: Room,
    ) -> Result<Self, String> {
        // Bind to local UDP socket
        let socket = UdpSocket::bind("0.0.0.0:9000")
            .await
            .map_err(|e| format!("Failed to bind UDP socket: {}", e))?;

        socket.set_broadcast(true)
            .map_err(|e| format!("Failed to set broadcast: {}", e))?;

        Ok(Self {
            current_room: Arc::new(Mutex::new(Some(room))),
            connections: Arc::new(Mutex::new(HashMap::new())),
            my_peer_id,
            local_socket: Arc::new(socket),
            active: Arc::new(AtomicBool::new(false)),
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Start the P2P mesh - discover and connect to all peers
    pub async fn start_mesh(&self) -> Result<(), String> {
        let room = self.current_room.lock().await;
        let room = room.as_ref().ok_or("No room active")?;

        // Don't try to connect to ourselves
        for (peer_id, peer) in &room.peers {
            if peer_id == &self.my_peer_id {
                continue; // Skip self
            }

            // Create connection to this peer
            self.connect_to_peer(peer.clone()).await?;
        }

        self.active.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Establish connection to a specific peer
    async fn connect_to_peer(&self, peer: Peer) -> Result<(), String> {
        let socket_addr: SocketAddr = peer.socket_addr()
            .parse()
            .map_err(|e| format!("Invalid peer address: {}", e))?;

        // Create a UDP connection to the peer
        let peer_socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| format!("Failed to bind peer socket: {}", e))?;

        // Try to connect (UDP "connect" just sets default destination)
        peer_socket.connect(socket_addr)
            .await
            .map_err(|e| format!("Failed to connect to peer: {}", e))?;

        // Send initial HELLO packet
        let hello_packet = format!("HELLO:{}", self.my_peer_id);
        peer_socket.send(hello_packet.as_bytes())
            .await
            .map_err(|e| format!("Failed to send HELLO: {}", e))?;

        // Create connection record
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let connection = PeerConnection {
            peer: peer.clone(),
            socket: Arc::new(peer_socket),
            last_seen: now,
            packets_sent: 1, // HELLO packet
            packets_received: 0,
            latency_ms: 0,
        };

        let mut conns = self.connections.lock().await;
        conns.insert(peer.id.clone(), connection);

        Ok(())
    }

    /// Send a packet to a specific peer
    pub async fn send_to_peer(&self, peer_id: &str, data: &[u8]) -> Result<(), String> {
        let conns = self.connections.lock().await;
        let conn = conns.get(peer_id)
            .ok_or("Peer not connected")?;

        conn.socket.send(data)
            .await
            .map_err(|e| format!("Send failed: {}", e))?;

        Ok(())
    }

    /// Broadcast a packet to all peers
    pub async fn broadcast(&self, data: &[u8]) -> Result<u32, String> {
        let conns = self.connections.lock().await;
        let mut sent = 0u32;

        for (_, conn) in conns.iter() {
            if let Ok(_) = conn.socket.send(data).await {
                sent += 1;
            }
        }

        Ok(sent)
    }

    /// Receive packets from all peers (non-blocking)
    pub async fn receive_from_peers(&self) -> Result<(String, Vec<u8>), String> {
        let mut buffer = [0u8; 4096];
        let (n, addr) = self.local_socket.recv_from(&mut buffer)
            .await
            .map_err(|e| format!("Receive failed: {}", e))?;

        // Find which peer this came from
        let conns = self.connections.lock().await;
        let peer_id = conns
            .values()
            .find(|c| c.socket.peer_addr().map(|pa| pa == addr).unwrap_or(false))
            .map(|c| c.peer.id.clone())
            .ok_or("Unknown sender")?;

        Ok((peer_id, buffer[..n].to_vec()))
    }

    /// Update peer status in the mesh
    pub async fn update_peer_status(&self, peer_id: &str, status: PeerStatus) -> Result<(), String> {
        let mut conns = self.connections.lock().await;
        
        if let Some(conn) = conns.get_mut(peer_id) {
            conn.peer.status = status;
            
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            conn.last_seen = now;
            
            Ok(())
        } else {
            Err("Peer not found".to_string())
        }
    }

    /// Get connection statistics for a peer
    pub async fn get_peer_stats(&self, peer_id: &str) -> Result<(u64, u64, u32), String> {
        let conns = self.connections.lock().await;
        
        let conn = conns.get(peer_id)
            .ok_or("Peer not connected")?;
        
        Ok((conn.packets_sent, conn.packets_received, conn.latency_ms))
    }

    /// List all connected peers
    pub async fn list_connected_peers(&self) -> Result<Vec<Peer>, String> {
        let conns = self.connections.lock().await;
        Ok(conns.values().map(|c| c.peer.clone()).collect())
    }

    /// Shutdown the mesh network
    pub async fn shutdown(&self) -> Result<(), String> {
        self.shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
        self.active.store(false, std::sync::atomic::Ordering::Relaxed);
        
        let mut conns = self.connections.lock().await;
        conns.clear();
        
        Ok(())
    }

    /// Check if mesh is active and healthy
    pub async fn is_healthy(&self) -> bool {
        if !self.active.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }

        let conns = self.connections.lock().await;
        
        // Healthy if at least one peer is connected
        conns.iter().any(|(_, conn)| conn.peer.status == PeerStatus::Online)
    }

    /// Keep-alive: send periodic PING packets
    pub async fn keep_alive(&self) -> Result<(), String> {
        let ping = b"PING";
        let sent = self.broadcast(ping).await?;
        
        if sent == 0 {
            return Err("No peers to ping".to_string());
        }
        
        Ok(())
    }
}

/// Packet types for P2P protocol
#[derive(Debug, Clone)]
pub enum P2PPacket {
    Hello { peer_id: String },
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },
    Data { data: Vec<u8> },
    Disconnect { reason: String },
}

impl P2PPacket {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            P2PPacket::Hello { peer_id } => {
                format!("HELLO:{}", peer_id).into_bytes()
            }
            P2PPacket::Ping { timestamp } => {
                format!("PING:{}", timestamp).into_bytes()
            }
            P2PPacket::Pong { timestamp } => {
                format!("PONG:{}", timestamp).into_bytes()
            }
            P2PPacket::Data { data } => {
                let mut buf = vec![0u8; 5 + data.len()];
                buf[0..5].copy_from_slice(b"DATA:");
                buf[5..].copy_from_slice(data);
                buf
            }
            P2PPacket::Disconnect { reason } => {
                format!("DISC:{}", reason).into_bytes()
            }
        }
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let s = String::from_utf8(data.to_vec())
            .map_err(|e| format!("Invalid UTF-8: {}", e))?;

        if s.starts_with("HELLO:") {
            let peer_id = s.strip_prefix("HELLO:").unwrap().to_string();
            Ok(P2PPacket::Hello { peer_id })
        } else if s.starts_with("PING:") {
            let ts_str = s.strip_prefix("PING:").unwrap();
            let timestamp = ts_str.parse()
                .map_err(|_| "Invalid timestamp")?;
            Ok(P2PPacket::Ping { timestamp })
        } else if s.starts_with("PONG:") {
            let ts_str = s.strip_prefix("PONG:").unwrap();
            let timestamp = ts_str.parse()
                .map_err(|_| "Invalid timestamp")?;
            Ok(P2PPacket::Pong { timestamp })
        } else if s.starts_with("DATA:") {
            let data = s.strip_prefix("DATA:").unwrap().as_bytes().to_vec();
            Ok(P2PPacket::Data { data })
        } else if s.starts_with("DISC:") {
            let reason = s.strip_prefix("DISC:").unwrap().to_string();
            Ok(P2PPacket::Disconnect { reason })
        } else {
            Err("Unknown packet type".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_serialization() {
        let pkt = P2PPacket::Hello { peer_id: "test".to_string() };
        let bytes = pkt.to_bytes();
        let pkt2 = P2PPacket::from_bytes(&bytes).unwrap();
        
        match pkt2 {
            P2PPacket::Hello { peer_id } => assert_eq!(peer_id, "test"),
            _ => panic!("Wrong packet type"),
        }
    }
}
