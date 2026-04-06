//! Network module - P2P UDP communication layer
//! Handles peer-to-peer networking, packet exchange, and NAT traversal
//! 
//! This module is platform-independent and only deals with UDP sockets.
//! It does NOT handle TUN device integration or OS-specific networking.

pub mod node;
pub mod packet_handler;
pub mod p2p_network;
pub mod peer_connection;
pub mod nat_traversal;
pub mod mesh_controller;

// Re-export main types
#[allow(unused_imports)]
pub use p2p_network::P2PNetwork;
#[allow(unused_imports)]
pub use peer_connection::{PeerConnection, ConnectionMetrics, ConnectionQuality};
#[allow(unused_imports)]
pub use nat_traversal::{NatTraversal, NatType, NatInfo};
#[allow(unused_imports)]
pub use mesh_controller::{MeshController, MeshStatus, PeerMetrics};
