#![allow(clippy::all)]
use spin::Mutex;
use alloc::vec::Vec;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SocketProtocol {
    TCP,
    UDP,
    ICMP,
}

pub struct SmolTcpSocket {
    pub handle: smoltcp::iface::SocketHandle,
    pub local_port: u16,
    pub waker_task_id: Option<usize>,
    pub is_connecting: bool,
}

pub enum Socket {
    SmolTcp(Mutex<SmolTcpSocket>),
}

pub struct SocketTable {
    pub sockets: Vec<Option<Socket>>,
}

impl SocketTable {
    pub const fn new() -> Self {
        SocketTable {
            sockets: Vec::new(),
        }
    }

    pub fn alloc_socket(&mut self, proto: SocketProtocol) -> usize {
        let handle = if proto == SocketProtocol::TCP {
            let tcp_rx_buffer = smoltcp::socket::tcp::SocketBuffer::new(alloc::vec![0; 8192]);
            let tcp_tx_buffer = smoltcp::socket::tcp::SocketBuffer::new(alloc::vec![0; 8192]);
            let tcp_socket = smoltcp::socket::tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer);
            crate::net::smoltcp::try_with_smoltcp_sockets(|sockets| {
                sockets.add(tcp_socket)
            }).unwrap()
        } else {
            let tcp_rx_buffer = smoltcp::socket::tcp::SocketBuffer::new(alloc::vec![0; 8192]);
            let tcp_tx_buffer = smoltcp::socket::tcp::SocketBuffer::new(alloc::vec![0; 8192]);
            let tcp_socket = smoltcp::socket::tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer);
            crate::net::smoltcp::try_with_smoltcp_sockets(|sockets| {
                sockets.add(tcp_socket)
            }).unwrap()
        };

        let socket = Some(Socket::SmolTcp(spin::Mutex::new(SmolTcpSocket {
            handle,
            local_port: 0,
            waker_task_id: None,
            is_connecting: false,
        })));

        for (i, slot) in self.sockets.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = socket;
                return i;
            }
        }
        let id = self.sockets.len();
        self.sockets.push(socket);
        id
    }

    pub fn close_socket(&mut self, id: usize) {
        if id < self.sockets.len() {
            if let Some(Socket::SmolTcp(ref tcp_mutex)) = self.sockets[id] {
                let tcp = tcp_mutex.lock();
                let handle = tcp.handle;
                if let Some(task_id) = tcp.waker_task_id {
                    crate::task::scheduler::SCHEDULER.lock().set_task_state(task_id, crate::task::TaskState::Ready);
                }
                drop(tcp);
                crate::net::smoltcp::try_with_smoltcp_sockets(|sockets| {
                    sockets.remove(handle);
                });
            }
            self.sockets[id] = None;
        }
    }

    pub fn get_socket(&self, id: usize) -> Option<&Socket> {
        self.sockets.get(id)?.as_ref()
    }
    
    pub fn snapshots(&self) -> alloc::vec::Vec<(&'static str, u16)> {
        let mut list = alloc::vec::Vec::new();
        for slot in &self.sockets {
            if let Some(Socket::SmolTcp(tcp)) = slot {
                list.push(("SMOLTCP", tcp.lock().local_port));
            }
        }
        list
    }
}

pub static SOCKET_TABLE: Mutex<SocketTable> = Mutex::new(SocketTable::new());

pub fn sys_bind(socket_id: usize, port: u16) -> Result<(), &'static str> {
    let table = SOCKET_TABLE.lock();
    let sock = table.get_socket(socket_id).ok_or("Invalid socket ID")?;
    match sock {
        Socket::SmolTcp(tcp) => {
            let mut t = tcp.lock();
            if t.local_port != 0 {
                return Err("Already bound");
            }
            t.local_port = port;
            Ok(())
        }
    }
}

pub fn sys_sendto(_socket_id: usize, _dest_ip: u32, _dest_port: u16, _data: &[u8]) -> Result<usize, &'static str> {
    Err("Legacy UDP disabled")
}

pub enum RecvResult {
    Data { src_ip: u32, src_port: u16, len: usize },
    WouldBlock,
}

pub fn sys_recvfrom(_socket_id: usize, _buf: &mut [u8], _task_id: usize) -> Result<RecvResult, &'static str> {
    Err("Legacy UDP disabled")
}

pub fn sys_connect(socket_id: usize, dest_ip: u32, dest_port: u16, task_id: usize) -> Result<(), &'static str> {
    let table = SOCKET_TABLE.lock();
    let sock = table.get_socket(socket_id).ok_or("Invalid socket ID")?;
    match sock {
        Socket::SmolTcp(tcp_mutex) => {
            let mut tcp = tcp_mutex.lock();
            let dest_addr = smoltcp::wire::IpAddress::v4((dest_ip >> 24) as u8, (dest_ip >> 16) as u8, (dest_ip >> 8) as u8, dest_ip as u8);
            let handle = tcp.handle;
            
            let result = crate::net::smoltcp::try_with_smoltcp_sockets(|sockets| {
                let socket = sockets.get_mut::<smoltcp::socket::tcp::Socket>(handle);
                match socket.state() {
                    smoltcp::socket::tcp::State::Closed => {
                        if tcp.is_connecting {
                            tcp.is_connecting = false;
                            Err("ConnectionRefused")
                        } else {
                            let local_port = if tcp.local_port == 0 { 49152 + (socket_id as u16) } else { tcp.local_port };
                            tcp.local_port = local_port;
                            tcp.is_connecting = true;
                            if let Err(e) = socket.connect(crate::net::smoltcp::SMOLTCP_IFACE.lock().as_mut().unwrap().context(), (dest_addr, dest_port), local_port) { crate::console::write_str(alloc::format!("[SYS_CONNECT] connect err: {:?}\n", e).as_str()); }
                            Err("WouldBlock")
                        }
                    },
                    smoltcp::socket::tcp::State::Established => {
                        tcp.is_connecting = false;
                        Ok(())
                    },
                    state => { crate::console::write_str(alloc::format!("[SYS_CONNECT] state: {:?}\n", state).as_str()); Err("WouldBlock") },
                }
            }).unwrap_or(Err("Network offline"));
            
            if result.is_err() {
                tcp.waker_task_id = Some(task_id);
            }
            result
        }
    }
}

pub fn sys_send(socket_id: usize, data: &[u8]) -> Result<usize, &'static str> {
    let table = SOCKET_TABLE.lock();
    let sock = table.get_socket(socket_id).ok_or("Invalid socket ID")?;
    match sock {
        Socket::SmolTcp(tcp_mutex) => {
            let tcp = tcp_mutex.lock();
            let handle = tcp.handle;
            crate::net::smoltcp::try_with_smoltcp_sockets(|sockets| {
                let socket = sockets.get_mut::<smoltcp::socket::tcp::Socket>(handle);
                if socket.can_send() {
                    match socket.send_slice(data) {
                        Ok(len) => Ok(len),
                        Err(_) => Err("Send failed"),
                    }
                } else {
                    Err("Cannot send")
                }
            }).unwrap_or(Err("Network offline"))
        }
    }
}

pub fn sys_recv(socket_id: usize, buf: &mut [u8], task_id: usize) -> Result<RecvResult, &'static str> {
    let table = SOCKET_TABLE.lock();
    let sock = table.get_socket(socket_id).ok_or("Invalid socket ID")?;
    match sock {
        Socket::SmolTcp(tcp_mutex) => {
            let mut tcp = tcp_mutex.lock();
            let handle = tcp.handle;
            let result = crate::net::smoltcp::try_with_smoltcp_sockets(|sockets| {
                let socket = sockets.get_mut::<smoltcp::socket::tcp::Socket>(handle);
                if socket.can_recv() {
                    match socket.recv_slice(buf) {
                        Ok(len) if len > 0 => {
                            let endpt = socket.remote_endpoint().unwrap();
                            let smoltcp::wire::IpAddress::Ipv4(v4) = endpt.addr;
                            let b = v4.octets();
                            let src_ip = ((b[0] as u32) << 24) | ((b[1] as u32) << 16) | ((b[2] as u32) << 8) | (b[3] as u32);
                            Ok(RecvResult::Data { src_ip, src_port: endpt.port, len })
                        }
                        _ => Err("WouldBlock")
                    }
                } else if !socket.may_recv() {
                    // EOF
                    Ok(RecvResult::Data { src_ip: 0, src_port: 0, len: 0 })
                } else {
                    Err("WouldBlock")
                }
            }).unwrap_or(Err("Network offline"));
            
            match result {
                Ok(r) => {
                    tcp.waker_task_id = None;
                    Ok(r)
                },
                Err(e) if e == "WouldBlock" => {
                    tcp.waker_task_id = Some(task_id);
                    Ok(RecvResult::WouldBlock)
                },
                Err(e) => Err(e),
            }
        }
    }
}

pub fn wake_smoltcp_sockets() {
    if let Some(table) = SOCKET_TABLE.try_lock() {
        for slot in &table.sockets {
            if let Some(Socket::SmolTcp(tcp_mutex)) = slot {
                if let Some(mut tcp) = tcp_mutex.try_lock() {
                    if let Some(task_id) = tcp.waker_task_id {
                        let should_wake = crate::net::smoltcp::try_with_smoltcp_sockets(|sockets| {
                            let socket = sockets.get_mut::<smoltcp::socket::tcp::Socket>(tcp.handle);
                            socket.can_recv() || socket.can_send() || !socket.is_open() || !socket.may_recv()
                        }).unwrap_or(false);
                        if should_wake {
                            tcp.waker_task_id = None;
                            crate::task::scheduler::SCHEDULER.lock().set_task_state(task_id, crate::task::TaskState::Ready);
                        }
                    }
                }
            }
        }
    }
}
