use alloc::vec::Vec;
use core::sync::atomic::Ordering;
use crate::net::{htons, ntohs, htonl, ntohl, calculate_checksum};

pub const IP_PROTO_ICMP: u8 = 1;
pub const IP_PROTO_TCP: u8 = 6;
pub const IP_PROTO_UDP: u8 = 17;

#[repr(C, packed)]
pub struct Ipv4Header {
    pub version_ihl: u8,
    pub tos: u8,
    pub total_length: u16,
    pub identification: u16,
    pub flags_fragment_offset: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub header_checksum: u16,
    pub src_ip: u32,
    pub dest_ip: u32,
}

pub fn handle_ipv4_packet(payload: &[u8]) {
    if payload.len() < core::mem::size_of::<Ipv4Header>() {
        return; // Too short
    }
    
    let header = unsafe { core::ptr::read_unaligned(payload.as_ptr() as *const Ipv4Header) };
    
    let version = header.version_ihl >> 4;
    let ihl = header.version_ihl & 0x0F;
    
    if version != 4 || ihl < 5 {
        return; // Not IPv4 or bad IHL
    }
    
    let header_len = (ihl as usize) * 4;
    if payload.len() < header_len {
        return;
    }
    
    let total_len = ntohs(header.total_length) as usize;
    if payload.len() < total_len {
        return; // Truncated
    }
    
    // Validate checksum
    let calc_checksum = calculate_checksum(&payload[0..header_len]);
    if calc_checksum != 0 {
        return; // Bad checksum
    }
    
    let local_ip = crate::net::LOCAL_IP.load(Ordering::Relaxed);
    let dest_ip = ntohl(header.dest_ip);
    
    if dest_ip != local_ip && dest_ip != 0xFFFFFFFF && local_ip != 0 {
        return; // Not for us
    }
    
    let data = &payload[header_len..total_len];
    let src_ip = ntohl(header.src_ip);
    
    match header.protocol {
        IP_PROTO_ICMP => {
            crate::net::icmp::handle_icmp_packet(data, src_ip);
        }
        IP_PROTO_UDP => {
            crate::net::udp::handle_udp_packet(src_ip, dest_ip, data);
        }
        IP_PROTO_TCP => {
            crate::net::tcp::handle_tcp_packet(src_ip, data);
        }
        _ => {
            // Unhandled protocol
            // Ignore unknown protocols
        }
    }
}

pub fn send_ipv4_packet(dest_ip: u32, protocol: u8, payload: &[u8]) -> Result<(), &'static str> {
    let local_ip = crate::net::LOCAL_IP.load(Ordering::Relaxed);
    let netmask = crate::net::NETMASK.load(Ordering::Relaxed);
    let gateway = crate::net::GATEWAY.load(Ordering::Relaxed);
    
    // Routing logic
    let target_ip = if dest_ip == 0xFFFFFFFF || (dest_ip & netmask) == (local_ip & netmask) {
        dest_ip // Broadcast or Same subnet
    } else {
        gateway // Different subnet, route to gateway
    };
    
    let dest_mac = match crate::net::arp::arp_resolve(target_ip) {
        Some(mac) => mac,
        None => {
            return Err("ARP resolving... retry later");
        }
    };
    
    let mut packet = Vec::with_capacity(core::mem::size_of::<Ipv4Header>() + payload.len());
    
    let total_length = (core::mem::size_of::<Ipv4Header>() + payload.len()) as u16;
    static NEXT_ID: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(1);
    let pkt_id = NEXT_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    
    let mut header = Ipv4Header {
        version_ihl: (4 << 4) | 5,
        tos: 0,
        total_length: htons(total_length),
        identification: htons(pkt_id),
        flags_fragment_offset: htons(0x4000), // DF bit = 1
        ttl: 64,
        protocol,
        header_checksum: 0,
        src_ip: htonl(local_ip),
        dest_ip: htonl(dest_ip),
    };
    
    let header_bytes = unsafe {
        core::slice::from_raw_parts(
            &header as *const Ipv4Header as *const u8,
            core::mem::size_of::<Ipv4Header>()
        )
    };
    
    let checksum = calculate_checksum(header_bytes);
    header.header_checksum = htons(checksum);
    
    let header_bytes = unsafe {
        core::slice::from_raw_parts(
            &header as *const Ipv4Header as *const u8,
            core::mem::size_of::<Ipv4Header>()
        )
    };
    
    packet.extend_from_slice(header_bytes);
    packet.extend_from_slice(payload);
    
    crate::net::ethernet::send_ethernet_frame(dest_mac, crate::net::ethernet::ETHERTYPE_IPv4, &packet)
}
