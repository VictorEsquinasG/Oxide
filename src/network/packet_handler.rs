//! Packet handler module - ARP and IPv4 packet manipulation
//! 
//! Provides utilities for parsing and generating ARP replies in the VPN tunnel.
//! This enables LAN emulation by automatically responding to ARP requests.
//!
//! Note: This module is only compiled when the 'pnet' feature is enabled,
//! which requires Npcap SDK (Windows) or equivalent packet libraries.

#[cfg(feature = "pnet")]
use pnet::packet::arp::{ArpOperations, ArpPacket, MutableArpPacket};
#[cfg(feature = "pnet")]
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
#[cfg(feature = "pnet")]
use pnet::packet::Packet;
#[cfg(feature = "pnet")]
use pnet::util::MacAddr;
#[cfg(feature = "pnet")]
use std::net::Ipv4Addr;

/// Handle incoming ARP request packets
/// 
/// Automatically generates ARP reply for requests matching our IP address.
/// This enables the VPN to emulate a physical LAN interface.
/// 
/// # Arguments
/// * `packet` - Raw packet bytes from TUN device
/// * `our_mac` - MAC address to respond with
/// * `our_ip` - IPv4 address to respond with
/// 
/// # Returns
/// Some(reply_packet) if this is an ARP request for our IP, None otherwise
#[cfg(feature = "pnet")]
pub fn handle_arp_request(
    packet: &[u8],
    our_mac: MacAddr,
    our_ip: &Ipv4Addr,
) -> Option<Vec<u8>> {
    // Parse Ethernet frame
    let eth = EthernetPacket::new(packet)?;
    
    // Check if it's an ARP packet
    if eth.get_ethertype() != EtherTypes::Arp {
        return None;
    }

    // Parse ARP packet
    let arp = ArpPacket::new(eth.payload())?;
    
    // Check if it's an ARP request for our IP
    if arp.get_operation() != ArpOperations::Request {
        return None;
    }

    if arp.get_target_proto_addr() != *our_ip {
        return None;
    }

    // Build ARP reply
    let mut eth_response = vec![0u8; eth.packet().len()];
    let mut eth_reply = MutableEthernetPacket::new(&mut eth_response)?;
    
    eth_reply.set_source(our_mac);
    eth_reply.set_destination(eth.get_source());
    eth_reply.set_ethertype(EtherTypes::Arp);

    // Build ARP reply payload
    let mut arp_response = vec![0u8; arp.packet().len()];
    let mut arp_reply = MutableArpPacket::new(&mut arp_response)?;
    
    arp_reply.set_hardware_type(arp.get_hardware_type());
    arp_reply.set_protocol_type(arp.get_protocol_type());
    arp_reply.set_hw_addr_len(arp.get_hw_addr_len());
    arp_reply.set_proto_addr_len(arp.get_proto_addr_len());
    arp_reply.set_operation(ArpOperations::Reply);
    arp_reply.set_sender_hw_addr(our_mac);
    arp_reply.set_sender_proto_addr(*our_ip);
    arp_reply.set_target_hw_addr(arp.get_sender_hw_addr());
    arp_reply.set_target_proto_addr(arp.get_sender_proto_addr());

    eth_reply.set_payload(&arp_response);
    
    Some(eth_response)
}
