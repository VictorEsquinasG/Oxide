use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::net::UdpSocket;
use socket2::{Socket, Domain, Type};

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

use crate::app::AppState;
use crate::packet::{ControlMessage, Packet, PacketPayload};

/// NetworkNode manages direct UDP communication without virtual interfaces.
/// Data flows directly: Local App → UDP Socket → Peer
#[derive(Clone)]
pub struct NetworkNode {
    socket: Arc<UdpSocket>,
    peer: Arc<tokio::sync::Mutex<SocketAddr>>, // Make peer mutable to handle NAT reflexive addresses
    state: Arc<AppState>,
}

impl NetworkNode {
    /// Initialize the VPN node with direct UDP tunnel (no TUN device required)
    /// The connection is transparent - data goes directly over UDP
    pub async fn new(
        bind_addr: SocketAddr,
        peer: SocketAddr,
        state: Arc<AppState>,
        _virtual_ip: &str,
    ) -> anyhow::Result<Self> {
        // Try to get or create a shared socket
        let socket = {
            let mut socket_guard = state.shared_socket.lock().unwrap();
            
            if let Some(existing_socket) = socket_guard.as_ref() {
                // Reuse existing socket
                existing_socket.clone()
            } else {
                // Create a new socket using socket2 to set options BEFORE binding
                let socket2 = Socket::new(Domain::for_address(bind_addr), Type::DGRAM, None)?;
                
                // Set SO_REUSEADDR BEFORE binding to allow immediate reuse
                socket2.set_reuse_address(true)?;
                
                #[cfg(unix)]
                {
                    // On Unix, also set SO_REUSEPORT for even better reuse behavior
                    let true_val: libc::c_int = 1;
                    unsafe {
                        let result = libc::setsockopt(
                            socket2.as_raw_fd(),
                            libc::SOL_SOCKET,
                            libc::SO_REUSEPORT,
                            &true_val as *const _ as *const libc::c_void,
                            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                        );
                        if result != 0 {
                            state.log("⚠️ Warning: Could not set SO_REUSEPORT".into());
                        }
                    }
                }
                
                #[cfg(windows)]
                {
                    // On Windows, set SO_REUSEADDR to allow address reuse
                    // This is sufficient - we don't need to disable SO_EXCLUSIVEADDRUSE
                    // since set_reuse_address(true) achieves the same effect
                }
                
                // Now bind the socket
                socket2.bind(&bind_addr.into())?;
                socket2.set_nonblocking(true)?;
                
                // Convert to Tokio UdpSocket
                let std_socket = std::net::UdpSocket::from(socket2);
                let socket = UdpSocket::from_std(std_socket)?;
                let socket = Arc::new(socket);
                
                // Store in shared state
                *socket_guard = Some(socket.clone());
                socket
            }
        };

        state.log(format!("🔌 Socket bound on {}, peer={}", bind_addr, peer));
        state.log(format!("📡 Direct UDP tunnel configured (no TUN device needed)"));

        Ok(Self {
            socket,
            peer: Arc::new(tokio::sync::Mutex::new(peer)),
            state,
        })
    }

    // Accept Shutdown flag
    pub async fn run(&self, shutdown: Arc<AtomicBool>) -> anyhow::Result<()> {
        let mut buf_udp = [0u8; 4096]; // Buffer for UDP packets

        let ping_interval = tokio::time::Duration::from_secs(2);
        let mut last_ping = tokio::time::Instant::now();

        self.state.log("🤝 Sending HELLO".into());
        let hello = Packet::hello();
        let peer_addr = *self.peer.lock().await;
        match self.socket.send_to(&hello.encode(), peer_addr).await {
            Ok(bytes) => {
                self.state.log(format!("✅ HELLO sent ({} bytes to {})", bytes, peer_addr));
            }
            Err(e) => {
                self.state.log(format!("❌ Failed to send HELLO: {}", e));
                return Err(e.into());
            }
        }

        let start = tokio::time::Instant::now();
        let timeout = tokio::time::Duration::from_secs(5);

        self.auto_ping();

        let mut packet_count = 0;
        let mut unknown_source_count = 0;

        loop {
            // Check for shutdown request
            if shutdown.load(Ordering::Relaxed) {
                self.state.log("🛑 Network loop stopped".into());
                break;
            }
            // Check for timeout
            if !self.state.connected.load(Ordering::Relaxed) && start.elapsed() > timeout {
                self.state.log(format!(
                    "❌ Connection failed (timeout after {:.1}s, received {} packets)",
                    start.elapsed().as_secs_f32(),
                    packet_count
                ));
                return Ok(());
            }
            // Check for real disconnection
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let last_seen = self.state.last_seen.load(Ordering::Relaxed);

            if self.state.connected.load(Ordering::Relaxed) && now.saturating_sub(last_seen) > 6 {
                self.state.connected.store(false, Ordering::Relaxed);
                self.state.log("❌ Peer not responding (timeout)".into());
                return Ok(());
            }

            // Send PING periodically
            if self.state.connected.load(Ordering::Relaxed) && last_ping.elapsed() > ping_interval {
                let ping = Packet::ping();
                let current_peer = *self.peer.lock().await;
                let _ = self.socket.send_to(&ping.encode(), current_peer).await;
                last_ping = tokio::time::Instant::now();
            }

            // Direct UDP packet handling - no TUN device
            match self.socket.try_recv_from(&mut buf_udp) {
                Ok((n, src)) => {
                    packet_count += 1;
                    let expected_peer = *self.peer.lock().await;
                    
                    self.state.log(format!(
                        "📨 Received {} bytes from {}",
                        n, src
                    ));

                    // Check if this is from the expected peer OR if we haven't verified the peer yet
                    let is_expected_peer = src == expected_peer;
                    let is_same_port = src.port() == expected_peer.port();
                    let is_peer_candidate = !self.state.connected.load(Ordering::Relaxed) && is_same_port;
                    
                    if is_expected_peer || is_peer_candidate {
                        // If from a different address but same port and not connected, 
                        // update peer address (handles NAT reflexive addresses)
                        if !is_expected_peer && is_peer_candidate {
                            self.state.log(format!(
                                "🔄 NAT detected! Updating peer address from {} to {}",
                                expected_peer, src
                            ));
                            *self.peer.lock().await = src;
                        }
                        
                        self.state.log(format!(
                            "✅ Packet from peer: {}",
                            src
                        ));
                        
                        if let Ok(packet) = Packet::decode(&buf_udp[..n]) {
                            match packet.payload {
                                PacketPayload::Control(msg) => match msg {
                                    ControlMessage::Hello => {
                                        self.state.log("👋 HELLO received".into());
                                        let ack = Packet::hello_ack();
                                        let current_peer = *self.peer.lock().await;
                                        match self.socket.send_to(&ack.encode(), current_peer).await {
                                            Ok(bytes) => {
                                                self.state
                                                    .log(format!("✅ HELLO_ACK sent ({} bytes)", bytes));
                                                // Mark as connected after successful HELLO_ACK response
                                                if !self.state.connected.load(Ordering::Relaxed) {
                                                    self.state.connected.store(true, Ordering::Relaxed);
                                                    self.state.update_last_seen();
                                                    self.state.log("✅ Connected (sent HELLO_ACK)".into());
                                                }
                                            }
                                            Err(e) => {
                                                self.state.log(format!("❌ Failed to send HELLO_ACK: {}", e));
                                            }
                                        }
                                    }
                                    ControlMessage::HelloAck => {
                                        if !self.state.connected.load(Ordering::Relaxed) {
                                            self.state.connected.store(true, Ordering::Relaxed);
                                            self.state.update_last_seen();
                                            self.state.log("✅ Connected (received HELLO_ACK)".into());
                                        } else {
                                            self.state.log("ℹ️ Additional HELLO_ACK received".into());
                                        }
                                    }
                                    ControlMessage::Ping => {
                                        self.state.log("📍 PING received".into());
                                        let pong = Packet::pong();
                                        let current_peer = *self.peer.lock().await;
                                        match self.socket.send_to(&pong.encode(), current_peer).await {
                                            Ok(bytes) => {
                                                self.state.log(format!("🏓 PONG sent ({} bytes)", bytes));
                                            }
                                            Err(e) => {
                                                self.state.log(format!("❌ Failed to send PONG: {}", e));
                                            }
                                        }
                                    }
                                    ControlMessage::Pong => {
                                        self.state.update_last_seen();
                                        self.state.log("🏓 PONG received".into());
                                    }
                                },
                                PacketPayload::Data(_frame) => {
                                    // Direct UDP tunnel - data is handled by application
                                    // No TUN device needed for forwarding
                                    self.state.log("📦 Data packet received (UDP tunnel)".into());
                                }
                            }
                        } else {
                            self.state.log("⚠️ Failed to decode packet from peer".into());
                        }
                    } else {
                        unknown_source_count += 1;
                        self.state.log(format!(
                            "⚠️ Packet from unknown source: {} (expected: {}, count: {})",
                            src, expected_peer, unknown_source_count
                        ));
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available - this is expected for non-blocking socket
                }
                Err(e) => {
                    self.state.log(format!("❌ Socket receive error: {}", e));
                }
            }

            // Small sleep to avoid busy waiting
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        Ok(())
    }

    fn auto_ping(&self) -> () {
        let socket = self.socket.clone();
        let peer = self.peer.clone();
        let state = self.state.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));

            loop {
                interval.tick().await;

                if state.shutdown.load(Ordering::Relaxed) {
                    break;
                }

                if state.connected.load(Ordering::Relaxed) {
                    let ping = Packet::ping();
                    let current_peer = *peer.lock().await;
                    let _ = socket.send_to(&ping.encode(), current_peer).await;
                }
            }
        });
    }

    /// Send a packet through the UDP socket
    pub async fn send(&self, packet: &Packet) -> anyhow::Result<()> {
        let bytes = packet.encode();
        let current_peer = *self.peer.lock().await;
        self.socket.send_to(&bytes, current_peer).await?;
        Ok(())
    }
}
