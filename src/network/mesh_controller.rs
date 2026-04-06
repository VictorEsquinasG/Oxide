// Mesh Network Initialization and Lifecycle Management
// Bridges UI actions with P2P network startup and monitoring

use crate::app::AppState;
use crate::network::P2PNetwork;
use std::sync::Arc;

/// Handles initialization and lifecycle of P2P mesh network
pub struct MeshController {
    state: Arc<AppState>,
}

impl MeshController {
    /// Create new mesh controller
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Initialize P2P mesh from current room
    /// Called when user clicks "Connect to Network" button
    pub async fn initialize_mesh(&self) -> Result<(), String> {
        // Step 1: Get current room
        let room = {
            let room_opt = self.state.current_room.lock()
                .map_err(|e| format!("Failed to lock room: {}", e))?;
            
            room_opt.clone().ok_or_else(|| "No active room".to_string())?
        };

        self.state.log(format!("📡 Initializing P2P mesh for room: {}", room.name));
        self.state.log(format!("🎯 Room ID: {}", room.id));
        self.state.log(format!("👥 Players in room: {}", room.peers.len()));

        // Step 2: Validate peer count
        if room.peers.is_empty() {
            return Err("Cannot start mesh: no peers in room".to_string());
        }

        if room.peers.len() == 1 {
            self.state.log("⚠️ Only one peer in room (yourself)".into());
            self.state.log("✅ Mesh initialized (waiting for other players)".into());
            return Ok(());
        }

        // Step 3: Log peer details
        self.state.log("📋 Mesh participants:".into());
        for (peer_id, peer) in &room.peers {
            let my_marker = if peer_id == &self.state.player_id { " ← YOU" } else { "" };
            self.state.log(format!(
                "  • {} (IP: {}, Address: {}){}",
                peer.alias, peer.virtual_ip, peer.real_ip, my_marker
            ));
        }

        // Step 4: Create P2PNetwork instance
        self.state.log("🔧 Creating P2P network manager...".into());
        
        let my_peer_id = self.state.player_id.clone();
        let p2p_network = match P2PNetwork::new(my_peer_id, room.clone()).await {
            Ok(network) => network,
            Err(e) => {
                self.state.log(format!("❌ Failed to create P2P network: {}", e));
                return Err(format!("Network creation failed: {}", e));
            }
        };

        // Step 5: Store in AppState
        {
            let mut p2p_opt = self.state.p2p_network.lock()
                .map_err(|e| format!("Failed to lock p2p_network: {}", e))?;
            *p2p_opt = Some(p2p_network);
        }

        self.state.log("✅ P2P network manager created".into());

        // Step 6: Start mesh connections
        self.state.log("🔗 Starting mesh connections...".into());
        match self.start_mesh_connections().await {
            Ok(_) => {
                self.state.log("✅ Mesh initialization complete!".into());
                self.state.connected.store(true, std::sync::atomic::Ordering::Relaxed);
                Ok(())
            }
            Err(e) => {
                self.state.log(format!("❌ Mesh startup failed: {}", e));
                Err(e)
            }
        }
    }

    /// Start actual mesh connections to peers
    async fn start_mesh_connections(&self) -> Result<(), String> {
        // Get P2PNetwork and call start_mesh directly without cloning
        {
            let p2p_opt = self.state.p2p_network.lock()
                .map_err(|e| format!("Failed to lock p2p_network: {}", e))?;
            
            let network = p2p_opt.as_ref()
                .ok_or_else(|| "P2P network not initialized".to_string())?;

            // Start mesh - this will connect to all peers
            match network.start_mesh().await {
                Ok(_) => {
                    self.state.log("🔗 P2P mesh started - connecting to peers...".into());
                    
                    // Get peer count from room
                    let room = self.state.current_room.lock()
                        .ok()
                        .and_then(|r| r.as_ref().map(|room| room.peers.len()))
                        .unwrap_or(0);

                    self.state.log(format!("⏳ Connecting to {} peer(s)...", room.saturating_sub(1)));
                    
                    Ok(())
                }
                Err(e) => {
                    self.state.log(format!("❌ Failed to start mesh: {}", e));
                    Err(e)
                }
            }
        }
    }

    /// Stop the mesh network and clean up connections
    pub async fn stop_mesh(&self) -> Result<(), String> {
        self.state.log("🛑 Stopping P2P mesh...".into());

        let mut p2p_opt = self.state.p2p_network.lock()
            .map_err(|e| format!("Failed to lock p2p_network: {}", e))?;

        if let Some(network) = p2p_opt.take() {
            // Signal shutdown
            network.shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
            self.state.log("✅ P2P mesh stopped".into());
        } else {
            self.state.log("⚠️ No active mesh to stop".into());
        }

        self.state.connected.store(false, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Get current mesh status
    pub async fn get_mesh_status(&self) -> MeshStatus {
        let is_active = {
            let p2p_opt = self.state.p2p_network.lock().ok();
            p2p_opt.and_then(|r| r.as_ref().map(|n| n.active.load(std::sync::atomic::Ordering::Relaxed)))
                .unwrap_or(false)
        };

        let peer_count = {
            let room = self.state.current_room.lock().ok();
            room.and_then(|r| r.as_ref().map(|room| room.peers.len()))
                .unwrap_or(0)
        };

        let is_connected = self.state.connected.load(std::sync::atomic::Ordering::Relaxed);

        MeshStatus {
            is_active,
            is_connected,
            peer_count,
            connected_peers: 0, // Will be implemented with peer state tracking
        }
    }

    /// Get metrics for a specific peer
    pub async fn get_peer_metrics(&self, peer_id: &str) -> Option<PeerMetrics> {
        let p2p_opt = self.state.p2p_network.lock().ok()?;
        let network = p2p_opt.as_ref()?;

        // Get peer stats from network
        match network.get_peer_stats(peer_id).await {
            Ok((packets_sent, packets_received, latency_ms)) => {
                Some(PeerMetrics {
                    peer_id: peer_id.to_string(),
                    packets_sent,
                    packets_received,
                    latency_ms,
                    is_connected: latency_ms > 0,
                })
            }
            Err(_) => None,
        }
    }

    /// Broadcast data to all peers in mesh
    pub async fn broadcast_to_peers(&self, data: &[u8]) -> Result<u32, String> {
        let p2p_opt = self.state.p2p_network.lock()
            .map_err(|e| format!("Failed to lock p2p_network: {}", e))?;

        let network = p2p_opt.as_ref()
            .ok_or_else(|| "P2P network not initialized".to_string())?;

        network.broadcast(data).await
    }

    /// Send data to specific peer
    pub async fn send_to_peer(&self, peer_id: &str, data: &[u8]) -> Result<(), String> {
        let p2p_opt = self.state.p2p_network.lock()
            .map_err(|e| format!("Failed to lock p2p_network: {}", e))?;

        let network = p2p_opt.as_ref()
            .ok_or_else(|| "P2P network not initialized".to_string())?;

        network.send_to_peer(peer_id, data).await
    }
}

/// Current status of the P2P mesh
#[derive(Debug, Clone)]
pub struct MeshStatus {
    /// Whether mesh is actively running
    pub is_active: bool,
    /// Whether we have established connections
    pub is_connected: bool,
    /// Total peers in the room
    pub peer_count: usize,
    /// Number of successfully connected peers
    pub connected_peers: usize,
}

impl MeshStatus {
    /// Get status emoji
    pub fn emoji(&self) -> &str {
        match (self.is_active, self.is_connected) {
            (true, true) => "🟢",    // Fully connected
            (true, false) => "🟡",   // Connecting
            (false, _) => "⚫",       // Inactive
        }
    }

    /// Get human-readable status
    pub fn description(&self) -> String {
        match (self.is_active, self.is_connected) {
            (true, true) => format!(
                "Connected to {}/{} peers",
                self.connected_peers, self.peer_count
            ),
            (true, false) => "Connecting...".to_string(),
            (false, _) => "Offline".to_string(),
        }
    }
}

/// Metrics for a specific peer connection
#[derive(Debug, Clone)]
pub struct PeerMetrics {
    pub peer_id: String,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub latency_ms: u32,
    pub is_connected: bool,
}

impl PeerMetrics {
    /// Get connection quality indicator
    pub fn quality_emoji(&self) -> &str {
        match self.latency_ms {
            0..=10 => "🟢",    // Excellent
            11..=50 => "🟢",   // Good
            51..=100 => "🟡",  // Fair
            101..=200 => "🟠", // Poor
            _ => "🔴",         // Very Poor
        }
    }

    /// Get connection quality description
    pub fn quality_description(&self) -> &str {
        match self.latency_ms {
            0..=10 => "Excellent",
            11..=50 => "Good",
            51..=100 => "Fair",
            101..=200 => "Poor",
            _ => "Very Poor",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_status_emoji() {
        let status_active_connected = MeshStatus {
            is_active: true,
            is_connected: true,
            peer_count: 3,
            connected_peers: 2,
        };
        assert_eq!(status_active_connected.emoji(), "🟢");

        let status_active_not_connected = MeshStatus {
            is_active: true,
            is_connected: false,
            peer_count: 3,
            connected_peers: 0,
        };
        assert_eq!(status_active_not_connected.emoji(), "🟡");

        let status_inactive = MeshStatus {
            is_active: false,
            is_connected: false,
            peer_count: 0,
            connected_peers: 0,
        };
        assert_eq!(status_inactive.emoji(), "⚫");
    }

    #[test]
    fn test_mesh_status_description() {
        let status = MeshStatus {
            is_active: true,
            is_connected: true,
            peer_count: 3,
            connected_peers: 2,
        };
        assert_eq!(status.description(), "Connected to 2/3 peers");
    }

    #[test]
    fn test_peer_metrics_quality() {
        let metrics_excellent = PeerMetrics {
            peer_id: "test".to_string(),
            packets_sent: 100,
            packets_received: 100,
            latency_ms: 5,
            is_connected: true,
        };
        assert_eq!(metrics_excellent.quality_emoji(), "🟢");
        assert_eq!(metrics_excellent.quality_description(), "Excellent");

        let metrics_poor = PeerMetrics {
            peer_id: "test".to_string(),
            packets_sent: 100,
            packets_received: 90,
            latency_ms: 150,
            is_connected: true,
        };
        assert_eq!(metrics_poor.quality_emoji(), "🟠");
        assert_eq!(metrics_poor.quality_description(), "Poor");
    }
}
