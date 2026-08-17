use alloc::vec::Vec;
use core::sync::atomic::Ordering;
use crate::net::DNS_SERVER;
use crate::net::socket::{SocketProtocol, SOCKET_TABLE, sys_bind, sys_sendto, sys_recvfrom, RecvResult};
use crate::task::scheduler::block_current_task;
use crate::task::current_task_id;

const DNS_PORT: u16 = 53;

#[repr(C, packed)]
pub struct DnsHeader {
    pub id: u16,
    pub flags: u16,
    pub qdcount: u16,
    pub ancount: u16,
    pub nscount: u16,
    pub arcount: u16,
}

pub fn resolve(domain: &str) -> Option<u32> {
    let mut table = SOCKET_TABLE.lock();
    let sock_id = table.alloc_socket(SocketProtocol::UDP);
    drop(table);
    
    if sock_id == 0 {
        crate::drivers::storage::serial_print("[DNS] final result = ERROR\n");
        return None;
    }
    
    // Bind to ephemeral port
    static NEXT_PORT: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(49152);
    let port = NEXT_PORT.fetch_add(1, Ordering::Relaxed);
    if sys_bind(sock_id, port).is_err() {
        SOCKET_TABLE.lock().close_socket(sock_id);
        crate::drivers::storage::serial_print("[DNS] final result = ERROR\n");
        return None;
    }
    
    let dns_server = DNS_SERVER.load(Ordering::Relaxed);
    if dns_server == 0 {
        SOCKET_TABLE.lock().close_socket(sock_id);
        crate::drivers::storage::serial_print("[DNS] final result = ERROR\n");
        return None;
    }
    
    crate::drivers::storage::serial_print(&alloc::format!("[DNS] server = {}.{}.{}.{}\n", 
        (dns_server >> 24) & 0xFF, (dns_server >> 16) & 0xFF, (dns_server >> 8) & 0xFF, dns_server & 0xFF));
    
    // Construct query
    let mut packet = alloc::vec::Vec::new();
    let xid = 0x1234;
    
    let header = DnsHeader {
        id: crate::net::htons(xid),
        flags: crate::net::htons(0x0100), // Standard query, Recursion desired
        qdcount: crate::net::htons(1),
        ancount: 0,
        nscount: 0,
        arcount: 0,
    };
    
    let hdr_ptr = &header as *const _ as *const u8;
    let hdr_slice = unsafe { core::slice::from_raw_parts(hdr_ptr, core::mem::size_of::<DnsHeader>()) };
    packet.extend_from_slice(hdr_slice);
    
    for part in domain.split('.') {
        if part.is_empty() { continue; }
        packet.push(part.len() as u8);
        packet.extend_from_slice(part.as_bytes());
    }
    packet.push(0); // root label
    
    packet.push(0); // QTYPE A (1)
    packet.push(1);
    packet.push(0); // QCLASS IN (1)
    packet.push(1);
    
    let mut send_retries = 0;
    let mut sent = false;
    while send_retries < 100 {
        if sys_sendto(sock_id, dns_server, DNS_PORT, &packet).is_ok() {
            sent = true;
            break;
        }
        x86_64::instructions::interrupts::enable_and_hlt();
        x86_64::instructions::interrupts::disable();
        send_retries += 1;
    }
    
    if sent {
        crate::drivers::storage::serial_print("[DNS] query sent = YES\n");
    } else {
        crate::drivers::storage::serial_print("[DNS] query sent = NO\n");
        crate::drivers::storage::serial_print("[DNS] final result = ERROR\n");
        SOCKET_TABLE.lock().close_socket(sock_id);
        return None;
    }
    
    // Receive response
    let mut buf = [0u8; 1024];
    let current_task = crate::task::current_task_id();
    
    let mut resolved_ip = None;
    let mut final_result = "ERROR";
    let mut response_received = false;
    
    let start_ticks = crate::drivers::timer::TimerDriver::get_ticks();
    
    loop {
        match sys_recvfrom(sock_id, &mut buf, current_task) {
            Ok(RecvResult::Data { len, src_ip, .. }) => {
                response_received = true;
                crate::drivers::storage::serial_print("[DNS] response received = YES\n");
                crate::drivers::storage::serial_print(&alloc::format!("[DNS] response length = {}\n", len));
                
                if src_ip == dns_server && len >= core::mem::size_of::<DnsHeader>() {
                    let recv_hdr = unsafe { &*(buf.as_ptr() as *const DnsHeader) };
                    crate::drivers::storage::serial_print(&alloc::format!("[DNS] transaction ID = {:04x}\n", crate::net::ntohs(recv_hdr.id)));
                    let flags = crate::net::ntohs(recv_hdr.flags);
                    crate::drivers::storage::serial_print(&alloc::format!("[DNS] flags = {:04x}\n", flags));
                    
                    if recv_hdr.id == crate::net::htons(xid) {
                        let rcode = flags & 0x000F;
                        let ancount = crate::net::ntohs(recv_hdr.ancount);
                        crate::drivers::storage::serial_print(&alloc::format!("[DNS] ANCOUNT = {}\n", ancount));
                        
                        if (flags & 0x8000) != 0 {
                            if rcode == 3 {
                                final_result = "NXDOMAIN";
                                crate::drivers::storage::serial_print("[DNS] A record found = NO\n");
                            } else if rcode == 0 {
                                if ancount > 0 {
                                    // Skip question section
                                    let mut offset = core::mem::size_of::<DnsHeader>();
                                    let qdcount = crate::net::ntohs(recv_hdr.qdcount);
                                    for _ in 0..qdcount {
                                        let mut compressed = false;
                                        while offset < len && buf[offset] != 0 {
                                            if (buf[offset] & 0xC0) == 0xC0 {
                                                offset += 2;
                                                compressed = true;
                                                break;
                                            } else {
                                                offset += 1 + buf[offset] as usize;
                                            }
                                        }
                                        if !compressed && offset < len && buf[offset] == 0 {
                                            offset += 1;
                                        }
                                        offset += 4;
                                    }
                                    
                                    // Parse answers
                                    let mut found = false;
                                    for _ in 0..ancount {
                                        if offset >= len { break; }
                                        let mut compressed = false;
                                        while offset < len && buf[offset] != 0 {
                                            if (buf[offset] & 0xC0) == 0xC0 {
                                                offset += 2;
                                                compressed = true;
                                                break;
                                            } else {
                                                offset += 1 + buf[offset] as usize;
                                            }
                                        }
                                        if !compressed && offset < len && buf[offset] == 0 { offset += 1; }
                                        if offset + 10 > len { break; }
                                        
                                        let atype = crate::net::ntohs(unsafe { *(buf.as_ptr().add(offset) as *const u16) });
                                        let aclass = crate::net::ntohs(unsafe { *(buf.as_ptr().add(offset + 2) as *const u16) });
                                        let rdlength = crate::net::ntohs(unsafe { *(buf.as_ptr().add(offset + 8) as *const u16) }) as usize;
                                        offset += 10;
                                        
                                        if atype == 1 && aclass == 1 && rdlength == 4 && offset + 4 <= len {
                                            let ip = crate::net::ntohl(unsafe { *(buf.as_ptr().add(offset) as *const u32) });
                                            resolved_ip = Some(ip);
                                            found = true;
                                            break;
                                        }
                                        offset += rdlength;
                                    }
                                    
                                    if found {
                                        crate::drivers::storage::serial_print("[DNS] A record found = YES\n");
                                        let ip = resolved_ip.unwrap();
                                        crate::drivers::storage::serial_print(&alloc::format!("[DNS] IPv4 = {}.{}.{}.{}\n", 
                                            (ip >> 24) & 0xFF, (ip >> 16) & 0xFF, (ip >> 8) & 0xFF, ip & 0xFF));
                                        final_result = "SUCCESS";
                                    } else {
                                        crate::drivers::storage::serial_print("[DNS] A record found = NO\n");
                                        final_result = "NXDOMAIN"; // or similar, no A record
                                    }
                                } else {
                                    crate::drivers::storage::serial_print("[DNS] A record found = NO\n");
                                    final_result = "NXDOMAIN";
                                }
                            } else {
                                final_result = "NETWORK_ERROR";
                            }
                        } else {
                            final_result = "PARSE_ERROR";
                        }
                        break;
                    }
                } else {
                    final_result = "PARSE_ERROR";
                    break;
                }
            }
            Ok(RecvResult::WouldBlock) => {
                let current_ticks = crate::drivers::timer::TimerDriver::get_ticks();
                if current_ticks - start_ticks > 300 { 
                    final_result = "TIMEOUT";
                    break;
                }
                
                let mut table = SOCKET_TABLE.lock();
                if let Some(crate::net::socket::Socket::Udp(udp)) = &mut table.sockets[sock_id - 1] {
                    let mut u = udp.lock();
                    u.waker_task_id = None;
                }
                drop(table);

                x86_64::instructions::interrupts::enable();
                x86_64::instructions::hlt();
            }
            Err(_) => {
                final_result = "NETWORK_ERROR";
                break;
            }
        }
        
        let current_ticks = crate::drivers::timer::TimerDriver::get_ticks();
        if current_ticks - start_ticks > 300 {
            final_result = "TIMEOUT";
            break;
        }
    }
    
    if !response_received {
        crate::drivers::storage::serial_print("[DNS] response received = NO\n");
    }
    
    crate::drivers::storage::serial_print(&alloc::format!("[DNS] final result = {}\n", final_result));
    SOCKET_TABLE.lock().close_socket(sock_id);
    
    resolved_ip
}
