use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::net::UdpSocket;

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

use crate::app::AppState;
use crate::packet::{ControlMessage, Packet, PacketPayload};

/// NetworkNode manages direct UDP communication without virtual interfaces.
/// Data flows directly: Local App → UDP Socket → Peer
#[derive(Clone)]
pub struct NetworkNode {
    socket: Arc<UdpSocket>,
    peer: SocketAddr,
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
        // Create a standard UDP socket with SO_REUSEADDR to allow quick reconnection
        let std_socket = std::net::UdpSocket::bind(bind_addr)?;
        
        // Set SO_REUSEADDR and SO_REUSEPORT to allow immediate reuse of the port
        #[cfg(unix)]
        {
            use nix::sys::socket::{setsockopt, sockopt};
            use std::os::unix::io::BorrowedFd;
            let raw_fd = std_socket.as_raw_fd();
            let fd = unsafe { BorrowedFd::borrow_raw(raw_fd) };
            let _ = setsockopt(&fd, sockopt::ReuseAddr, &true);
            let _ = setsockopt(&fd, sockopt::ReusePort, &true);
        }
        
        std_socket.set_nonblocking(true)?;
        let socket = UdpSocket::from_std(std_socket)?;
        let socket = Arc::new(socket);

        state.log(format!("🔌 Socket bound on {}, peer={}", bind_addr, peer));
        state.log(format!("📡 Direct UDP tunnel configured (no TUN device needed)"));

        Ok(Self {
            socket,
            peer,
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
        self.socket.send_to(&hello.encode(), self.peer).await?;

        let start = tokio::time::Instant::now();
        let timeout = tokio::time::Duration::from_secs(5);

        self.auto_ping();

        loop {
            // Check for shutdown request
            if shutdown.load(Ordering::Relaxed) {
                self.state.log("🛑 Network loop stopped".into());
                break;
            }
            // Check for timeout
            if !self.state.connected.load(Ordering::Relaxed) && start.elapsed() > timeout {
                self.state.log("❌ Connection failed (timeout)".into());
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
                let _ = self.socket.send_to(&ping.encode(), self.peer).await;
                last_ping = tokio::time::Instant::now();
            }

            // Direct UDP packet handling - no TUN device
            if let Ok((n, src)) = self.socket.try_recv_from(&mut buf_udp) {
                if src == self.peer {
                    if let Ok(packet) = Packet::decode(&buf_udp[..n]) {
                        match packet.payload {
                            PacketPayload::Control(msg) => match msg {
                                ControlMessage::Hello => {
                                    self.state.log("👋 HELLO received".into());
                                    let ack = Packet::hello_ack();
                                    let _ = self.socket.send_to(&ack.encode(), self.peer).await;
                                }
                                ControlMessage::HelloAck => {
                                    self.state.connected.store(true, Ordering::Relaxed);
                                    self.state.log("✅ Connected (HELLO_ACK)".into());
                                }
                                ControlMessage::Ping => {
                                    let pong = Packet::pong();
                                    let _ = self.socket.send_to(&pong.encode(), self.peer).await;
                                }
                                ControlMessage::Pong => {
                                    self.state.update_last_seen();
                                    self.state.log("🏓 Pong received".into());
                                }
                            },
                            PacketPayload::Data(_frame) => {
                                // Direct UDP tunnel - data is handled by application
                                // No TUN device needed for forwarding
                                self.state.log("📦 Data packet received (UDP tunnel)".into());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn auto_ping(&self) -> () {
        let socket = self.socket.clone();
        let peer = self.peer;
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
                    let _ = socket.send_to(&ping.encode(), peer).await;
                }
            }
        });
    }

    /// Send a packet through the UDP socket
    pub async fn send(&self, packet: &Packet) -> anyhow::Result<()> {
        let bytes = packet.encode();
        self.socket.send_to(&bytes, self.peer).await?;
        Ok(())
    }
}
