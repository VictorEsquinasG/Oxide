/// VPN Tunnel integration - bridges TUN device with UDP network
/// Handles packet routing between TUN interface and remote peer
/// Implements the core VPN functionality

use crate::app::AppState;
use crate::system::tun_device::TunDevice;
use crate::network::packet_handler;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::net::UdpSocket;
use pnet::util::MacAddr;
use std::net::Ipv4Addr;
use anyhow::Result;

/// VPN Tunnel handler - bridges TUN and UDP
pub struct VpnTunnel {
    tun: TunDevice,
    socket: Arc<UdpSocket>,
    state: Arc<AppState>,
    our_mac: MacAddr,
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
        // Generate a MAC address for the TUN interface
        let our_mac = MacAddr(0x02, 0x42, 0xac, 0x11, 0x00, 0x01);
        let our_ipv4 = our_ip.parse::<Ipv4Addr>()?;

        state.log(format!("🌐 VPN Tunnel created: IP={}, MAC={}", our_ip, our_mac));

        Ok(VpnTunnel {
            tun,
            socket,
            state,
            our_mac,
            our_ip: our_ipv4,
        })
    }

    /// Main tunnel loop - handles bidirectional packet routing
    /// Reads from TUN, sends to peer; reads from UDP, writes to TUN
    pub async fn run(&mut self) -> Result<()> {
        self.state.log("🔄 VPN Tunnel loop started".into());

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

            // Yield to allow other tasks to run
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        }

        Ok(())
    }

    /// Handle packet from TUN device
    /// This is traffic from local applications that should go to the peer
    async fn handle_tun_packet(&mut self, packet: &[u8]) {
        // Check if it's an ARP request
        if let Some(arp_response) = packet_handler::handle_arp_request(packet, self.our_mac, &self.our_ip) {
            // Send ARP response back through TUN
            if let Err(e) = self.tun.write(&arp_response).await {
                self.state.log(format!("❌ Failed to send ARP response: {}", e));
            }
            return;
        }

        // Check if it's IPv4 data packet
        if packet_handler::is_ipv4_packet(packet) {
            // Extract IPv4 and forward to peer
            if let Some(ipv4_data) = packet_handler::get_ipv4_payload(packet) {
                // TODO: Create VPN data packet with IPv4 payload
                // For now, just log
                if let (Some(src), Some(dst)) = (
                    packet_handler::get_source_ip(&ipv4_data),
                    packet_handler::get_dest_ip(&ipv4_data),
                ) {
                    self.state.log(format!(
                        "📤 TUN packet: {} → {} ({} bytes)",
                        src, dst, packet.len()
                    ));
                }
            }
        }
    }

    /// Handle packet from remote peer
    /// This is traffic that should be written to TUN device
    pub async fn handle_remote_packet(&mut self, packet: &[u8]) -> Result<()> {
        // TODO: Parse VPN packet and extract IPv4 payload
        // Then write to TUN device
        
        self.state.log(format!(
            "📥 Remote packet received ({} bytes)",
            packet.len()
        ));

        // For now, just log
        Ok(())
    }
}
