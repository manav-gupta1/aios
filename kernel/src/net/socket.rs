use alloc::collections::VecDeque;
use alloc::vec::Vec;
use spin::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketProtocol {
    UDP,
    TCP,
}

pub struct UdpSocket {
    pub local_port: u16,
    pub rx_queue: VecDeque<(u32, u16, Vec<u8>)>, // (src_ip, src_port, payload)
    pub waker_task_id: Option<usize>, // Task blocked on recvfrom
}

pub enum Socket {
    Udp(Mutex<UdpSocket>),
    Tcp(Mutex<crate::net::tcp::TcpSocket>),
}

pub struct SocketTable {
    pub sockets: Vec<Option<Socket>>,
    next_id: usize,
}

impl SocketTable {
    pub const fn new() -> Self {
        SocketTable {
            sockets: Vec::new(),
            next_id: 1, // 0 is reserved/invalid
        }
    }

    pub fn alloc_socket(&mut self, protocol: SocketProtocol) -> usize {
        let sock = match protocol {
            SocketProtocol::UDP => {
                Socket::Udp(Mutex::new(UdpSocket {
                    local_port: 0,
                    rx_queue: VecDeque::with_capacity(32),
                    waker_task_id: None,
                }))
            }
            SocketProtocol::TCP => {
                Socket::Tcp(Mutex::new(crate::net::tcp::TcpSocket::new()))
            }
        };

        // Find free slot
        for (i, slot) in self.sockets.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(sock);
                return i + 1;
            }
        }

        self.sockets.push(Some(sock));
        self.sockets.len()
    }

    pub fn get_socket(&self, id: usize) -> Option<&Socket> {
        if id == 0 || id > self.sockets.len() {
            return None;
        }
        self.sockets[id - 1].as_ref()
    }

    pub fn close_socket(&mut self, id: usize) {
        if id > 0 && id <= self.sockets.len() {
            if let Some(sock) = self.sockets[id - 1].take() {
                match sock {
                    Socket::Udp(udp) => {
                        let u = udp.lock();
                        if let Some(task_id) = u.waker_task_id {
                            crate::task::scheduler::SCHEDULER.lock().set_task_state(task_id, crate::task::TaskState::Ready);
                        }
                    }
                    Socket::Tcp(tcp) => {
                        let t = tcp.lock();
                        if let Some(task_id) = t.waker_task_id {
                            crate::task::scheduler::SCHEDULER.lock().set_task_state(task_id, crate::task::TaskState::Ready);
                        }
                    }
                }
            }
        }
    }

    pub fn deliver_udp(&mut self, dest_port: u16, src_ip: u32, src_port: u16, data: &[u8]) -> Option<usize> {
        crate::drivers::storage::serial_print("[UDP RX] packet received\n");
        crate::drivers::storage::serial_print("[UDP RX] destination port\n");
        crate::drivers::storage::serial_print("[UDP RX] source port\n");
        for slot in &self.sockets {
            if let Some(Socket::Udp(udp)) = slot {
                let mut u = udp.lock();
                if u.local_port == dest_port {
                    if u.rx_queue.len() < 32 {
                        let mut payload = alloc::vec::Vec::with_capacity(data.len());
                        payload.extend_from_slice(data);
                        u.rx_queue.push_back((src_ip, src_port, payload));
                        crate::drivers::storage::serial_print("[UDP RX] delivered to socket\n");
                    }
                    if let Some(task_id) = u.waker_task_id {
                        u.waker_task_id = None;
                        return Some(task_id);
                    }
                }
            }
        }
        None
    }

    pub fn snapshots(&self) -> alloc::vec::Vec<(&'static str, u16)> {
        let mut list = alloc::vec::Vec::new();
        for slot in &self.sockets {
            if let Some(sock) = slot {
                match sock {
                    Socket::Udp(udp) => {
                        list.push(("UDP", udp.lock().local_port));
                    }
                    Socket::Tcp(tcp) => {
                        list.push(("TCP", tcp.lock().local_port));
                    }
                }
            }
        }
        list
    }
}

pub static SOCKET_TABLE: Mutex<SocketTable> = Mutex::new(SocketTable::new());

// Bind a socket to a local port
pub fn sys_bind(socket_id: usize, port: u16) -> Result<(), &'static str> {
    let table = SOCKET_TABLE.lock();
    let sock = table.get_socket(socket_id).ok_or("Invalid socket ID")?;
    
    match sock {
        Socket::Udp(udp) => {
            let mut u = udp.lock();
            if u.local_port != 0 {
                return Err("Already bound");
            }
            u.local_port = port;
            Ok(())
        }
        Socket::Tcp(tcp) => {
            let mut t = tcp.lock();
            if t.local_port != 0 {
                return Err("Already bound");
            }
            t.local_port = port;
            Ok(())
        }
    }
}

// Send data over a socket
pub fn sys_sendto(socket_id: usize, dest_ip: u32, dest_port: u16, data: &[u8]) -> Result<usize, &'static str> {
    let table = SOCKET_TABLE.lock();
    let sock = table.get_socket(socket_id).ok_or("Invalid socket ID")?;
    
    match sock {
        Socket::Udp(udp) => {
            let local_port = udp.lock().local_port;
            let _src_port = if local_port == 0 {
                // Auto-bind to ephemeral port (simplification: just pick a random/fixed one for now)
                let ephemeral = 49152 + (socket_id as u16);
                udp.lock().local_port = ephemeral;
                ephemeral
            } else {
                local_port
            };
            
            // We must unlock SOCKET_TABLE before sending, as sending might trigger ARP resolution
            // which might trigger receive_packet which might lock NETWORK_DEVICE. 
            // It's safe to drop here because `Socket` lives in the static table.
        }
        Socket::Tcp(_) => return Err("Use send for TCP"),
    }
    
    // Drop lock before doing network I/O
    drop(table);
    
    // Now get it again just to extract local_port
    let local_port = {
        let table = SOCKET_TABLE.lock();
        let sock = table.get_socket(socket_id).unwrap();
        match sock {
            Socket::Udp(udp) => udp.lock().local_port,
            Socket::Tcp(tcp) => tcp.lock().local_port,
        }
    };
    
    crate::net::udp::send_udp_packet(dest_ip, local_port, dest_port, data)?;
    Ok(data.len())
}

pub enum RecvResult {
    Data { src_ip: u32, src_port: u16, len: usize },
    WouldBlock,
}

// Receive data from a socket
pub fn sys_recvfrom(socket_id: usize, buf: &mut [u8], task_id: usize) -> Result<RecvResult, &'static str> {
    let table = SOCKET_TABLE.lock();
    let sock = table.get_socket(socket_id).ok_or("Invalid socket ID")?;
    
    match sock {
        Socket::Udp(udp) => {
            let mut u = udp.lock();
            if let Some((src_ip, src_port, payload)) = u.rx_queue.pop_front() {
                let copy_len = core::cmp::min(buf.len(), payload.len());
                buf[..copy_len].copy_from_slice(&payload[..copy_len]);
                u.waker_task_id = None;
                Ok(RecvResult::Data { src_ip, src_port, len: copy_len })
            } else {
                u.waker_task_id = Some(task_id);
                Ok(RecvResult::WouldBlock)
            }
        }
        Socket::Tcp(_) => Err("Use recv for TCP"),
    }
}

pub fn sys_connect(socket_id: usize, dest_ip: u32, dest_port: u16, task_id: usize) -> Result<(), &'static str> {
    let table = SOCKET_TABLE.lock();
    let sock = table.get_socket(socket_id).ok_or("Invalid socket ID")?;
    
    match sock {
        Socket::Udp(_) => Err("Connect not supported for UDP"),
        Socket::Tcp(tcp_mutex) => {
            let mut tcp = tcp_mutex.lock();
            
            if tcp.state == crate::net::tcp::TcpState::Established {
                return Ok(());
            }
            
            if tcp.state == crate::net::tcp::TcpState::Closed {
                tcp.remote_ip = dest_ip;
                tcp.remote_port = dest_port;
                if tcp.local_port == 0 {
                    tcp.local_port = 49152 + (socket_id as u16);
                }
                tcp.state = crate::net::tcp::TcpState::SynSent;
                tcp.waker_task_id = Some(task_id);
            } else if tcp.state == crate::net::tcp::TcpState::SynSent {
                tcp.waker_task_id = Some(task_id);
            }
            Err("WouldBlock")
        }
    }
}

pub fn tcp_retry_syn(socket_id: usize) {
    let table = SOCKET_TABLE.lock();
    if let Some(Socket::Tcp(tcp_mutex)) = table.get_socket(socket_id) {
        let tcp = tcp_mutex.lock();
        if tcp.state == crate::net::tcp::TcpState::SynSent {
            let _ = crate::net::tcp::send_tcp_packet(
                tcp.remote_ip, tcp.local_port, tcp.remote_port,
                tcp.seq_num, tcp.ack_num, crate::net::tcp::TCP_FLAG_SYN, &[]
            );
        }
    }
}

pub fn sys_send(socket_id: usize, data: &[u8]) -> Result<usize, &'static str> {
    let table = SOCKET_TABLE.lock();
    let sock = table.get_socket(socket_id).ok_or("Invalid socket ID")?;
    
    match sock {
        Socket::Udp(_) => Err("Use sendto for UDP"),
        Socket::Tcp(tcp_mutex) => {
            let mut tcp = tcp_mutex.lock();
            if tcp.state != crate::net::tcp::TcpState::Established {
                return Err("Not connected");
            }
            
            let _ = crate::net::tcp::send_tcp_packet(
                tcp.remote_ip, tcp.local_port, tcp.remote_port,
                tcp.seq_num, tcp.ack_num, crate::net::tcp::TCP_FLAG_ACK | crate::net::tcp::TCP_FLAG_PSH, data
            );
            
            crate::console::write_str(alloc::format!("[SYS_SEND] state after send: {:?}\n", tcp.state).as_str());
            tcp.seq_num += data.len() as u32;
            Ok(data.len())
        }
    }
}

pub fn sys_recv(socket_id: usize, buf: &mut [u8], task_id: usize) -> Result<RecvResult, &'static str> {
    let table = SOCKET_TABLE.lock();
    let sock = table.get_socket(socket_id).ok_or("Invalid socket ID")?;
    
    match sock {
        Socket::Udp(_) => Err("Use recvfrom for UDP"),
        Socket::Tcp(tcp_mutex) => {
            let mut tcp = tcp_mutex.lock();
            
            if !tcp.rx_queue.is_empty() {
                let copy_len = core::cmp::min(buf.len(), tcp.rx_queue.len());
                for i in 0..copy_len {
                    buf[i] = tcp.rx_queue.pop_front().unwrap();
                }
                tcp.waker_task_id = None;
                Ok(RecvResult::Data { src_ip: tcp.remote_ip, src_port: tcp.remote_port, len: copy_len })
            } else if tcp.state == crate::net::tcp::TcpState::Closed || tcp.state == crate::net::tcp::TcpState::CloseWait {
                crate::console::write_str(alloc::format!("[SYS_RECV] Returning EOF because state is {:?}\n", tcp.state).as_str());
                Ok(RecvResult::Data { src_ip: tcp.remote_ip, src_port: tcp.remote_port, len: 0 }) // EOF
            } else {
                tcp.waker_task_id = Some(task_id);
                Ok(RecvResult::WouldBlock)
            }
        }
    }
}
