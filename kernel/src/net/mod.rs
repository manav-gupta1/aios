pub mod socket;
pub mod smoltcp;
pub mod dns;
use core::sync::atomic::{AtomicU32, Ordering};
use crate::drivers::network::NETWORK_DEVICE;

// Static configuration for QEMU user networking
pub static LOCAL_IP: AtomicU32 = AtomicU32::new(0);
pub static NETMASK: AtomicU32 = AtomicU32::new(0);
pub static GATEWAY: AtomicU32 = AtomicU32::new(0);
pub static DNS_SERVER: AtomicU32 = AtomicU32::new(0x0A000203);

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
    // 10.0.2.3 (QEMU slirp built-in DNS forwarder -> host's DNS config)
    DNS_SERVER.store(ip(10, 0, 2, 3), Ordering::SeqCst);
    
    crate::drivers::storage::serial_print("[NET] IP Layer starting (using static IP for test)...\n");
    crate::drivers::storage::serial_print("[NET] DHCP skipped, using static configuration (10.0.2.15)\n");
    
    smoltcp::init_smoltcp();
}

pub fn handle_network_interrupt() {
    let mut packets = alloc::vec::Vec::new();
    {
        let mut net_dev = NETWORK_DEVICE.lock();
        if let Some(dev) = net_dev.as_mut() {
            dev.ack_interrupt();
            while let Some(pkt) = dev.receive_packet() {
                crate::drivers::storage::serial_print(&alloc::format!("[NET INT] packet received len={}\n", pkt.len()));
                packets.push(pkt);
            }
        }
    }
    
    {
        let mut queue = crate::net::smoltcp::SMOLTCP_RX_QUEUE.lock();
        for pkt in &packets {
            queue.push_back(pkt.clone());
        }
    }
    
    crate::net::smoltcp::poll_smoltcp();
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
