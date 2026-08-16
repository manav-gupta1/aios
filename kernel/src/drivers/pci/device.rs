use alloc::vec::Vec;
use super::config::read_config_dword;

#[derive(Debug, Clone)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub revision: u8,
    pub header_type: u8,
}

pub fn enumerate_devices() -> Vec<PciDevice> {
    let mut devices = Vec::new();

    for bus in 0..=255 {
        for device in 0..32 {
            let vendor_id = get_vendor_id(bus, device, 0);
            if vendor_id == 0xFFFF {
                continue; // Device does not exist
            }

            check_device(bus, device, &mut devices);
        }
    }

    devices
}

fn check_device(bus: u8, device: u8, devices: &mut Vec<PciDevice>) {
    let header_type = get_header_type(bus, device, 0);
    // If bit 7 of the header type is set, it is a multi-function device.
    let functions = if (header_type & 0x80) != 0 { 8 } else { 1 };

    for function in 0..functions {
        if get_vendor_id(bus, device, function) != 0xFFFF {
            let dev = read_device_info(bus, device, function);
            devices.push(dev);
        }
    }
}

fn get_vendor_id(bus: u8, device: u8, function: u8) -> u16 {
    (read_config_dword(bus, device, function, 0) & 0xFFFF) as u16
}

fn get_header_type(bus: u8, device: u8, function: u8) -> u8 {
    ((read_config_dword(bus, device, function, 0x0C) >> 16) & 0xFF) as u8
}

fn read_device_info(bus: u8, device: u8, function: u8) -> PciDevice {
    let word0 = read_config_dword(bus, device, function, 0x00);
    let word2 = read_config_dword(bus, device, function, 0x08);
    let word3 = read_config_dword(bus, device, function, 0x0C);

    PciDevice {
        bus,
        device,
        function,
        vendor_id: (word0 & 0xFFFF) as u16,
        device_id: ((word0 >> 16) & 0xFFFF) as u16,
        revision: (word2 & 0xFF) as u8,
        prog_if: ((word2 >> 8) & 0xFF) as u8,
        subclass: ((word2 >> 16) & 0xFF) as u8,
        class_code: ((word2 >> 24) & 0xFF) as u8,
        header_type: ((word3 >> 16) & 0xFF) as u8,
    }
}
