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

    // SAFETY: We are performing a read-only legacy PCI configuration space access.
    // This is safe provided we only use it for probing and do not disrupt active hardware.
    unsafe {
        address_port.write(address);
        data_port.read()
    }
}
