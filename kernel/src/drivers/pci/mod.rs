pub mod config;
pub mod device;

use alloc::string::String;

pub use device::{enumerate_devices, PciDevice};

/// Returns a human-readable name for a given PCI class and subclass.
pub fn pci_class_name(class: u8, subclass: u8) -> String {
    match class {
        0x00 => String::from("Unclassified"),
        0x01 => match subclass {
            0x01 => String::from("IDE Controller"),
            0x06 => String::from("SATA Controller"),
            0x08 => String::from("NVMe Controller"),
            _ => alloc::format!("Mass Storage Controller ({:02X})", subclass),
        },
        0x02 => match subclass {
            0x00 => String::from("Ethernet Controller"),
            _ => alloc::format!("Network Controller ({:02X})", subclass),
        },
        0x03 => match subclass {
            0x00 => String::from("VGA Controller"),
            _ => alloc::format!("Display Controller ({:02X})", subclass),
        },
        0x04 => String::from("Multimedia Controller"),
        0x05 => String::from("Memory Controller"),
        0x06 => match subclass {
            0x00 => String::from("Host Bridge"),
            0x01 => String::from("ISA Bridge"),
            0x04 => String::from("PCI-to-PCI Bridge"),
            _ => alloc::format!("Bridge ({:02X})", subclass),
        },
        0x07 => match subclass {
            0x00 => String::from("Serial Controller"),
            _ => String::from("Simple Communication Controller"),
        },
        0x08 => String::from("Base System Peripheral"),
        0x09 => String::from("Input Device Controller"),
        0x0C => match subclass {
            0x03 => String::from("USB Controller"),
            _ => alloc::format!("Serial Bus Controller ({:02X})", subclass),
        },
        _ => alloc::format!("Class {:02X} Subclass {:02X}", class, subclass),
    }
}
