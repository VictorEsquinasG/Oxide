//! Packet handler module - ARP and IPv4 packet manipulation
//!
//! Provides utilities for parsing and generating ARP replies in the VPN tunnel.
//! This enables LAN emulation by automatically responding to ARP requests.
//!
//! This module is dependency-free: it parses raw Ethernet/IPv4 bytes directly,
//! so it works on every platform without the Npcap SDK.

/// Check if a raw TUN packet is an Ethernet frame with ARP ethertype.
/// TUN devices in L3 mode deliver raw IP packets (first nibble 0x4 = IPv4),
/// while TAP/L2 devices deliver Ethernet frames (14-byte header).
pub fn is_arp_frame(packet: &[u8]) -> bool {
    // Ethernet: dst MAC (6) + src MAC (6) + ethertype (2)
    packet.len() >= 14 && packet[12..14] == [0x08, 0x06]
}

/// Check if a raw TUN packet is an Ethernet frame with IPv4 ethertype.
pub fn is_ipv4_frame(packet: &[u8]) -> bool {
    packet.len() >= 14 && packet[12..14] == [0x08, 0x00]
}

/// Check if a raw packet is a bare IPv4 packet (L3 TUN mode).
pub fn is_bare_ipv4_packet(packet: &[u8]) -> bool {
    packet.len() >= 20 && packet[0] >> 4 == 4
}

/// Check if a packet (Ethernet frame or bare IPv4) carries IPv4 traffic.
pub fn is_ipv4_packet(packet: &[u8]) -> bool {
    is_ipv4_frame(packet) || is_bare_ipv4_packet(packet)
}

/// Extract the IPv4 packet bytes from a raw TUN packet.
/// Handles both Ethernet-framed (L2) and bare (L3) input.
pub fn get_ipv4_payload(packet: &[u8]) -> Option<&[u8]> {
    if is_ipv4_frame(packet) {
        Some(&packet[14..])
    } else if is_bare_ipv4_packet(packet) {
        Some(packet)
    } else {
        None
    }
}

/// Get source IP from an IPv4 packet as string.
pub fn get_source_ip(ipv4_packet: &[u8]) -> Option<String> {
    if ipv4_packet.len() < 20 || ipv4_packet[0] >> 4 != 4 {
        return None;
    }
    let b = &ipv4_packet[12..16];
    Some(format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]))
}

/// Get destination IP from an IPv4 packet as string.
pub fn get_dest_ip(ipv4_packet: &[u8]) -> Option<String> {
    if ipv4_packet.len() < 20 || ipv4_packet[0] >> 4 != 4 {
        return None;
    }
    let b = &ipv4_packet[16..20];
    Some(format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]))
}

/// Check whether the IPv4 packet is addressed to a broadcast or multicast
/// destination (255.255.255.255, 224.0.0.0/4, or the subnet broadcast).
/// This is the traffic old LAN games (Command & Conquer, etc.) use to
/// discover games on the network.
pub fn is_broadcast_or_multicast(packet: &[u8]) -> bool {
    match get_dest_bytes(packet) {
        Some(dst) => is_broadcast_dest(dst) || is_subnet_broadcast(dst),
        None => false,
    }
}

/// Extract the 4-byte destination address of a raw TUN packet
/// (Ethernet frame or bare IPv4).
pub fn get_dest_bytes(packet: &[u8]) -> Option<[u8; 4]> {
    let ipv4 = get_ipv4_payload(packet)?;
    if ipv4.len() < 20 {
        return None;
    }
    Some([ipv4[16], ipv4[17], ipv4[18], ipv4[19]])
}

/// Check whether an IPv4 destination address is broadcast or multicast.
pub fn is_broadcast_dest(dst: [u8; 4]) -> bool {
    dst == [255, 255, 255, 255] || dst[0] & 0xf0 == 0xe0
}

/// Check whether an IPv4 destination is the /24 subnet broadcast
/// address (e.g. 10.0.0.255 for the 10.0.0.0/24 subnet).
pub fn is_subnet_broadcast(dst: [u8; 4]) -> bool {
    dst[0..3] == [10, 0, 0] && dst[3] == 255
}

/// Handle incoming ARP request packets (Ethernet-framed input only).
///
/// Automatically generates an ARP reply for requests matching our IP address.
/// This enables the VPN to emulate a physical LAN interface.
///
/// # Arguments
/// * `packet` - Raw packet bytes from TUN device (Ethernet frame)
/// * `our_mac` - MAC address to respond with
/// * `our_ip` - IPv4 address to respond with
///
/// # Returns
/// Some(reply_packet) if this is an ARP request for our IP, None otherwise
pub fn handle_arp_request(
    packet: &[u8],
    our_mac: [u8; 6],
    our_ip: &std::net::Ipv4Addr,
) -> Option<Vec<u8>> {
    if !is_arp_frame(packet) {
        return None;
    }

    // ARP payload starts after the 14-byte Ethernet header
    let arp = &packet[14..];
    // ARP: htype(2) ptype(2) hlen(1) plen(1) oper(2) sha(6) spa(4) tha(6) tpa(4)
    if arp.len() < 28 {
        return None;
    }

    // Must be an ARP request (operation 1) for our IPv4 address
    let operation = u16::from_be_bytes([arp[6], arp[7]]);
    if operation != 1 {
        return None;
    }

    let target_ip = [arp[24], arp[25], arp[26], arp[27]];
    if target_ip != our_ip.octets() {
        return None;
    }

    // Build the reply: swap Ethernet addresses, set ARP reply with our MAC/IP
    let mut reply = vec![0u8; 14 + 28];
    reply[0..6].copy_from_slice(&packet[6..12]); // dst = original src MAC
    reply[6..12].copy_from_slice(&our_mac); // src = our MAC
    reply[12..14].copy_from_slice(&[0x08, 0x06]); // ARP ethertype

    reply[14..16].copy_from_slice(&arp[0..2]); // hardware type
    reply[16..18].copy_from_slice(&arp[2..4]); // protocol type
    reply[18] = arp[4]; // hw addr len
    reply[19] = arp[5]; // proto addr len
    reply[20..22].copy_from_slice(&2u16.to_be_bytes()); // operation = reply
    reply[22..28].copy_from_slice(&our_mac); // sender hw addr
    reply[28..32].copy_from_slice(&our_ip.octets()); // sender proto addr
    reply[32..38].copy_from_slice(&packet[22..28]); // target hw = original sender MAC
    reply[38..42].copy_from_slice(&packet[28..32]); // target proto = original sender IP

    Some(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_arp_request(sender_mac: [u8; 6], sender_ip: [u8; 4], target_ip: [u8; 4]) -> Vec<u8> {
        let mut frame = vec![0u8; 42];
        frame[0..6].copy_from_slice(&[0xff; 6]); // broadcast dst
        frame[6..12].copy_from_slice(&sender_mac);
        frame[12..14].copy_from_slice(&[0x08, 0x06]); // ARP

        let arp = &mut frame[14..];
        arp[0..2].copy_from_slice(&1u16.to_be_bytes()); // ethernet
        arp[2..4].copy_from_slice(&0x0800u16.to_be_bytes()); // IPv4
        arp[4] = 6; // hw len
        arp[5] = 4; // proto len
        arp[6..8].copy_from_slice(&1u16.to_be_bytes()); // request
        arp[8..14].copy_from_slice(&sender_mac);
        arp[14..18].copy_from_slice(&sender_ip);
        arp[18..24].copy_from_slice(&[0; 6]); // target hw unknown
        arp[24..28].copy_from_slice(&target_ip);
        frame
    }

    #[test]
    fn test_arp_request_reply() {
        let our_mac = [0x02, 0x42, 0xac, 0x11, 0x00, 0x01];
        let our_ip = std::net::Ipv4Addr::new(10, 0, 0, 1);
        let packet = build_arp_request(
            [0xaa; 6],
            [10, 0, 0, 2],
            [10, 0, 0, 1],
        );

        let reply = handle_arp_request(&packet, our_mac, &our_ip).expect("should reply");
        assert_eq!(&reply[0..6], &[0xaa; 6]); // dst = original sender
        assert_eq!(&reply[6..12], &our_mac);
        assert_eq!(reply[12..14], [0x08, 0x06]);
        // ARP operation should be reply (2)
        assert_eq!(u16::from_be_bytes([reply[14 + 6], reply[14 + 7]]), 2);
    }

    #[test]
    fn test_arp_request_other_ip_ignored() {
        let our_ip = std::net::Ipv4Addr::new(10, 0, 0, 1);
        let packet = build_arp_request([0xaa; 6], [10, 0, 0, 2], [10, 0, 0, 99]);
        assert!(handle_arp_request(&packet, [0x02; 6], &our_ip).is_none());
    }

    #[test]
    fn test_ipv4_detection() {
        // Bare IPv4 packet
        let mut ipv4 = vec![0u8; 20];
        ipv4[0] = 0x45; // version 4, IHL 5
        assert!(is_bare_ipv4_packet(&ipv4));
        assert!(is_ipv4_packet(&ipv4));
        assert_eq!(get_ipv4_payload(&ipv4), Some(&ipv4[..]));

        // Ethernet-framed IPv4
        let mut framed = vec![0u8; 34];
        framed[12..14].copy_from_slice(&[0x08, 0x00]);
        framed[14] = 0x45;
        assert!(is_ipv4_frame(&framed));
        assert!(is_ipv4_packet(&framed));
        assert_eq!(get_ipv4_payload(&framed), Some(&framed[14..]));
    }

    #[test]
    fn test_broadcast_dest() {
        assert!(is_broadcast_dest([255, 255, 255, 255]));
        assert!(is_broadcast_dest([239, 255, 255, 250]));
        assert!(!is_broadcast_dest([10, 0, 0, 5]));
    }
}
