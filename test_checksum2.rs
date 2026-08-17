fn calculate_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i < data.len() {
        let word = if i + 1 < data.len() {
            ((data[i] as u32) << 8) | (data[i+1] as u32)
        } else {
            (data[i] as u32) << 8
        };
        sum = sum.wrapping_add(word);
        i += 2;
    }
    
    while (sum >> 16) > 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    
    !(sum as u16)
}

fn htons(v: u16) -> u16 {
    v.to_be()
}

fn htonl(v: u32) -> u32 {
    v.to_be()
}

#[repr(C, packed)]
struct TcpPseudoHeader {
    pub src_ip: u32,
    pub dest_ip: u32,
    pub reserved: u8,
    pub protocol: u8,
    pub tcp_length: u16,
}

#[repr(C, packed)]
struct TcpHeader {
    pub src_port: u16,
    pub dest_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset_flags: u16,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_pointer: u16,
}

fn ip(a: u8, b: u8, c: u8, d: u8) -> u32 {
    ((a as u32) << 24) | ((b as u32) << 16) | ((c as u32) << 8) | (d as u32)
}

fn main() {
    let local_ip = ip(10, 0, 2, 15);
    let dest_ip = ip(104, 20, 23, 154);
    let payload = b"GET / HTTP/1.1\r\nHost: www.example.com\r\nConnection: close\r\n\r\n";
    
    let header_len = core::mem::size_of::<TcpHeader>();
    let total_len = header_len + payload.len();
    
    let mut packet = vec![0u8; total_len];
    packet[header_len..].copy_from_slice(payload);
    
    let data_offset = 5u16;
    let flags = 0x18; // ACK | PSH
    let data_offset_flags = (data_offset << 12) | flags;
    
    let header = unsafe { &mut *(packet.as_mut_ptr() as *mut TcpHeader) };
    header.src_port = htons(49153);
    header.dest_port = htons(80);
    header.seq_num = htonl(12345679);
    header.ack_num = htonl(42);
    header.data_offset_flags = htons(data_offset_flags);
    header.window_size = htons(8192);
    header.checksum = 0;
    header.urgent_pointer = 0;
    
    let pseudo_header = TcpPseudoHeader {
        src_ip: htonl(local_ip),
        dest_ip: htonl(dest_ip),
        reserved: 0,
        protocol: 6,
        tcp_length: htons(total_len as u16),
    };
    
    let ph_bytes = unsafe {
        core::slice::from_raw_parts(
            &pseudo_header as *const _ as *const u8,
            core::mem::size_of::<TcpPseudoHeader>(),
        )
    };
    
    let mut checksum_data = Vec::new();
    checksum_data.extend_from_slice(ph_bytes);
    checksum_data.extend_from_slice(&packet);
    
    let checksum = calculate_checksum(&checksum_data);
    header.checksum = htons(checksum);
    
    println!("Checksum: {:04x}", checksum);
    println!("Packet: {:02x?}", packet);
}
