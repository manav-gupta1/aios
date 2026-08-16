use x86_64::instructions::port::Port;

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

/// Reads a 32-bit value from the PCI configuration space.
/// 
/// `offset` must be 4-byte aligned (bits 0 and 1 are ignored by the hardware).
pub fn read_config_dword(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let address: u32 = 0x8000_0000
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC);

    let mut address_port = Port::<u32>::new(CONFIG_ADDRESS);
    let mut data_port = Port::<u32>::new(CONFIG_DATA);

    unsafe {
        address_port.write(address);
        data_port.read()
    }
}

pub fn write_config_dword(bus: u8, device: u8, function: u8, offset: u8, value: u32) {
    let address: u32 = 0x8000_0000
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xFC);

    let mut address_port = Port::<u32>::new(CONFIG_ADDRESS);
    let mut data_port = Port::<u32>::new(CONFIG_DATA);

    unsafe {
        address_port.write(address);
        data_port.write(value);
    }
}

pub fn read_config_word(bus: u8, device: u8, function: u8, offset: u8) -> u16 {
    let dword = read_config_dword(bus, device, function, offset);
    ((dword >> ((offset & 2) * 8)) & 0xFFFF) as u16
}

pub fn write_config_word(bus: u8, device: u8, function: u8, offset: u8, value: u16) {
    let dword = read_config_dword(bus, device, function, offset);
    let shift = (offset & 2) * 8;
    let new_dword = (dword & !(0xFFFF << shift)) | ((value as u32) << shift);
    write_config_dword(bus, device, function, offset, new_dword);
}

pub fn read_config_byte(bus: u8, device: u8, function: u8, offset: u8) -> u8 {
    let dword = read_config_dword(bus, device, function, offset);
    ((dword >> ((offset & 3) * 8)) & 0xFF) as u8
}

pub fn write_config_byte(bus: u8, device: u8, function: u8, offset: u8, value: u8) {
    let dword = read_config_dword(bus, device, function, offset);
    let shift = (offset & 3) * 8;
    let new_dword = (dword & !(0xFF << shift)) | ((value as u32) << shift);
    write_config_dword(bus, device, function, offset, new_dword);
}
