use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UdpSocket;

use crate::app::AppState;
use crate::packet::{ControlMessage, Packet, PacketPayload};

/// NetworkNode manages the bridge between the physical network (UDP)
/// and the virtual network (TAP interface)
#[derive(Clone)]
pub struct NetworkNode {
    socket: Arc<UdpSocket>,
    peer: SocketAddr,
    state: Arc<AppState>,
    tun_device: Arc<tokio::sync::Mutex<tun::AsyncDevice>>,
}

impl NetworkNode {
    /// Initialize the VPN node
    /// `virtual_ip`: The IP inside the VPN (e.g. 10.0.0.2)
    pub async fn new(
        bind_addr: SocketAddr,
        peer: SocketAddr,
        state: Arc<AppState>,
        virtual_ip: &str,
    ) -> anyhow::Result<Self> {
        // Bind UDP socket (Physical layer)
        let socket = UdpSocket::bind(bind_addr).await?;
        let socket = Arc::new(socket);

        state.log(format!("🔌 Socket bound on {}, peer={}", bind_addr, peer));

        // Configure TAP device (Virtual layer)
        let mut config = tun::Configuration::default();
        config
            .address(virtual_ip)
            .netmask("255.255.255.0")
            .destination(virtual_ip)
            .up();

        #[cfg(target_os = "linux")]
        config.platform(|config| {
            config.packet_information(true); // Linux specific header handling
        });
        #[cfg(target_os = "windows")]
        config.platform(|config| {
            // config.name("Ethernet");
        });
        let tun_device = tun::create_as_async(&config)?;
        let tun_device = Arc::new(tokio::sync::Mutex::new(tun_device));

        Ok(Self {
            socket,
            peer,
            state,
            tun_device,
        })
    }

    // Accept Shutdown flag
    pub async fn run(&self, shutdown: Arc<AtomicBool>) -> anyhow::Result<()> {
        let mut buf_udp = [0u8; 4096]; // Buffer for reading from Physical Interface

        let ping_interval = tokio::time::Duration::from_secs(2);
        let mut last_ping = tokio::time::Instant::now();

        self.state.log("🤝 Sending HELLO".into());
        let hello = Packet::hello();
        self.socket.send_to(&hello.encode(), self.peer).await?;

        let start = tokio::time::Instant::now();
        let timeout = tokio::time::Duration::from_secs(5);

        self.auto_ping();
        self.spawn_tun_reader(shutdown.clone());

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

            // Enviar PING periódicamente
            if self.state.connected.load(Ordering::Relaxed) && last_ping.elapsed() > ping_interval {
                let ping = Packet::ping();
                let _ = self.socket.send_to(&ping.encode(), self.peer).await;
                last_ping = tokio::time::Instant::now();
            }

            // Use tokio select to handle whichever comes first:
            // 1. The game sends a packet (Read from TUN)
            // 2. The peer sends a packet (Read from UDP)
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
                            PacketPayload::Data(frame) => {
                                let mut dev = self.tun_device.lock().await;
                                let _ = dev.write_all(&frame).await;
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

    /// Spawn the TUN reader task that forwards packets to the peer
    fn spawn_tun_reader(&self, shutdown: Arc<AtomicBool>) {
        let tun_device = self.tun_device.clone();
        let socket = self.socket.clone();
        let peer = self.peer;
        let state = self.state.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            loop {
                if shutdown.load(Ordering::Relaxed) {
                    state.log("🛑 TUN reader stopped".into());
                    break;
                }

                if !state.connected.load(Ordering::Relaxed) {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    continue;
                }

                let mut dev = tun_device.lock().await;
                match dev.read(&mut buf).await {
                    Ok(n) if n > 0 => {
                        let frame = buf[..n].to_vec();
                        let packet = Packet::data(frame);
                        if let Err(e) = socket.send_to(&packet.encode(), peer).await {
                            state.log(format!("❌ Failed to send data packet: {}", e));
                        }
                    }
                    Ok(_) => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }
                    Err(e) => {
                        state.log(format!("❌ TUN read error: {}", e));
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
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
