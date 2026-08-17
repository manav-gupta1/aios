use alloc::vec::Vec;
use crate::net::{htons, ntohs, ipv4::send_ipv4_packet, ipv4::IP_PROTO_UDP};

#[repr(C, packed)]
pub struct UdpHeader {
    pub src_port: u16,
    pub dest_port: u16,
    pub length: u16,
    pub checksum: u16,
}

pub fn handle_udp_packet(src_ip: u32, _dest_ip: u32, payload: &[u8]) {
    if payload.len() < 8 {
        return; // Too short
    }

    let header = unsafe { &*(payload.as_ptr() as *const UdpHeader) };
    let dest_port = ntohs(header.dest_port);
    let src_port = ntohs(header.src_port);
    let length = ntohs(header.length) as usize;


    if payload.len() < length {
        return; // Truncated
    }

    let data = &payload[8..length];

    // Find socket in SOCKET_TABLE and queue packet
    let mut table = crate::net::socket::SOCKET_TABLE.lock();
    let target_task_id = table.deliver_udp(dest_port, src_ip, src_port, data);
    
    drop(table); // Drop before waking to avoid deadlocks

    if let Some(task_id) = target_task_id {
        crate::task::scheduler::SCHEDULER.lock().set_task_state(task_id, crate::task::TaskState::Ready);
    }
}

pub fn send_udp_packet(dest_ip: u32, src_port: u16, dest_port: u16, data: &[u8]) -> Result<(), &'static str> {
    let mut packet = Vec::with_capacity(8 + data.len());
    let length = (8 + data.len()) as u16;

    let header = UdpHeader {
        src_port: htons(src_port),
        dest_port: htons(dest_port),
        length: htons(length),
        checksum: 0,
    };

    let header_bytes = unsafe {
        core::slice::from_raw_parts(
            &header as *const UdpHeader as *const u8,
            core::mem::size_of::<UdpHeader>()
        )
    };

    packet.extend_from_slice(header_bytes);
    packet.extend_from_slice(data);

    send_ipv4_packet(dest_ip, IP_PROTO_UDP, &packet)
}
