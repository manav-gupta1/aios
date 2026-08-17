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

fn main() {
    let data = [0x45, 0x00, 0x00, 0x3c, 0x1c, 0x46, 0x40, 0x00, 0x40, 0x06, 0x00, 0x00, 0xac, 0x10, 0x0a, 0x63, 0xac, 0x10, 0x0a, 0x0c];
    let mut pkt = data.to_vec();
    let cs = calculate_checksum(&pkt);
    pkt[10] = (htons(cs) & 0xff) as u8;
    pkt[11] = (htons(cs) >> 8) as u8;
    println!("CS = {:x}, htons = {:x}", cs, htons(cs));
}
