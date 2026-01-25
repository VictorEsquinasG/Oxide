//! Network module - P2P UDP communication layer
//! Handles peer-to-peer networking, packet exchange, and NAT traversal
//! 
//! This module is platform-independent and only deals with UDP sockets.
//! It does NOT handle TUN device integration or OS-specific networking.

pub mod node;
pub mod packet_handler;
