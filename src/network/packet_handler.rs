/// Simple packet handling for VPN
/// Supports ARP and basic IPv4 packet manipulation
/// Used to handle LAN emulation without full TCP/IP stack

use pnet::packet::arp::{ArpOperations, ArpPacket, MutableArpPacket};
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::packet::ipv4::{Ipv4Packet};
use pnet::packet::Packet;
use pnet::util::MacAddr;
use std::net::Ipv4Addr;

/// Handle incoming ARP packets
/// Automatically responds to ARP requests
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

/// Check if packet is an IPv4 data packet (not ARP/control)
pub fn is_ipv4_packet(packet: &[u8]) -> bool {
    let eth = match EthernetPacket::new(packet) {
        Some(p) => p,
        None => return false,
    };
    
    eth.get_ethertype() == EtherTypes::Ipv4
}

/// Extract IPv4 packet from Ethernet frame
pub fn get_ipv4_payload(packet: &[u8]) -> Option<Vec<u8>> {
    let eth = EthernetPacket::new(packet)?;
    
    if eth.get_ethertype() != EtherTypes::Ipv4 {
        return None;
    }
    
    Some(eth.payload().to_vec())
}

/// Get source IP from IPv4 packet
pub fn get_source_ip(ipv4_packet: &[u8]) -> Option<Ipv4Addr> {
    let packet = Ipv4Packet::new(ipv4_packet)?;
    Some(packet.get_source())
}

/// Get destination IP from IPv4 packet
pub fn get_dest_ip(ipv4_packet: &[u8]) -> Option<Ipv4Addr> {
    let packet = Ipv4Packet::new(ipv4_packet)?;
    Some(packet.get_destination())
}
