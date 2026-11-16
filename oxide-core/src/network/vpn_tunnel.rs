/// VPN Tunnel integration - bridges TUN device with UDP network
/// Handles packet routing between TUN interface and remote peers
/// Implements the core VPN functionality
use crate::app::AppState;
use crate::network::packet_handler;
use crate::system::tun_device::TunDevice;
use anyhow::Result;
use std::net::Ipv4Addr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::net::UdpSocket;

/// Wire format for VPN data packets sent over UDP.
///
/// ```text
/// [0..3]  magic  0x4F 0x58 0x44 ("OXD")
/// [3]     version (0x01)
/// [4]     packet type (0x01 = data)
/// [5..]   payload: raw IPv4 packet (L3 TUN mode)
/// ```
pub const VPN_MAGIC: [u8; 3] = [0x4F, 0x58, 0x44];
pub const VPN_VERSION: u8 = 0x01;
pub const VPN_TYPE_DATA: u8 = 0x01;

/// Minimum size of a valid VPN data packet (header + minimal IPv4 packet).
pub const VPN_HEADER_LEN: usize = 5;

/// Wrap a raw IPv4 packet into the VPN wire format.
pub fn encapsulate(ipv4_packet: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(VPN_HEADER_LEN + ipv4_packet.len());
    out.extend_from_slice(&VPN_MAGIC);
    out.push(VPN_VERSION);
    out.push(VPN_TYPE_DATA);
    out.extend_from_slice(ipv4_packet);
    out
}

/// Try to decapsulate a VPN data packet received over UDP.
/// Returns the raw IPv4 payload, or None if this is not a VPN data packet.
pub fn decapsulate(packet: &[u8]) -> Option<&[u8]> {
    if packet.len() < VPN_HEADER_LEN {
        return None;
    }
    if packet[0..3] != VPN_MAGIC || packet[3] != VPN_VERSION || packet[4] != VPN_TYPE_DATA {
        return None;
    }
    Some(&packet[VPN_HEADER_LEN..])
}

/// VPN Tunnel handler - bridges TUN and UDP
pub struct VpnTunnel {
    tun: TunDevice,
    /// UDP socket for sending/receiving packets
    socket: Arc<UdpSocket>,
    /// Addresses of the remote peers (real_ip:port)
    peer_addrs: Vec<std::net::SocketAddr>,
    state: Arc<AppState>,
    our_mac: [u8; 6],
    our_ip: Ipv4Addr,
}

impl VpnTunnel {
    /// Create a new VPN tunnel
    pub async fn new(
        tun: TunDevice,
        socket: Arc<UdpSocket>,
        state: Arc<AppState>,
        our_ip: &str,
    ) -> Result<Self> {
        Self::with_peers(tun, socket, state, our_ip, Vec::new()).await
    }

    /// Create a new VPN tunnel with a list of remote peer addresses.
    pub async fn with_peers(
        tun: TunDevice,
        socket: Arc<UdpSocket>,
        state: Arc<AppState>,
        our_ip: &str,
        peer_addrs: Vec<std::net::SocketAddr>,
    ) -> Result<Self> {
        // Generate a MAC address for the TUN interface
        let our_mac = [0x02, 0x42, 0xac, 0x11, 0x00, 0x01];
        let our_ipv4 = our_ip.parse::<Ipv4Addr>()?;

        state.log(format!(
            "🌐 VPN Tunnel created: IP={}, MAC={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            our_ip, our_mac[0], our_mac[1], our_mac[2], our_mac[3], our_mac[4], our_mac[5]
        ));

        Ok(VpnTunnel {
            tun,
            socket,
            peer_addrs,
            state,
            our_mac,
            our_ip: our_ipv4,
        })
    }

    /// Main tunnel loop - handles bidirectional packet routing
    /// Reads from TUN, sends to peers; reads from UDP, writes to TUN
    pub async fn run(&mut self) -> Result<()> {
        self.state.log("🔄 VPN Tunnel loop started".into());

        let mut udp_buf = vec![0u8; 65536];

        loop {
            // Check for shutdown
            if self.state.shutdown.load(Ordering::Relaxed) {
                self.state.log("🛑 VPN Tunnel stopped".into());
                break;
            }

            // Read from TUN device (non-blocking)
            match self.tun.read().await {
                Ok(packet) if !packet.is_empty() => {
                    self.handle_tun_packet(&packet).await;
                }
                Err(e) => {
                    self.state.log(format!("⚠️ TUN read error: {}", e));
                }
                _ => {}
            }

            // Drain pending UDP packets from remote peers
            loop {
                match self.socket.try_recv_from(&mut udp_buf) {
                    Ok((n, _sender)) => {
                        if let Err(e) = self.handle_remote_packet(&udp_buf[..n]).await {
                            self.state
                                .log(format!("⚠️ Failed to deliver remote packet: {}", e));
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => {
                        self.state.log(format!("⚠️ UDP receive error: {}", e));
                        break;
                    }
                }
            }

            // Yield to allow other tasks to run
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        }

        Ok(())
    }

    /// Handle packet from TUN device
    /// This is traffic from local applications that should go to the peers
    async fn handle_tun_packet(&mut self, packet: &[u8]) {
        // Check if it's an ARP request
        if let Some(arp_response) =
            packet_handler::handle_arp_request(packet, self.our_mac, &self.our_ip)
        {
            // Send ARP response back through TUN
            if let Err(e) = self.tun.write(&arp_response).await {
                self.state
                    .log(format!("❌ Failed to send ARP response: {}", e));
            }
            return;
        }

        // Check if it's IPv4 data packet
        if packet_handler::is_ipv4_packet(packet) {
            // Broadcast/multicast (LAN game discovery): fan out to every peer
            if packet_handler::is_broadcast_or_multicast(packet) {
                let n = self.send_to_all_peers(packet).await;
                if n == 0 && !self.peer_addrs.is_empty() {
                    self.state
                        .log("⚠️ Failed to send broadcast to any peer".into());
                }
                return;
            }

            // Unicast: forward to peers (mesh has no routing table yet, so
            // every peer receives it and drops packets not addressed to it)
            let _ = self.send_to_all_peers(packet).await;
        }
    }

    /// Handle packet from remote peer
    /// This is traffic that should be written to TUN device
    pub async fn handle_remote_packet(&mut self, packet: &[u8]) -> Result<()> {
        // Only accept well-formed VPN data packets
        let Some(ipv4_packet) = decapsulate(packet) else {
            return Ok(()); // Not a VPN data packet (HELLO/PING/etc.) - ignore
        };

        if ipv4_packet.is_empty() {
            return Ok(());
        }

        if let (Some(src), Some(dst)) = (
            packet_handler::get_source_ip(ipv4_packet),
            packet_handler::get_dest_ip(ipv4_packet),
        ) {
            self.state.log(format!(
                "📥 Remote packet: {} → {} ({} bytes)",
                src,
                dst,
                ipv4_packet.len()
            ));
        }

        // Write the IPv4 packet into the TUN device so the local OS
        // delivers it to the listening application (the game)
        self.tun.write(ipv4_packet).await?;

        Ok(())
    }

    /// Send a raw IPv4 packet (already encapsulated) to every known peer.
    /// Returns the number of peers it was successfully sent to.
    async fn send_to_all_peers(&self, ipv4_packet: &[u8]) -> usize {
        if self.peer_addrs.is_empty() {
            return 0;
        }

        let datagram = encapsulate(ipv4_packet);
        let mut sent = 0;

        for peer_addr in &self.peer_addrs {
            match self.socket.send_to(&datagram, *peer_addr).await {
                Ok(_) => sent += 1,
                Err(e) => {
                    self.state
                        .log(format!("⚠️ Send to {} failed: {}", peer_addr, e));
                }
            }
        }

        sent
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
