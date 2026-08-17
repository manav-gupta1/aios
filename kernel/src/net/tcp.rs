use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::Ordering;
use crate::net::{htons, ntohs, htonl, ntohl, calculate_checksum, LOCAL_IP};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    SynSent,
    Established,
    FinWait1,
    FinWait2,
    TimeWait,
    CloseWait,
    LastAck,
}

pub struct TcpSocket {
    pub local_port: u16,
    pub remote_ip: u32,
    pub remote_port: u16,
    pub state: TcpState,
    pub seq_num: u32,
    pub ack_num: u32,
    pub rx_queue: VecDeque<u8>,
    pub waker_task_id: Option<usize>,
}

impl TcpSocket {
    pub fn new() -> Self {
        Self {
            local_port: 0,
            remote_ip: 0,
            remote_port: 0,
            state: TcpState::Closed,
            seq_num: 12345678, // Random initial sequence number
            ack_num: 0,
            rx_queue: VecDeque::new(),
            waker_task_id: None,
        }
    }
}

#[repr(C, packed)]
pub struct TcpHeader {
    pub src_port: u16,
    pub dest_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset_flags: u16,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_pointer: u16,
}

// TCP Pseudo header for checksum computation
#[repr(C, packed)]
struct TcpPseudoHeader {
    pub src_ip: u32,
    pub dest_ip: u32,
    pub reserved: u8,
    pub protocol: u8,
    pub tcp_length: u16,
}

pub const TCP_FLAG_FIN: u16 = 0x01;
pub const TCP_FLAG_SYN: u16 = 0x02;
pub const TCP_FLAG_RST: u16 = 0x04;
pub const TCP_FLAG_PSH: u16 = 0x08;
pub const TCP_FLAG_ACK: u16 = 0x10;

pub fn send_tcp_packet(
    dest_ip: u32,
    src_port: u16,
    dest_port: u16,
    seq_num: u32,
    ack_num: u32,
    flags: u16,
    payload: &[u8],
) -> Result<(), &'static str> {
    let local_ip = LOCAL_IP.load(Ordering::Relaxed);
    if local_ip == 0 {
        return Err("No local IP");
    }

    let header_len = core::mem::size_of::<TcpHeader>();
    let total_len = header_len + payload.len();

    let mut packet = Vec::with_capacity(total_len);
    packet.resize(total_len, 0);

    let data_offset = 5u16; // 5 32-bit words (20 bytes)
    let data_offset_flags = (data_offset << 12) | flags;

    let header = unsafe { &mut *(packet.as_mut_ptr() as *mut TcpHeader) };
    header.src_port = htons(src_port);
    header.dest_port = htons(dest_port);
    header.seq_num = htonl(seq_num);
    header.ack_num = htonl(ack_num);
    header.data_offset_flags = htons(data_offset_flags);
    header.window_size = htons(8192); // Basic window size
    header.checksum = 0;
    header.urgent_pointer = 0;

    packet[header_len..].copy_from_slice(payload);

    // Compute checksum
    let pseudo_header = TcpPseudoHeader {
        src_ip: htonl(local_ip),
        dest_ip: htonl(dest_ip),
        reserved: 0,
        protocol: crate::net::ipv4::IP_PROTO_TCP,
        tcp_length: htons(total_len as u16),
    };

    let ph_bytes = unsafe {
        core::slice::from_raw_parts(
            &pseudo_header as *const _ as *const u8,
            core::mem::size_of::<TcpPseudoHeader>(),
        )
    };

    let mut checksum_data = Vec::with_capacity(ph_bytes.len() + packet.len());
    checksum_data.extend_from_slice(ph_bytes);
    checksum_data.extend_from_slice(&packet);

    let checksum = calculate_checksum(&checksum_data);
    
    // Set checksum
    let header_mut = unsafe { &mut *(packet.as_mut_ptr() as *mut TcpHeader) };
    header_mut.checksum = crate::net::htons(checksum);

    crate::console::write_str(alloc::format!("[TCP OUT] len={} src_port={} dest_port={} seq={} ack={} flags={:#x} csum={:#x}\n", packet.len(), src_port, dest_port, seq_num, ack_num, flags, checksum).as_str());
    crate::net::ipv4::send_ipv4_packet(dest_ip, crate::net::ipv4::IP_PROTO_TCP, &packet)
}

pub fn handle_tcp_packet(src_ip: u32, payload: &[u8]) {
    if payload.len() < core::mem::size_of::<TcpHeader>() {
        return;
    }

    let header = unsafe { &*(payload.as_ptr() as *const TcpHeader) };
    let dest_port = ntohs(header.dest_port);
    let src_port = ntohs(header.src_port);
    let seq_num = ntohl(header.seq_num);
    let _ack_num = ntohl(header.ack_num);
    let data_offset_flags = ntohs(header.data_offset_flags);
    
    let data_offset = (data_offset_flags >> 12) * 4;
    let flags = data_offset_flags & 0x1FF;
    
    let tcp_payload = if (data_offset as usize) <= payload.len() {
        &payload[(data_offset as usize)..]
    } else {
        &[]
    };
    
    let is_syn = (flags & TCP_FLAG_SYN) != 0;
    let is_ack = (flags & TCP_FLAG_ACK) != 0;
    let is_fin = (flags & TCP_FLAG_FIN) != 0;
    let is_rst = (flags & TCP_FLAG_RST) != 0;

    let table = crate::net::socket::SOCKET_TABLE.lock();
    
    // Find matching TCP socket
    let mut matching_sock_id = None;
    for (id, sock_opt) in table.sockets.iter().enumerate() {
        if let Some(crate::net::socket::Socket::Tcp(tcp_mutex)) = sock_opt {
            let tcp = tcp_mutex.lock();
            if tcp.local_port == dest_port && (tcp.remote_ip == 0 || tcp.remote_ip == src_ip) && (tcp.remote_port == 0 || tcp.remote_port == src_port) {
                matching_sock_id = Some(id + 1);
                break;
            }
        }
    }

    if let Some(sock_id) = matching_sock_id {
        let sock_opt = table.get_socket(sock_id).unwrap();
        if let crate::net::socket::Socket::Tcp(tcp_mutex) = sock_opt {
            let mut tcp = tcp_mutex.lock();
            
            if is_rst {
                crate::console::write_str(alloc::format!("[TCP] Socket {} RST received -> Closed\n", sock_id).as_str());
                tcp.state = TcpState::Closed;
                if let Some(task_id) = tcp.waker_task_id.take() {
                    crate::task::scheduler::SCHEDULER.lock().set_task_state(task_id, crate::task::TaskState::Ready);
                }
                return;
            }

            match tcp.state {
                TcpState::SynSent => {
                    if is_syn && is_ack {
                        tcp.ack_num = seq_num + 1;
                        tcp.seq_num += 1;
                        tcp.state = TcpState::Established;
                        
                        // Send ACK
                        let _ = send_tcp_packet(
                            tcp.remote_ip, tcp.local_port, tcp.remote_port,
                            tcp.seq_num, tcp.ack_num, TCP_FLAG_ACK, &[]
                        );
                        
                        if let Some(task_id) = tcp.waker_task_id.take() {
                            crate::task::scheduler::SCHEDULER.lock().set_task_state(task_id, crate::task::TaskState::Ready);
                        }
                    }
                }
                TcpState::Established => {
                    let expected_seq = tcp.ack_num;
                    if seq_num == expected_seq {
                        if !tcp_payload.is_empty() {
                            tcp.rx_queue.extend(tcp_payload);
                            tcp.ack_num += tcp_payload.len() as u32;
                            
                            // Send ACK
                            let _ = send_tcp_packet(
                                tcp.remote_ip, tcp.local_port, tcp.remote_port,
                                tcp.seq_num, tcp.ack_num, TCP_FLAG_ACK, &[]
                            );
                            
                            if let Some(task_id) = tcp.waker_task_id.take() {
                                crate::task::scheduler::SCHEDULER.lock().set_task_state(task_id, crate::task::TaskState::Ready);
                            }
                        }
                        
                        if is_fin {
                            tcp.ack_num += 1;
                            tcp.state = TcpState::CloseWait;
                            
                            // Send ACK for FIN
                            let _ = send_tcp_packet(
                                tcp.remote_ip, tcp.local_port, tcp.remote_port,
                                tcp.seq_num, tcp.ack_num, TCP_FLAG_ACK, &[]
                            );
                            
                            if let Some(task_id) = tcp.waker_task_id.take() {
                                crate::task::scheduler::SCHEDULER.lock().set_task_state(task_id, crate::task::TaskState::Ready);
                            }
                        }
                    } else if seq_num > expected_seq {
                        // Out of order packet, send ACK for what we have
                        let _ = send_tcp_packet(
                            tcp.remote_ip, tcp.local_port, tcp.remote_port,
                            tcp.seq_num, tcp.ack_num, TCP_FLAG_ACK, &[]
                        );
                    }
                }
                TcpState::FinWait1 => {
                    if is_ack {
                        tcp.state = TcpState::FinWait2;
                    }
                    if is_fin {
                        tcp.ack_num = seq_num + 1;
                        let _ = send_tcp_packet(
                            tcp.remote_ip, tcp.local_port, tcp.remote_port,
                            tcp.seq_num, tcp.ack_num, TCP_FLAG_ACK, &[]
                        );
                        tcp.state = if tcp.state == TcpState::FinWait2 { TcpState::TimeWait } else { TcpState::Closed };
                    }
                }
                TcpState::FinWait2 => {
                    if is_fin {
                        tcp.ack_num = seq_num + 1;
                        let _ = send_tcp_packet(
                            tcp.remote_ip, tcp.local_port, tcp.remote_port,
                            tcp.seq_num, tcp.ack_num, TCP_FLAG_ACK, &[]
                        );
                        tcp.state = TcpState::Closed; // Skip TimeWait for minimal impl
                        if let Some(task_id) = tcp.waker_task_id.take() {
                            crate::task::scheduler::SCHEDULER.lock().set_task_state(task_id, crate::task::TaskState::Ready);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
