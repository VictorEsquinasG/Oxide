//! Network packet structures and serialization
//!
//! Defines the packet format used for P2P communication between peers.
//! Uses bincode for efficient binary serialization over UDP.

use serde::{Deserialize, Serialize};

/// Control message types for peer-to-peer communication
#[derive(Serialize, Deserialize, Debug)]
pub enum ControlMessage {
    /// Initial connection request
    Hello,
    /// Acknowledgment of connection request
    HelloAck,
    /// Keep-alive ping request
    Ping,
    /// Keep-alive ping response
    Pong,
}

/// Packet payload enum - either control message or raw data
#[derive(Serialize, Deserialize, Debug)]
pub enum PacketPayload {
    /// Control messages for connection management
    Control(ControlMessage),
    /// Raw data (ethernet frames from TUN device)
    Data(Vec<u8>),
}

/// Network packet structure for P2P communication
///
/// Every packet exchanged between peers includes:
/// - Unique ID for tracking
/// - Protocol ID to filter out non-VPN traffic
/// - Payload (control message or data)
#[derive(Serialize, Deserialize, Debug)]
pub struct Packet {
    /// Unique packet identifier (random)
    pub id: u64,
    /// Protocol magic number (0xDEADBEEF) - filters garbage traffic
    pub protocol_id: u32,
    /// Packet content (control or data)
    pub payload: PacketPayload,
}

impl Packet {
    /// Create a packet with custom payload
    pub fn new(payload: PacketPayload) -> Self {
        Self {
            id: rand::random(),
            protocol_id: 0xDEADBEEF,
            payload,
        }
    }

    /// Serialize packet to binary format (bincode)
    pub fn encode(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    /// Deserialize packet from binary format
    pub fn decode(data: &[u8]) -> anyhow::Result<Self> {
        Ok(bincode::deserialize(data)?)
    }

    /// Factory method: Create a PING keep-alive packet
    pub fn ping() -> Self {
        Self {
            id: rand::random(),
            protocol_id: 0xDEADBEEF,
            payload: PacketPayload::Control(ControlMessage::Ping),
        }
    }

    /// Factory method: Create a PONG response packet
    pub fn pong() -> Self {
        Self {
            id: rand::random(),
            protocol_id: 0xDEADBEEF,
            payload: PacketPayload::Control(ControlMessage::Pong),
        }
    }

    /// Factory method: Create a data packet from raw bytes
    pub fn data(bytes: Vec<u8>) -> Self {
        Self {
            id: rand::random(),
            protocol_id: 0xDEADBEEF,
            payload: PacketPayload::Data(bytes),
        }
    }

    /// Factory method: Create a HELLO connection request packet
    pub fn hello() -> Self {
        Self {
            id: rand::random(),
            protocol_id: 0xDEADBEEF,
            payload: PacketPayload::Control(ControlMessage::Hello),
        }
    }

    /// Factory method: Create a HELLO_ACK response packet
    pub fn hello_ack() -> Self {
        Self {
            id: rand::random(),
            protocol_id: 0xDEADBEEF,
            payload: PacketPayload::Control(ControlMessage::HelloAck),
        }
    }
}
