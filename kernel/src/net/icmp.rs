use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use crate::net::{htons, ntohs, calculate_checksum};

const ICMP_TYPE_ECHO_REPLY: u8 = 0;
const ICMP_TYPE_ECHO_REQUEST: u8 = 8;

pub static LAST_PING_REPLY: AtomicU32 = AtomicU32::new(0);

#[repr(C, packed)]
pub struct IcmpHeader {
    pub type_: u8,
    pub code: u8,
    pub checksum: u16,
    pub identifier: u16,
    pub sequence: u16,
}

pub fn handle_icmp_packet(payload: &[u8], src_ip: u32) {
    if payload.len() < core::mem::size_of::<IcmpHeader>() {
        return;
    }
    
    // Verify checksum
    if calculate_checksum(payload) != 0 {
        return; // Bad checksum
    }
    
    let header = unsafe { core::ptr::read_unaligned(payload.as_ptr() as *const IcmpHeader) };
    
    match header.type_ {
        ICMP_TYPE_ECHO_REQUEST => {
            // Reply
            if header.code == 0 {
                let data = &payload[core::mem::size_of::<IcmpHeader>()..];
                let _ = send_icmp_echo_reply(src_ip, ntohs(header.identifier), ntohs(header.sequence), data);
            }
        }
        ICMP_TYPE_ECHO_REPLY => {
            // Echo Reply received
            LAST_PING_REPLY.store(src_ip, Ordering::Relaxed);
        }
        _ => {}
    }
}

pub fn send_icmp_echo_reply(dest_ip: u32, identifier: u16, sequence: u16, data: &[u8]) -> Result<(), &'static str> {
    let mut packet = Vec::with_capacity(core::mem::size_of::<IcmpHeader>() + data.len());
    
    let mut header = IcmpHeader {
        type_: ICMP_TYPE_ECHO_REPLY,
        code: 0,
        checksum: 0,
        identifier: htons(identifier),
        sequence: htons(sequence),
    };
    
    let mut temp_packet = Vec::with_capacity(core::mem::size_of::<IcmpHeader>() + data.len());
    let header_bytes = unsafe {
        core::slice::from_raw_parts(
            &header as *const IcmpHeader as *const u8,
            core::mem::size_of::<IcmpHeader>()
        )
    };
    temp_packet.extend_from_slice(header_bytes);
    temp_packet.extend_from_slice(data);
    
    header.checksum = htons(calculate_checksum(&temp_packet));
    
    let header_bytes = unsafe {
        core::slice::from_raw_parts(
            &header as *const IcmpHeader as *const u8,
            core::mem::size_of::<IcmpHeader>()
        )
    };
    packet.extend_from_slice(header_bytes);
    packet.extend_from_slice(data);
    
    crate::net::ipv4::send_ipv4_packet(dest_ip, crate::net::ipv4::IP_PROTO_ICMP, &packet)
}

pub fn send_icmp_echo_request(dest_ip: u32, identifier: u16, sequence: u16, data: &[u8]) -> Result<(), &'static str> {
    let mut packet = Vec::with_capacity(core::mem::size_of::<IcmpHeader>() + data.len());
    
    let mut header = IcmpHeader {
        type_: ICMP_TYPE_ECHO_REQUEST,
        code: 0,
        checksum: 0,
        identifier: htons(identifier),
        sequence: htons(sequence),
    };
    
    let mut temp_packet = Vec::with_capacity(core::mem::size_of::<IcmpHeader>() + data.len());
    let header_bytes = unsafe {
        core::slice::from_raw_parts(
            &header as *const IcmpHeader as *const u8,
            core::mem::size_of::<IcmpHeader>()
        )
    };
    temp_packet.extend_from_slice(header_bytes);
    temp_packet.extend_from_slice(data);
    
    header.checksum = htons(calculate_checksum(&temp_packet));
    
    let header_bytes = unsafe {
        core::slice::from_raw_parts(
            &header as *const IcmpHeader as *const u8,
            core::mem::size_of::<IcmpHeader>()
        )
    };
    packet.extend_from_slice(header_bytes);
    packet.extend_from_slice(data);
    
    crate::net::ipv4::send_ipv4_packet(dest_ip, crate::net::ipv4::IP_PROTO_ICMP, &packet)
}
