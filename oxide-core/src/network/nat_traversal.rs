// NAT Traversal Module
// Implements hole punching and NAT detection for P2P connectivity

use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;

/// NAT type detection result
#[derive(Debug, Clone, PartialEq)]
pub enum NatType {
    /// No NAT - directly accessible
    Public,
    /// Full cone NAT - port reuses for all destinations
    FullCone,
    /// Address restricted cone NAT - port reuses but restricted by address
    AddressRestricted,
    /// Port restricted cone NAT - requires same port and address
    PortRestricted,
    /// Symmetric NAT - different port for different destinations
    Symmetric,
    /// Unknown NAT type
    Unknown,
}

impl std::fmt::Display for NatType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NatType::Public => write!(f, "Public (No NAT)"),
            NatType::FullCone => write!(f, "Full Cone NAT"),
            NatType::AddressRestricted => write!(f, "Address Restricted NAT"),
            NatType::PortRestricted => write!(f, "Port Restricted NAT"),
            NatType::Symmetric => write!(f, "Symmetric NAT"),
            NatType::Unknown => write!(f, "Unknown NAT"),
        }
    }
}

/// Result of NAT detection
#[derive(Debug, Clone)]
pub struct NatInfo {
    pub nat_type: NatType,
    pub external_ip: Option<String>,
    pub external_port: Option<u16>,
    pub internal_ip: String,
    pub internal_port: u16,
}

/// NAT Traversal Handler
pub struct NatTraversal {
    // STUN server to detect external IP
    stun_servers: Vec<&'static str>,
}

impl NatTraversal {
    /// Create new NAT traversal handler
    pub fn new() -> Self {
        Self {
            stun_servers: vec![
                "stun.l.google.com:19302",
                "stun1.l.google.com:19302",
                "stun2.l.google.com:19302",
            ],
        }
    }

    /// Detect NAT type using STUN protocol
    pub async fn detect_nat(&self, bind_addr: SocketAddr) -> Result<NatInfo, String> {
        let socket = UdpSocket::bind(bind_addr)
            .await
            .map_err(|e| format!("Failed to bind socket: {}", e))?;

        let local_addr = socket
            .local_addr()
            .map_err(|e| format!("Failed to get local address: {}", e))?;

        // Try STUN detection
        match self.stun_detect(&socket).await {
            Ok((external_ip, external_port)) => {
                Ok(NatInfo {
                    nat_type: NatType::FullCone, // Simplified - assume full cone if we can detect external IP
                    external_ip: Some(external_ip),
                    external_port: Some(external_port),
                    internal_ip: local_addr.ip().to_string(),
                    internal_port: local_addr.port(),
                })
            }
            Err(_) => {
                // STUN failed, assume we're behind restrictive NAT or no NAT
                Ok(NatInfo {
                    nat_type: NatType::Symmetric,
                    external_ip: None,
                    external_port: None,
                    internal_ip: local_addr.ip().to_string(),
                    internal_port: local_addr.port(),
                })
            }
        }
    }

    /// Perform STUN detection to get external IP
    async fn stun_detect(&self, socket: &UdpSocket) -> Result<(String, u16), String> {
        // Try each STUN server
        for stun_server in &self.stun_servers {
            match self.query_stun_server(socket, stun_server).await {
                Ok(result) => return Ok(result),
                Err(_) => continue, // Try next server
            }
        }
        Err("All STUN servers failed".to_string())
    }

    /// Query a single STUN server
    async fn query_stun_server(
        &self,
        socket: &UdpSocket,
        stun_server: &str,
    ) -> Result<(String, u16), String> {
        // Send STUN binding request
        let stun_request = create_stun_binding_request();

        socket
            .send_to(&stun_request, stun_server)
            .await
            .map_err(|e| format!("Send failed: {}", e))?;

        // Wait for response
        let mut buffer = [0u8; 1024];
        match timeout(Duration::from_secs(2), socket.recv(&mut buffer)).await {
            Ok(Ok(n)) => {
                // Parse STUN response
                parse_stun_response(&buffer[..n])
            }
            Ok(Err(e)) => Err(format!("Receive error: {}", e)),
            Err(_) => Err("STUN timeout".to_string()),
        }
    }

    /// Attempt hole punching to a peer
    pub async fn punch_hole(&self, peer_addr: SocketAddr, attempts: u32) -> Result<(), String> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| format!("Bind failed: {}", e))?;

        // Send multiple packets to punch through NAT
        for i in 0..attempts {
            let msg = format!("PUNCH:{}", i);
            socket
                .send_to(msg.as_bytes(), peer_addr)
                .await
                .map_err(|e| format!("Send failed: {}", e))?;

            // Small delay between attempts
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        Ok(())
    }
}

/// Create a STUN binding request (simplified)
fn create_stun_binding_request() -> Vec<u8> {
    // Minimal STUN binding request (RFC 5389)
    vec![
        0x00, 0x01, // Message Type: Binding Request
        0x00, 0x00, // Message Length: 0
        // Magic Cookie
        0x21, 0x12, 0xa4, 0x42, // Transaction ID (96 bits, random)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]
}

/// Parse a STUN binding response (simplified)
fn parse_stun_response(data: &[u8]) -> Result<(String, u16), String> {
    if data.len() < 20 {
        return Err("Response too short".to_string());
    }

    // Look for XOR-MAPPED-ADDRESS attribute (type 0x0020)
    let mut i = 20; // Skip header

    while i < data.len() {
        if i + 4 > data.len() {
            break;
        }

        let attr_type = u16::from_be_bytes([data[i], data[i + 1]]);
        let attr_len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;

        if attr_type == 0x0020 && attr_len >= 8 {
            // XOR-MAPPED-ADDRESS
            // Skip reserved byte and address family
            let port_bytes = [data[i + 6], data[i + 7]];
            let port = u16::from_be_bytes(port_bytes) ^ 0x2112; // XOR with magic cookie

            // IP address (simplified - assume IPv4)
            if data.len() >= i + 8 {
                let ip_bytes = [data[i + 4], data[i + 5], data[i + 8], data[i + 9]];
                let ip = Ipv4Addr::new(
                    ip_bytes[0] ^ 0x21,
                    ip_bytes[1] ^ 0x12,
                    ip_bytes[2] ^ 0xa4,
                    ip_bytes[3] ^ 0x42,
                );
                return Ok((ip.to_string(), port));
            }
        }

        i += 4 + attr_len;
    }

    Err("No XOR-MAPPED-ADDRESS in response".to_string())
}

/// Attempt direct connection between two peers
pub async fn attempt_direct_connection(
    my_addr: SocketAddr,
    peer_addr: SocketAddr,
    timeout_secs: u64,
) -> Result<(), String> {
    let socket = UdpSocket::bind(my_addr)
        .await
        .map_err(|e| format!("Bind failed: {}", e))?;

    // Send initial packet
    let hello = b"HELLO_DIRECT";
    socket
        .send_to(hello, peer_addr)
        .await
        .map_err(|e| format!("Send failed: {}", e))?;

    // Try to receive response
    let mut buffer = [0u8; 1024];
    match timeout(Duration::from_secs(timeout_secs), socket.recv(&mut buffer)).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(format!("Receive failed: {}", e)),
        Err(_) => Err(format!("Connection timeout after {} seconds", timeout_secs)),
    }
}

/// Check if two peers can communicate directly
pub async fn check_peer_reachability(from_addr: SocketAddr, to_addr: SocketAddr) -> bool {
    match attempt_direct_connection(from_addr, to_addr, 5).await {
        Ok(_) => true,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nat_type_display() {
        assert_eq!(NatType::Public.to_string(), "Public (No NAT)");
        assert_eq!(NatType::FullCone.to_string(), "Full Cone NAT");
    }

    #[test]
    fn test_stun_request_creation() {
        let req = create_stun_binding_request();
        assert_eq!(req.len(), 20);
        assert_eq!(req[0], 0x00);
        assert_eq!(req[1], 0x01);
    }
}
