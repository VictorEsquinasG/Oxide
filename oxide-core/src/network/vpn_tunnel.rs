/// VPN Tunnel integration - bridges TUN device with UDP network
///
/// Topology: hub-and-spoke. One machine is the *hub* (reachable on a public
/// UDP port, e.g. port-forwarded). Every other machine is a *client* that
/// points at the hub's address. Clients send keepalives so the hub learns
/// their NAT-mapped address; the hub relays traffic between clients
/// (unicast by learned virtual IP, broadcast/multicast to everyone).
use crate::app::AppState;
use crate::network::packet_handler;
use crate::system::tun_device::TunDevice;
use anyhow::Result;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;

/// Wire format for VPN packets sent over UDP.
///
/// ```text
/// [0..3]  magic  0x4F 0x58 0x44 ("OXD")
/// [3]     version (0x01)
/// [4]     packet type (0x01 = data, 0x02 = keepalive)
/// [5..]   payload: raw IPv4 packet (data) or empty (keepalive)
/// ```
pub const VPN_MAGIC: [u8; 3] = [0x4F, 0x58, 0x44];
pub const VPN_VERSION: u8 = 0x01;
pub const VPN_TYPE_DATA: u8 = 0x01;
pub const VPN_TYPE_KEEPALIVE: u8 = 0x02;

/// Size of the VPN header.
pub const VPN_HEADER_LEN: usize = 5;

const KEEPALIVE_EVERY: Duration = Duration::from_secs(5);
const PEER_TIMEOUT: Duration = Duration::from_secs(30);

/// Wrap a raw IPv4 packet into the VPN wire format.
pub fn encapsulate(ipv4_packet: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(VPN_HEADER_LEN + ipv4_packet.len());
    out.extend_from_slice(&VPN_MAGIC);
    out.push(VPN_VERSION);
    out.push(VPN_TYPE_DATA);
    out.extend_from_slice(ipv4_packet);
    out
}

/// Build a keepalive datagram. Payload = sender's virtual IP (4 bytes), so
/// the hub can map virtual IP -> address before any data flows.
pub fn keepalive_packet(our_ip: Ipv4Addr) -> [u8; VPN_HEADER_LEN + 4] {
    let mut p = [0u8; VPN_HEADER_LEN + 4];
    p[0..3].copy_from_slice(&VPN_MAGIC);
    p[3] = VPN_VERSION;
    p[4] = VPN_TYPE_KEEPALIVE;
    p[5..9].copy_from_slice(&our_ip.octets());
    p
}

/// Parse the VPN header. Returns (packet type, payload) or None if not a VPN packet.
pub fn parse(packet: &[u8]) -> Option<(u8, &[u8])> {
    if packet.len() < VPN_HEADER_LEN || packet[0..3] != VPN_MAGIC || packet[3] != VPN_VERSION {
        return None;
    }
    Some((packet[4], &packet[VPN_HEADER_LEN..]))
}

/// Try to decapsulate a VPN data packet received over UDP.
/// Returns the raw IPv4 payload, or None if this is not a VPN data packet.
pub fn decapsulate(packet: &[u8]) -> Option<&[u8]> {
    match parse(packet) {
        Some((VPN_TYPE_DATA, payload)) => Some(payload),
        _ => None,
    }
}

/// VPN Tunnel handler - bridges TUN and UDP
pub struct VpnTunnel {
    tun: TunDevice,
    socket: Arc<UdpSocket>,
    /// Addresses we always talk to (the hub, when we are a client)
    seeds: Vec<SocketAddr>,
    /// If true, relay traffic between peers
    hub: bool,
    state: Arc<AppState>,
    our_ip: Ipv4Addr,
    /// Peers we have heard from -> last time
    peers: HashMap<SocketAddr, Instant>,
    /// Virtual IP -> peer address, learned from source IPs of received packets
    vip_map: HashMap<Ipv4Addr, SocketAddr>,
}

impl VpnTunnel {
    /// Create a tunnel with no seeds (hub that waits for clients).
    pub async fn new(
        tun: TunDevice,
        socket: Arc<UdpSocket>,
        state: Arc<AppState>,
        our_ip: &str,
    ) -> Result<Self> {
        Self::with_peers(tun, socket, state, our_ip, Vec::new()).await
    }

    /// Create a tunnel that connects to the given seed addresses (the hub).
    pub async fn with_peers(
        tun: TunDevice,
        socket: Arc<UdpSocket>,
        state: Arc<AppState>,
        our_ip: &str,
        seeds: Vec<SocketAddr>,
    ) -> Result<Self> {
        let our_ipv4 = our_ip.split('/').next().unwrap_or(our_ip).parse::<Ipv4Addr>()?;
        state.log(format!("🌐 VPN Tunnel created: IP={}", our_ipv4));
        Ok(VpnTunnel {
            tun,
            socket,
            seeds,
            hub: false,
            state,
            our_ip: our_ipv4,
            peers: HashMap::new(),
            vip_map: HashMap::new(),
        })
    }

    /// Enable relaying between peers (use on the machine everyone connects to).
    pub fn set_hub(mut self, hub: bool) -> Self {
        self.hub = hub;
        self
    }

    /// Main loop: TUN -> UDP, UDP -> TUN, keepalives.
    pub async fn run(&mut self) -> Result<()> {
        self.state.log(format!(
            "🔄 VPN Tunnel loop started ({})",
            if self.hub { "hub" } else { "client" }
        ));

        // TUN reads block, so they live on their own thread.
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1024);
        let tun = self.tun.clone();
        let state = self.state.clone();
        std::thread::spawn(move || loop {
            if state.shutdown.load(Ordering::Relaxed) {
                break;
            }
            match tun.read_blocking() {
                Ok(pkt) if pkt.is_empty() => {}
                Ok(pkt) => {
                    if tx.blocking_send(pkt).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    if state.shutdown.load(Ordering::Relaxed) {
                        break;
                    }
                    state.log(format!("⚠️ TUN read error: {}", e));
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        });

        let mut udp_buf = vec![0u8; 65536];
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        let mut last_keepalive = Instant::now() - KEEPALIVE_EVERY;

        loop {
            tokio::select! {
                Some(pkt) = rx.recv() => self.handle_tun_packet(&pkt).await,
                res = self.socket.recv_from(&mut udp_buf) => match res {
                    Ok((n, from)) => {
                        let datagram = udp_buf[..n].to_vec();
                        self.handle_remote_packet(&datagram, from).await;
                    }
                    // Windows reports ICMP port-unreachable as a recv error
                    Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {}
                    Err(e) => {
                        self.state.log(format!("⚠️ UDP receive error: {}", e));
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                },
                _ = tick.tick() => {
                    if self.state.shutdown.load(Ordering::Relaxed) {
                        self.state.log("🛑 VPN Tunnel stopped".into());
                        return Ok(());
                    }
                    let now = Instant::now();
                    if now.duration_since(last_keepalive) >= KEEPALIVE_EVERY {
                        last_keepalive = now;
                        self.send_keepalives().await;
                    }
                    self.expire_peers(now);
                }
            }
        }
    }

    async fn send_keepalives(&self) {
        let ka = keepalive_packet(self.our_ip);
        let targets: Vec<SocketAddr> = self
            .seeds
            .iter()
            .copied()
            .chain(self.peers.keys().copied().filter(|p| !self.seeds.contains(p)))
            .collect();
        for addr in targets {
            let _ = self.socket.send_to(&ka, addr).await;
        }
    }

    fn expire_peers(&mut self, now: Instant) {
        let dead: Vec<SocketAddr> = self
            .peers
            .iter()
            .filter(|(a, t)| !self.seeds.contains(a) && now.duration_since(**t) > PEER_TIMEOUT)
            .map(|(a, _)| *a)
            .collect();
        for addr in dead {
            self.peers.remove(&addr);
            self.vip_map.retain(|_, a| *a != addr);
            self.state.log(format!("👋 Peer {} timed out", addr));
        }
    }

    /// Packet from the local OS (the game): send to the right peer(s).
    async fn handle_tun_packet(&mut self, packet: &[u8]) {
        let Some(ipv4) = packet_handler::get_ipv4_payload(packet) else {
            return; // not IPv4 (e.g. IPv6) - ignore
        };
        if ipv4.len() < 20 {
            return;
        }
        let dst = Ipv4Addr::new(ipv4[16], ipv4[17], ipv4[18], ipv4[19]);
        let datagram = encapsulate(ipv4);

        let is_bcast = packet_handler::is_broadcast_or_multicast(ipv4);
        if !is_bcast {
            if let Some(addr) = self.vip_map.get(&dst) {
                let _ = self.socket.send_to(&datagram, *addr).await;
                return;
            }
        }
        // Broadcast, or unicast to an unknown virtual IP: everyone we know.
        self.send_to_all(&datagram, None).await;
    }

    /// Datagram from the network.
    async fn handle_remote_packet(&mut self, datagram: &[u8], from: SocketAddr) {
        let Some((kind, payload)) = parse(datagram) else {
            return;
        };

        if self.peers.insert(from, Instant::now()).is_none() {
            self.state.log(format!("🤝 Peer connected: {}", from));
        }

        if kind == VPN_TYPE_KEEPALIVE {
            if payload.len() >= 4 {
                let vip = Ipv4Addr::new(payload[0], payload[1], payload[2], payload[3]);
                if vip.octets()[..3] == [10, 0, 0]
                    && vip != self.our_ip
                    && self.vip_map.insert(vip, from) != Some(from)
                {
                    self.state.log(format!("📍 {} is at {}", vip, from));
                }
            }
            return;
        }
        if kind != VPN_TYPE_DATA || payload.len() < 20 || payload[0] >> 4 != 4 {
            return;
        }

        let src = Ipv4Addr::new(payload[12], payload[13], payload[14], payload[15]);
        let dst = Ipv4Addr::new(payload[16], payload[17], payload[18], payload[19]);

        // Only accept sources from the virtual network, never our own address
        if src.octets()[..3] != [10, 0, 0] || src == self.our_ip {
            return;
        }
        if self.vip_map.insert(src, from) != Some(from) {
            self.state.log(format!("📍 {} is at {}", src, from));
        }

        let bcast = packet_handler::is_broadcast_or_multicast(payload);

        if dst == self.our_ip || bcast {
            if let Err(e) = self.tun.write_blocking(payload) {
                self.state.log(format!("⚠️ Failed to write to TUN: {}", e));
            }
        }

        if self.hub && dst != self.our_ip {
            if bcast {
                self.send_to_all(datagram, Some(from)).await;
            } else if let Some(addr) = self.vip_map.get(&dst).copied() {
                if addr != from {
                    let _ = self.socket.send_to(datagram, addr).await;
                }
            } else {
                // Unknown destination: flood like a switch
                self.send_to_all(datagram, Some(from)).await;
            }
        }
    }

    /// Send a datagram to every seed and known peer, except `skip`.
    async fn send_to_all(&self, datagram: &[u8], skip: Option<SocketAddr>) {
        let targets: Vec<SocketAddr> = self
            .seeds
            .iter()
            .copied()
            .chain(self.peers.keys().copied())
            .filter(|a| Some(*a) != skip)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        for addr in targets {
            if let Err(e) = self.socket.send_to(datagram, addr).await {
                self.state.log(format!("⚠️ Send to {} failed: {}", addr, e));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encapsulation_roundtrip() {
        let mut ipv4 = vec![0u8; 40];
        ipv4[0] = 0x45; // version 4, IHL 5
        ipv4[16..20].copy_from_slice(&[10, 0, 0, 255]); // subnet broadcast

        let datagram = encapsulate(&ipv4);
        assert_eq!(datagram[0..3], VPN_MAGIC);
        assert_eq!(datagram[3], VPN_VERSION);
        assert_eq!(datagram[4], VPN_TYPE_DATA);

        let payload = decapsulate(&datagram).expect("should decapsulate");
        assert_eq!(payload, &ipv4[..]);
    }

    #[test]
    fn test_decapsulate_rejects_garbage() {
        assert!(decapsulate(b"HELLO:peer1").is_none());
        assert!(decapsulate(b"PING:1234").is_none());
        assert!(decapsulate(&[]).is_none());
        // Wrong version / type
        let bad = [0x4F, 0x58, 0x44, 0x02, 0x01, 0x00];
        assert!(decapsulate(&bad).is_none());
        let bad_type = [0x4F, 0x58, 0x44, 0x01, 0x02, 0x00];
        assert!(decapsulate(&bad_type).is_none());
    }
}
