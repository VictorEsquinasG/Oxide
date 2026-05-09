//! Network module - P2P UDP communication layer
//! Handles peer-to-peer networking, packet exchange, and NAT traversal
//!
//! This module is platform-independent and only deals with UDP sockets.
//! It does NOT handle TUN device integration or OS-specific networking.

pub mod mesh_controller;
pub mod nat_traversal;
pub mod node;
pub mod p2p_network;
pub mod packet_handler;
pub mod peer_connection;
pub mod vpn_tunnel;

// Re-export main types
#[allow(unused_imports)]
pub use mesh_controller::{MeshController, MeshStatus, PeerMetrics};
#[allow(unused_imports)]
pub use nat_traversal::{NatInfo, NatTraversal, NatType};
#[allow(unused_imports)]
pub use p2p_network::P2PNetwork;
#[allow(unused_imports)]
pub use peer_connection::{ConnectionMetrics, ConnectionQuality, PeerConnection};
