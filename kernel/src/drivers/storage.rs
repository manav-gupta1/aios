use alloc::boxed::Box;
use spin::Mutex;
use crate::fs::ata::AtaPio;

pub trait BlockDevice {
    fn read_sector(&self, lba: u32, buf: &mut [u8]) -> Result<(), ()>;
    fn write_sector(&self, lba: u32, buf: &[u8]) -> Result<(), ()>;
    fn sector_size(&self) -> usize;
    fn capacity_sectors(&self) -> u64;
    fn backend_name(&self) -> &'static str;
}

pub struct AtaBackend;

impl BlockDevice for AtaBackend {
    fn read_sector(&self, lba: u32, buf: &mut [u8]) -> Result<(), ()> {
        if buf.len() != crate::fs::ata::SECTOR_SIZE { return Err(()); }
        let arr = unsafe { &mut *(buf.as_mut_ptr() as *mut [u8; 512]) };
        AtaPio::read_sector(lba, arr)
    }

    fn write_sector(&self, lba: u32, buf: &[u8]) -> Result<(), ()> {
        if buf.len() != crate::fs::ata::SECTOR_SIZE { return Err(()); }
        let arr = unsafe { &*(buf.as_ptr() as *const [u8; 512]) };
        AtaPio::write_sector(lba, arr)
    }

    fn sector_size(&self) -> usize {
        512
    }

    fn capacity_sectors(&self) -> u64 {
        0 // ATA PIO driver currently doesn't fetch capacity
    }
    
    fn backend_name(&self) -> &'static str {
        "ATA PIO"
    }
}

pub static STORAGE_DEVICE: Mutex<Option<Box<dyn BlockDevice + Send>>> = Mutex::new(None);

pub fn serial_print(s: &str) {
    let mut port = x86_64::instructions::port::Port::<u8>::new(0x3F8);
    for b in s.bytes() {
        unsafe { port.write(b) };
    }
}

pub fn init(text: &mut crate::graphics::text::TextWriter) {
    let pci_devices = crate::drivers::pci::device::enumerate_devices();
    
    serial_print("[VIRTIO] init start\n");

    // First try to find a VirtIO Block device
    for dev in pci_devices {
        if dev.vendor_id == 0x1AF4 && (dev.device_id == 0x1001 || dev.device_id == 0x1042) {
            serial_print("[VIRTIO] device found\n");
            if let Ok(virtio) = crate::drivers::virtio::block::VirtioBlock::new(dev.bus, dev.device, dev.function, text) {
                *STORAGE_DEVICE.lock() = Some(Box::new(spin::Mutex::new(virtio)));
                serial_print("[VIRTIO] registered\n");
                return;
            }
        }
    }
    
    serial_print("[VIRTIO] not found, fallback to ATA\n");
    // Fallback to ATA PIO
    if AtaPio::is_available() {
        *STORAGE_DEVICE.lock() = Some(Box::new(AtaBackend));
    }
}
