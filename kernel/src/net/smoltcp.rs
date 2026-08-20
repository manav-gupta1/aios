#![allow(clippy::collapsible_if)]
use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::collections::VecDeque;
use spin::Mutex;

use crate::drivers::network::NetworkDevice;
use crate::drivers::timer::TimerDriver;

use smoltcp::time::Instant;
use smoltcp::phy::{Device, DeviceCapabilities, Medium};
use smoltcp::wire::{EthernetAddress, Ipv4Address, IpCidr, HardwareAddress, IpAddress};
use smoltcp::iface::{Interface, Config, SocketSet};

const SMOLTCP_MTU: usize = 1500;

pub struct RxToken {
    buffer: Vec<u8>,
}

impl smoltcp::phy::RxToken for RxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.buffer)
    }
}

pub struct TxToken<'a> {
    dev: &'a mut Box<dyn NetworkDevice + Send>,
}

impl<'a> smoltcp::phy::TxToken for TxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = alloc::vec![0; len];
        let result = f(&mut buffer);
        
        // Pad frame to 60 bytes minimum required by Ethernet/QEMU
        if buffer.len() < 60 {
            buffer.resize(60, 0);
        }
        if !buffer.len().is_multiple_of(2) {
            buffer.push(0);
        }
        crate::drivers::storage::serial_print(&alloc::format!("[SMOLTCP TX] buffer hex: {:02x?}\n", &buffer[..core::cmp::min(buffer.len(), 20)])); let _ = self.dev.send_packet(&buffer);
        result
    }
}

pub struct VirtioNetAdapter<'a> { 
    pub dev: &'a mut Box<dyn NetworkDevice + Send> 
}

impl<'a> Device for VirtioNetAdapter<'a> {
    type RxToken<'b> = RxToken where Self: 'b;
    type TxToken<'b> = TxToken<'b> where Self: 'b;

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = SMOLTCP_MTU;
        caps.medium = Medium::Ethernet;
        caps
    }

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        if let Some(packet) = SMOLTCP_RX_QUEUE.lock().pop_front() {
            let rx = RxToken { buffer: packet };
            let tx = TxToken { dev: self.dev };
            Some((rx, tx))
        } else {
            None
        }
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(TxToken { dev: self.dev })
    }
}

pub static SMOLTCP_IFACE: Mutex<Option<Interface>> = Mutex::new(None);
pub static SMOLTCP_SOCKETS: Mutex<Option<SocketSet<'static>>> = Mutex::new(None);
pub static SMOLTCP_RX_QUEUE: Mutex<VecDeque<Vec<u8>>> = Mutex::new(VecDeque::new());
pub static SMOLTCP_DNS_SOCKET_HANDLE: Mutex<Option<smoltcp::iface::SocketHandle>> = Mutex::new(None);
pub static SMOLTCP_ICMP_SOCKET_HANDLE: Mutex<Option<smoltcp::iface::SocketHandle>> = Mutex::new(None);

fn smoltcp_timestamp() -> Instant {
    // ticks are 10ms each (100 Hz)
    Instant::from_millis((TimerDriver::get_ticks() as i64) * 10)
}

pub fn init_smoltcp() {
    let mut net_dev_guard = crate::drivers::network::NETWORK_DEVICE.lock();
    if let Some(ref mut dev) = *net_dev_guard {
        let mac_bytes = dev.mac_address();
        let hardware_addr = HardwareAddress::Ethernet(EthernetAddress(mac_bytes));
        
        let mut config = Config::new(hardware_addr);
        config.random_seed = unsafe { core::arch::x86_64::_rdtsc() };

        let mut adapter = VirtioNetAdapter { dev };
        let mut iface = Interface::new(config, &mut adapter, smoltcp_timestamp());

        // Interface + SocketSet with 10.0.2.15/24, gateway 10.0.2.2, DNS 10.0.2.3
        iface.update_ip_addrs(|ip_addrs| {
            let _ = ip_addrs.push(IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24));
        });

        iface.routes_mut().add_default_ipv4_route(Ipv4Address::new(10, 0, 2, 2)).unwrap();

        let mut sockets = SocketSet::new(alloc::vec![]);
        
        let dns_servers = &[IpAddress::v4(10, 0, 2, 3)];
        let dns_socket = smoltcp::socket::dns::Socket::new(dns_servers, alloc::vec![]);
        let dns_handle = sockets.add(dns_socket);
        *SMOLTCP_DNS_SOCKET_HANDLE.lock() = Some(dns_handle);

        let icmp_rx_meta = [smoltcp::socket::icmp::PacketMetadata::EMPTY; 4];
        let icmp_tx_meta = [smoltcp::socket::icmp::PacketMetadata::EMPTY; 4];
        let icmp_rx_buffer = smoltcp::socket::icmp::PacketBuffer::new(alloc::vec::Vec::from(icmp_rx_meta), alloc::vec![0; 1024]);
        let icmp_tx_buffer = smoltcp::socket::icmp::PacketBuffer::new(alloc::vec::Vec::from(icmp_tx_meta), alloc::vec![0; 1024]);
        let mut icmp_socket = smoltcp::socket::icmp::Socket::new(icmp_rx_buffer, icmp_tx_buffer);
        icmp_socket.bind(smoltcp::socket::icmp::Endpoint::Ident(0x1234)).unwrap();
        let icmp_handle = sockets.add(icmp_socket);
        *SMOLTCP_ICMP_SOCKET_HANDLE.lock() = Some(icmp_handle);

        *SMOLTCP_IFACE.lock() = Some(iface);
        *SMOLTCP_SOCKETS.lock() = Some(sockets);
        
        drop(net_dev_guard); // Drop lock before calling poll_smoltcp
        
        // Initial poll
        poll_smoltcp();
        crate::drivers::storage::serial_print("[SMOLTCP] Initialized with 10.0.2.15/24 and DNS 10.0.2.3\n");
    } else {
        crate::drivers::storage::serial_print("[SMOLTCP] Failed to initialize: no network device\n");
    }
}

pub fn poll_smoltcp() {
    let mut polled = false;
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut iface_guard = SMOLTCP_IFACE.lock();
        let mut sockets_guard = SMOLTCP_SOCKETS.lock();
        let mut net_dev_guard = crate::drivers::network::NETWORK_DEVICE.lock();
        
        if let (Some(iface), Some(sockets), Some(dev)) = (iface_guard.as_mut(), sockets_guard.as_mut(), net_dev_guard.as_mut()) {
            let mut adapter = VirtioNetAdapter { dev };
            let _ = iface.poll(smoltcp_timestamp(), &mut adapter, sockets);
            polled = true;
        }
    });
    
    if polled {
        crate::net::socket::wake_smoltcp_sockets();
    }
}

pub fn with_smoltcp_sockets<F, R>(f: F) -> R
where F: FnOnce(&mut smoltcp::iface::SocketSet<'static>) -> R
{
    let mut sockets = SMOLTCP_SOCKETS.lock();
    f(sockets.as_mut().unwrap())
}

pub fn with_smoltcp_context<F, R>(f: F) -> R
where F: FnOnce(&mut smoltcp::iface::Context) -> R
{
    let mut iface = SMOLTCP_IFACE.lock();
    f(iface.as_mut().unwrap().context())
}

pub fn try_with_smoltcp_sockets<F, R>(f: F) -> Option<R>
where F: FnOnce(&mut smoltcp::iface::SocketSet<'static>) -> R
{
    SMOLTCP_SOCKETS.try_lock().map(|mut sockets| f(sockets.as_mut().unwrap()))
}

pub fn smoltcp_ping_send(dest_ip: u32, ident: u16, sequence: u16, payload: &[u8]) -> Result<(), &'static str> {
    with_smoltcp_sockets(|sockets| {
        let handle = SMOLTCP_ICMP_SOCKET_HANDLE.lock().unwrap();
        let socket = sockets.get_mut::<smoltcp::socket::icmp::Socket>(handle);
        
        let mut icmp_buf = alloc::vec![0u8; 8 + payload.len()];
        let mut icmp_packet = smoltcp::wire::Icmpv4Packet::new_unchecked(&mut icmp_buf);
        let icmp_repr = smoltcp::wire::Icmpv4Repr::EchoRequest {
            ident,
            seq_no: sequence,
            data: payload,
        };
        icmp_repr.emit(&mut icmp_packet, &smoltcp::phy::ChecksumCapabilities::default());
        
        let ip_addr = smoltcp::wire::IpAddress::v4(
            (dest_ip >> 24) as u8,
            (dest_ip >> 16) as u8,
            (dest_ip >> 8) as u8,
            dest_ip as u8,
        );
        
        match socket.send_slice(&icmp_buf, ip_addr) {
            Ok(_) => {
                crate::drivers::storage::serial_print("[SMOLTCP ICMP] ping sent\n");
                Ok(())
            }
            Err(_) => {
                crate::drivers::storage::serial_print("[SMOLTCP ICMP] ping send error (buffer full)\n");
                Err("ICMP buffer full")
            }
        }
    })
}

pub fn smoltcp_ping_recv(expected_ident: u16) -> Option<(u32, u16, u16)> {
    with_smoltcp_sockets(|sockets| {
        let handle = SMOLTCP_ICMP_SOCKET_HANDLE.lock().unwrap();
        let socket = sockets.get_mut::<smoltcp::socket::icmp::Socket>(handle);
        
        if let Ok((payload, src_addr)) = socket.recv() {
            if let Ok(icmp_packet) = smoltcp::wire::Icmpv4Packet::new_checked(payload) {
                if let Ok(smoltcp::wire::Icmpv4Repr::EchoReply { ident, seq_no, .. }) = 
                    smoltcp::wire::Icmpv4Repr::parse(&icmp_packet, &smoltcp::phy::ChecksumCapabilities::default()) 
                {
                    if ident == expected_ident {
                        #[allow(irrefutable_let_patterns)]
                        if let smoltcp::wire::IpAddress::Ipv4(v4) = src_addr {
                            let bytes = v4.octets();
                            let src_ip = ((bytes[0] as u32) << 24) | ((bytes[1] as u32) << 16) | ((bytes[2] as u32) << 8) | (bytes[3] as u32);
                            return Some((src_ip, ident, seq_no));
                        }
                    }
                }
            }
        }
        None
    })
}
