use alloc::vec::Vec;
use core::sync::atomic::Ordering;
use crate::net::{ip, LOCAL_IP, NETMASK, GATEWAY, DNS_SERVER};
use crate::net::socket::{SocketProtocol, SOCKET_TABLE, sys_bind, sys_sendto, sys_recvfrom, RecvResult};
use crate::task::scheduler::block_current_task;
use crate::task::current_process_id;

const DHCP_CLIENT_PORT: u16 = 68;
const DHCP_SERVER_PORT: u16 = 67;

#[repr(C, packed)]
pub struct DhcpPacket {
    pub op: u8,
    pub htype: u8,
    pub hlen: u8,
    pub hops: u8,
    pub xid: u32,
    pub secs: u16,
    pub flags: u16,
    pub ciaddr: u32,
    pub yiaddr: u32,
    pub siaddr: u32,
    pub giaddr: u32,
    pub chaddr: [u8; 16],
    pub sname: [u8; 64],
    pub file: [u8; 128],
    pub magic: u32, // Magic cookie
    // Options follow...
}

const DHCP_MAGIC: u32 = 0x63825363; // Network byte order for 99.130.83.99

pub fn run_dhcp_client() -> Result<(), &'static str> {
    let mut table = SOCKET_TABLE.lock();
    let sock_id = table.alloc_socket(SocketProtocol::UDP);
    drop(table);
    
    // Clear IP while acquiring DHCP
    crate::net::LOCAL_IP.store(0, core::sync::atomic::Ordering::Relaxed);
    crate::net::NETMASK.store(0, core::sync::atomic::Ordering::Relaxed);
    crate::net::GATEWAY.store(0, core::sync::atomic::Ordering::Relaxed);
    
    // Bind to port 68
    if sys_bind(sock_id, DHCP_CLIENT_PORT).is_err() {
        SOCKET_TABLE.lock().close_socket(sock_id);
        return Err("Failed to bind DHCP socket");
    }
    
    // Send DHCPDISCOVER
    let xid = 0x12345678; // Hardcoded XID for simplicity
    
    let mut discover = Vec::with_capacity(300);
    let mut pkt = DhcpPacket {
        op: 1, // BOOTREQUEST
        htype: 1, // Ethernet
        hlen: 6, // MAC len
        hops: 0,
        xid: crate::net::htonl(xid),
        secs: 0,
        flags: crate::net::htons(0x8000), // Broadcast
        ciaddr: 0,
        yiaddr: 0,
        siaddr: 0,
        giaddr: 0,
        chaddr: [0; 16],
        sname: [0; 64],
        file: [0; 128],
        magic: crate::net::htonl(DHCP_MAGIC),
    };
    
    let mac = match crate::drivers::network::NETWORK_DEVICE.lock().as_ref() {
        Some(dev) => dev.mac_address(),
        None => {
            SOCKET_TABLE.lock().close_socket(sock_id);
            return Err("No network device");
        }
    };
    pkt.chaddr[0..6].copy_from_slice(&mac);
    
    let pkt_bytes = unsafe {
        core::slice::from_raw_parts(&pkt as *const _ as *const u8, core::mem::size_of::<DhcpPacket>())
    };
    discover.extend_from_slice(pkt_bytes);
    
    // DHCP Options
    discover.push(53); // Message type
    discover.push(1);
    discover.push(1); // DISCOVER
    
    // Client Identifier
    discover.push(61);
    discover.push(7);
    discover.push(1); // Ethernet
    discover.extend_from_slice(&mac);
    
    // Parameter Request List
    discover.push(55);
    discover.push(3); // length
    discover.push(1); // Subnet Mask
    discover.push(3); // Router
    discover.push(6); // Domain Name Server
    
    discover.push(255); // End
    
    // Pad to 300 bytes minimum payload size
    while discover.len() < 300 {
        discover.push(0);
    }
    
    // Send to 255.255.255.255:67
    let _ = sys_sendto(sock_id, 0xFFFFFFFF, DHCP_SERVER_PORT, &discover);
    
    // Receive DHCPOFFER
    let mut buf = [0u8; 1024];
    let current_task = current_process_id();
    
    let mut offered_ip = 0;
    let mut server_id = 0;
    
    let mut retries = 0;
    let mut offer_received = false;
    
    while retries < 10000 {
        match sys_recvfrom(sock_id, &mut buf, current_task) {
            Ok(RecvResult::Data { len, .. }) => {
                if len >= core::mem::size_of::<DhcpPacket>() {
                    let recv_pkt = unsafe { &*(buf.as_ptr() as *const DhcpPacket) };
                    if recv_pkt.op == 2 && recv_pkt.xid == crate::net::htonl(xid) {
                        offered_ip = crate::net::ntohl(recv_pkt.yiaddr);
                        
                        // Parse options for Server Identifier
                        let mut offset = core::mem::size_of::<DhcpPacket>();
                        while offset < len {
                            let opt = buf[offset];
                            if opt == 255 { break; }
                            if opt == 0 { offset += 1; continue; }
                            
                            let opt_len = buf[offset + 1] as usize;
                            if opt == 54 && opt_len == 4 { // Server Identifier
                                server_id = crate::net::ntohl(unsafe { *(buf.as_ptr().add(offset + 2) as *const u32) });
                            }
                            
                            offset += opt_len + 2;
                        }
                        offer_received = true;
                        break;
                    }
                }
            }
            Ok(RecvResult::WouldBlock) => {
                block_current_task();
                x86_64::instructions::hlt();
            }
            Err(_) => {
                break;
            }
        }
        retries += 1;
    }
    
    if !offer_received {
        SOCKET_TABLE.lock().close_socket(sock_id);
        return Err("DHCP timeout");
    }
    
    // Send DHCPREQUEST
    let mut request = Vec::with_capacity(300);
    request.extend_from_slice(pkt_bytes);
    
    request.push(53);
    request.push(1);
    request.push(3); // REQUEST
    
    // Client Identifier
    request.push(61);
    request.push(7);
    request.push(1); // Ethernet
    request.extend_from_slice(&mac);
    
    request.push(50); // Requested IP Address
    request.push(4);
    let ip_bytes = crate::net::htonl(offered_ip).to_ne_bytes();
    request.extend_from_slice(&ip_bytes);
    
    request.push(54); // Server Identifier
    request.push(4);
    let srv_bytes = crate::net::htonl(server_id).to_ne_bytes();
    request.extend_from_slice(&srv_bytes);
    
    request.push(255); // End
    
    // Pad to 300 bytes
    while request.len() < 300 {
        request.push(0);
    }
    
    let _ = sys_sendto(sock_id, 0xFFFFFFFF, DHCP_SERVER_PORT, &request);
    
    // Wait for DHCPACK
    let mut ack_received = false;
    
    let mut final_netmask = ip(255, 255, 255, 0);
    let mut final_gateway = 0;
    let mut final_dns = 0;
    
    let mut retries = 0;
    while retries < 10000 {
        match sys_recvfrom(sock_id, &mut buf, current_task) {
            Ok(RecvResult::Data { len, .. }) => {
                if len >= core::mem::size_of::<DhcpPacket>() {
                    let recv_pkt = unsafe { &*(buf.as_ptr() as *const DhcpPacket) };
                    if recv_pkt.op == 2 && recv_pkt.xid == crate::net::htonl(xid) {
                        // Parse options
                        let mut offset = core::mem::size_of::<DhcpPacket>();
                        let mut is_ack = false;
                        
                        while offset < len {
                            let opt = buf[offset];
                            if opt == 255 { break; }
                            if opt == 0 { offset += 1; continue; }
                            
                            let opt_len = buf[offset + 1] as usize;
                            if offset + 2 + opt_len > len { break; }
                            
                            match opt {
                                53 => { // Message Type
                                    if buf[offset + 2] == 5 { is_ack = true; } // ACK
                                }
                                1 => { // Subnet Mask
                                    if opt_len == 4 {
                                        final_netmask = crate::net::ntohl(unsafe { *(buf.as_ptr().add(offset + 2) as *const u32) });
                                    }
                                }
                                3 => { // Router (Gateway)
                                    if opt_len >= 4 {
                                        final_gateway = crate::net::ntohl(unsafe { *(buf.as_ptr().add(offset + 2) as *const u32) });
                                    }
                                }
                                6 => { // DNS Server
                                    if opt_len >= 4 {
                                        final_dns = crate::net::ntohl(unsafe { *(buf.as_ptr().add(offset + 2) as *const u32) });
                                    }
                                }
                                _ => {}
                            }
                            
                            offset += opt_len + 2;
                        }
                        
                        if is_ack {
                            ack_received = true;
                            break;
                        }
                    }
                }
            }
            Ok(RecvResult::WouldBlock) => {
                block_current_task();
                x86_64::instructions::hlt();
            }
            Err(_) => {
                break;
            }
        }
        retries += 1;
    }
    
    SOCKET_TABLE.lock().close_socket(sock_id);
    
    if ack_received {
        LOCAL_IP.store(offered_ip, Ordering::SeqCst);
        NETMASK.store(final_netmask, Ordering::SeqCst);
        GATEWAY.store(final_gateway, Ordering::SeqCst);
        if final_dns != 0 {
            DNS_SERVER.store(final_dns, Ordering::SeqCst);
        } else {
            DNS_SERVER.store(final_gateway, Ordering::SeqCst); // Fallback to gateway if DNS not provided
        }
        Ok(())
    } else {
        Err("DHCP ACK timeout")
    }
}
