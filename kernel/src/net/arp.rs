use spin::Mutex;
use alloc::collections::BTreeMap;
use core::sync::atomic::Ordering;
use crate::net::{htons, ntohs};

const ARP_HW_ETHERNET: u16 = 1;
const ARP_PROTO_IPV4: u16 = 0x0800;

const ARP_OP_REQUEST: u16 = 1;
const ARP_OP_REPLY: u16 = 2;

#[repr(C, packed)]
pub struct ArpHeader {
    pub hw_type: u16,
    pub proto_type: u16,
    pub hw_len: u8,
    pub proto_len: u8,
    pub opcode: u16,
    pub sender_mac: [u8; 6],
    pub sender_ip: u32,
    pub target_mac: [u8; 6],
    pub target_ip: u32,
}

pub static ARP_CACHE: Mutex<Option<BTreeMap<u32, [u8; 6]>>> = Mutex::new(None);

pub fn init() {
    *ARP_CACHE.lock() = Some(BTreeMap::new());
}

pub fn handle_arp_packet(payload: &[u8]) {
    if payload.len() < core::mem::size_of::<ArpHeader>() {
        return;
    }
    
    let header = unsafe { core::ptr::read_unaligned(payload.as_ptr() as *const ArpHeader) };
    
    let hw_type = ntohs(header.hw_type);
    let proto_type = ntohs(header.proto_type);
    let opcode = ntohs(header.opcode);
    
    let sender_ip = crate::net::ntohl(header.sender_ip);
    let target_ip = crate::net::ntohl(header.target_ip);
    
    crate::drivers::storage::serial_print(&alloc::format!("ARP IN opcode={} src={:x} dst={:x}\n", opcode, sender_ip, target_ip));
    
    if hw_type != ARP_HW_ETHERNET || proto_type != ARP_PROTO_IPV4 || header.hw_len != 6 || header.proto_len != 4 {
        crate::drivers::storage::serial_print(&alloc::format!("ARP drop: hw={} proto={:x} hlen={} plen={}\n", hw_type, proto_type, header.hw_len, header.proto_len));
        return; // Unsupported hardware/protocol type
    }
    
    // Update cache
    if let Some(cache) = ARP_CACHE.lock().as_mut() {
        cache.insert(sender_ip, header.sender_mac);
    }
    
    let local_ip = crate::net::LOCAL_IP.load(Ordering::Relaxed);
    
    // If it's a request for us (or we are acquiring an IP via DHCP), reply
    if opcode == ARP_OP_REQUEST {
        crate::drivers::storage::serial_print(&alloc::format!("ARP REQ target={:x} local={:x}\n", target_ip, local_ip));
        if target_ip == local_ip || local_ip == 0 {
            let mut net_dev = crate::drivers::network::NETWORK_DEVICE.lock();
            if let Some(dev) = net_dev.as_mut() {
                let my_mac = dev.mac_address();
            // Drop lock so we can call send_ethernet_frame
            drop(net_dev);
            
            let reply = ArpHeader {
                hw_type: htons(ARP_HW_ETHERNET),
                proto_type: htons(ARP_PROTO_IPV4),
                hw_len: 6,
                proto_len: 4,
                opcode: htons(ARP_OP_REPLY),
                sender_mac: my_mac,
                sender_ip: crate::net::htonl(target_ip),
                target_mac: header.sender_mac,
                target_ip: crate::net::htonl(sender_ip),
            };
            
            let reply_bytes = unsafe {
                core::slice::from_raw_parts(
                    &reply as *const ArpHeader as *const u8,
                    core::mem::size_of::<ArpHeader>()
                )
            };
            
            let _ = crate::net::ethernet::send_ethernet_frame(header.sender_mac, crate::net::ethernet::ETHERTYPE_ARP, reply_bytes);
        }
    }
    }
}

pub fn arp_resolve(ip: u32) -> Option<[u8; 6]> {
    if ip == 0xFFFFFFFF {
        return Some([0xFF; 6]);
    }
    
    // 1. Check cache
    if let Some(cache) = ARP_CACHE.lock().as_ref() {
        if let Some(mac) = cache.get(&ip) {
            return Some(*mac);
        }
    }
    
    // 2. Not in cache, send ARP Request
    let local_ip = crate::net::LOCAL_IP.load(Ordering::Relaxed);
    
    let my_mac = {
        let net_dev = crate::drivers::network::NETWORK_DEVICE.lock();
        if let Some(dev) = net_dev.as_ref() {
            dev.mac_address()
        } else {
            return None;
        }
    };
    
    let req = ArpHeader {
        hw_type: htons(ARP_HW_ETHERNET),
        proto_type: htons(ARP_PROTO_IPV4),
        hw_len: 6,
        proto_len: 4,
        opcode: htons(ARP_OP_REQUEST),
        sender_mac: my_mac,
        sender_ip: crate::net::htonl(local_ip),
        target_mac: [0; 6],
        target_ip: crate::net::htonl(ip),
    };
    
    let req_bytes = unsafe {
        core::slice::from_raw_parts(
            &req as *const ArpHeader as *const u8,
            core::mem::size_of::<ArpHeader>()
        )
    };
    
    let _ = crate::net::ethernet::send_ethernet_frame(crate::net::ethernet::BROADCAST_MAC, crate::net::ethernet::ETHERTYPE_ARP, req_bytes);
    
    // Don't wait for reply here, just return None. The caller will have to retry.
    None
}
