use alloc::vec::Vec;

pub const ETHERTYPE_IPv4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;

pub const BROADCAST_MAC: [u8; 6] = [0xFF; 6];

#[repr(C, packed)]
pub struct EthernetHeader {
    pub dest_mac: [u8; 6],
    pub src_mac: [u8; 6],
    pub ethertype: u16,
}

pub fn handle_ethernet_frame(packet: &[u8]) {
    if packet.len() < core::mem::size_of::<EthernetHeader>() {
        return; // Malformed / too short
    }
    
    let header = unsafe { &*(packet.as_ptr() as *const EthernetHeader) };
    let ethertype = crate::net::ntohs(header.ethertype);
    let payload = &packet[core::mem::size_of::<EthernetHeader>()..];
    
    match ethertype {
        ETHERTYPE_ARP => {
            crate::net::arp::handle_arp_packet(payload);
        }
        ETHERTYPE_IPv4 => {
            crate::net::ipv4::handle_ipv4_packet(payload);
        }
        _ => {
            // Ignore unknown ethertypes
        }
    }
}

pub fn send_ethernet_frame(dest_mac: [u8; 6], ethertype: u16, payload: &[u8]) -> Result<(), &'static str> {
    let header = EthernetHeader {
        dest_mac,
        // We will fill src_mac inside the lock
        src_mac: [0; 6],
        ethertype: crate::net::htons(ethertype),
    };
    
    let res = x86_64::instructions::interrupts::without_interrupts(|| {
        let mut dev_lock = crate::drivers::network::NETWORK_DEVICE.lock();
        if let Some(dev) = dev_lock.as_mut() {
            let mut hdr = header;
            hdr.src_mac = dev.mac_address();
            let header_bytes = unsafe {
                core::slice::from_raw_parts(
                    &hdr as *const EthernetHeader as *const u8,
                    core::mem::size_of::<EthernetHeader>()
                )
            };
            
            let mut frame = Vec::with_capacity(60.max(14 + payload.len()));
            frame.extend_from_slice(header_bytes);
            frame.extend_from_slice(payload);
            
            // Pad to minimum ethernet frame size (60 bytes)
            while frame.len() < 60 {
                frame.push(0);
            }
            
            dev.send_packet(&frame)
        } else {
            Err("No network device")
        }
    });
    res
}
