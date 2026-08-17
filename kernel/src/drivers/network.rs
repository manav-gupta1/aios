use alloc::boxed::Box;
use spin::Mutex;

pub trait NetworkDevice {
    fn mac_address(&self) -> [u8; 6];
    fn is_up(&self) -> bool;
    fn send_packet(&mut self, data: &[u8]) -> Result<(), &'static str>;
    fn receive_packet(&mut self) -> Option<alloc::vec::Vec<u8>>;
    fn ack_interrupt(&mut self) -> bool;
    fn rx_packets(&self) -> u64;
    fn tx_packets(&self) -> u64;
    fn rx_bytes(&self) -> u64;
    fn tx_bytes(&self) -> u64;
}

pub static NETWORK_DEVICE: Mutex<Option<Box<dyn NetworkDevice + Send>>> = Mutex::new(None);

pub fn init(text: &mut crate::graphics::text::TextWriter) {
    let pci_devices = crate::drivers::pci::device::enumerate_devices();
    
    crate::drivers::storage::serial_print("[NET] init start\n");

    for dev in pci_devices {
        if dev.vendor_id == 0x1AF4 && (dev.device_id == 0x1000 || dev.device_id == 0x1041) {
            crate::drivers::storage::serial_print("[NET] VirtIO network device found\n");
            match crate::drivers::virtio::net::VirtioNet::new(dev.bus, dev.device, dev.function, text) {
                Ok(virtio) => {
                    *NETWORK_DEVICE.lock() = Some(Box::new(virtio));
                    crate::drivers::storage::serial_print("[NET] VirtIO network registered\n");
                }
                Err(_e) => {
                    crate::drivers::storage::serial_print("[NET] VirtIO network init failed\n");
                }
            }
            return;
        }
    }
    
    crate::drivers::storage::serial_print("[NET] no network device found\n");
}
