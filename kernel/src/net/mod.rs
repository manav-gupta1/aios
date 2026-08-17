pub mod ethernet;
pub mod arp;
pub mod ipv4;
pub mod icmp;
pub mod tcp;
pub mod udp;
pub mod socket;
pub mod dhcp;
pub mod dns;

use core::sync::atomic::{AtomicU32, Ordering};
use crate::drivers::network::NETWORK_DEVICE;

// Static configuration for QEMU user networking
pub static LOCAL_IP: AtomicU32 = AtomicU32::new(0);
pub static NETMASK: AtomicU32 = AtomicU32::new(0);
pub static GATEWAY: AtomicU32 = AtomicU32::new(0);
pub static DNS_SERVER: AtomicU32 = AtomicU32::new(0);

// Helper to construct an IPv4 integer from 4 bytes (Big Endian)
pub const fn ip(a: u8, b: u8, c: u8, d: u8) -> u32 {
    ((a as u32) << 24) | ((b as u32) << 16) | ((c as u32) << 8) | (d as u32)
}

pub fn init() {
    // 10.0.2.15
    LOCAL_IP.store(ip(10, 0, 2, 15), Ordering::SeqCst);
    // 255.255.255.0
    NETMASK.store(ip(255, 255, 255, 0), Ordering::SeqCst);
    // 10.0.2.2
    GATEWAY.store(ip(10, 0, 2, 2), Ordering::SeqCst);
    // 10.0.2.3 (QEMU SLIRP DNS)
    DNS_SERVER.store(ip(10, 0, 2, 3), Ordering::SeqCst);
    
    arp::init();
    
    crate::drivers::storage::serial_print("[NET] IP Layer starting (using static IP for test)...\n");
    // dhcp::run_dhcp_client() currently blocks indefinitely in QEMU user net.
    // if let Err(_) = dhcp::run_dhcp_client() {
    crate::drivers::storage::serial_print("[NET] DHCP skipped, using static configuration (10.0.2.15)\n");
    // } else {
    //     crate::drivers::storage::serial_print("[NET] DHCP successful\n");
    // }
}

pub fn handle_packet(packet: &[u8]) {
    // Route packet to Ethernet layer
    ethernet::handle_ethernet_frame(packet);
}

pub fn handle_network_interrupt() {
    let mut packets = alloc::vec::Vec::new();
    {
        let mut net_dev = NETWORK_DEVICE.lock();
        if let Some(dev) = net_dev.as_mut() {
            dev.ack_interrupt();
            while let Some(pkt) = dev.receive_packet() {
                packets.push(pkt);
            }
        }
    }
    
    for pkt in packets {
        handle_packet(&pkt);
    }
}

// Convert u16 to Big Endian bytes
pub fn htons(v: u16) -> u16 {
    v.to_be()
}

pub fn ntohs(v: u16) -> u16 {
    u16::from_be(v)
}

pub fn htonl(v: u32) -> u32 {
    v.to_be()
}

pub fn ntohl(v: u32) -> u32 {
    u32::from_be(v)
}

// Checksum calculation (RFC 1071)
pub fn calculate_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i < data.len() {
        let word = if i + 1 < data.len() {
            ((data[i] as u32) << 8) | (data[i+1] as u32)
        } else {
            (data[i] as u32) << 8
        };
        sum = sum.wrapping_add(word);
        i += 2;
    }
    
    while (sum >> 16) > 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    
    !(sum as u16)
}
