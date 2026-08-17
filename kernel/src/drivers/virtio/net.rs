use alloc::vec::Vec;
use core::sync::atomic::{compiler_fence, Ordering};
use x86_64::instructions::port::Port;
use crate::drivers::virtio::virtq::*;
use crate::drivers::network::NetworkDevice;

// VirtIO Net device config offset 0 contains the MAC address
const REG_MAC: u16 = 0;

pub struct VirtioNet {
    io_base: u16,
    irq: u8,
    mac: [u8; 6],
    
    // RX Queue
    rx_desc: *mut VirtqDesc,
    rx_avail: *mut VirtqAvail,
    rx_used: *mut VirtqUsed,
    rx_last_used_idx: u16,
    rx_buffers: *mut u8,
    
    // TX Queue
    tx_desc: *mut VirtqDesc,
    tx_avail: *mut VirtqAvail,
    tx_used: *mut VirtqUsed,
    tx_avail_idx: u16,
    tx_buffer: *mut u8,
    
    // Stats
    rx_pkts: u64,
    tx_pkts: u64,
    rx_bytes: u64,
    tx_bytes: u64,
}

unsafe impl Send for VirtioNet {}
unsafe impl Sync for VirtioNet {}

impl VirtioNet {
    pub fn new(bus: u8, device: u8, function: u8, _text: &mut crate::graphics::text::TextWriter) -> Result<Self, &'static str> {
        use crate::drivers::pci::config::*;
        
        let vendor = read_config_word(bus, device, function, 0x00);
        let device_id = read_config_word(bus, device, function, 0x02);
        
        if vendor != 0x1AF4 || (device_id != 0x1000 && device_id != 0x1041) {
            return Err("Not a VirtIO Network device");
        }
        
        let bar0 = read_config_dword(bus, device, function, 0x10);
        if (bar0 & 1) == 0 {
            return Err("BAR0 is not I/O space");
        }
        let io_base = (bar0 & !3) as u16;
        let irq = read_config_byte(bus, device, function, 0x3C);
        
        crate::drivers::storage::serial_print(&alloc::format!("[VIRTIO-NET] BAR0 and IRQ {} found\n", irq));
        
        // Reset device
        unsafe { Port::<u8>::new(io_base + REG_DEVICE_STATUS).write(0) };
        
        let mut status = STATUS_ACKNOWLEDGE | STATUS_DRIVER;
        unsafe { Port::<u8>::new(io_base + REG_DEVICE_STATUS).write(status) };
        
        // Negotiate features
        let features = unsafe { Port::<u32>::new(io_base + REG_DEVICE_FEATURES).read() };
        crate::drivers::storage::serial_print(&alloc::format!("VirtIO Net Features: {:#x}\n", features));
        
        // We only support VIRTIO_NET_F_MAC (bit 5)
        unsafe { Port::<u32>::new(io_base + REG_GUEST_FEATURES).write(1 << 5) };
        
        // Read MAC address
        let mut mac = [0u8; 6];
        for i in 0..6 {
            mac[i] = unsafe { Port::<u8>::new(io_base + REG_CONFIG + 0 + i as u16).read() };
        }
        crate::drivers::storage::serial_print(&alloc::format!("VirtIO-Net MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}\n", mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]));
        
        // Setup RX Queue (Queue 0)
        unsafe { Port::<u16>::new(io_base + REG_QUEUE_SELECT).write(0) };
        let rx_queue_size = unsafe { Port::<u16>::new(io_base + REG_QUEUE_SIZE).read() };
        if rx_queue_size == 0 || rx_queue_size > 256 {
            return Err("Unsupported RX queue size");
        }
        
        // Setup TX Queue (Queue 1)
        unsafe { Port::<u16>::new(io_base + REG_QUEUE_SELECT).write(1) };
        let tx_queue_size = unsafe { Port::<u16>::new(io_base + REG_QUEUE_SIZE).read() };
        if tx_queue_size == 0 || tx_queue_size > 256 {
            return Err("Unsupported TX queue size");
        }
        
        crate::drivers::storage::serial_print(&alloc::format!("VirtIO Net RX size: {}, TX size: {}\n", rx_queue_size, tx_queue_size));
        
        // Allocate contiguous physical frames for virtqueues.
        // We need 198 pages (811,008 bytes) for RX and TX queues and buffers.
        let phys_frame = crate::memory::allocate_contiguous_frames(198).ok_or("Failed to alloc frames")?;
        let phys_addr = phys_frame.start_address().as_u64();
        let phys_offset = crate::memory::get_phys_offset().unwrap();
        let virt_addr = phys_addr + phys_offset;
        
        // Zero out memory
        unsafe { core::ptr::write_bytes(virt_addr as *mut u8, 0, 198 * 4096) };
        
        // RX Queue structures
        let rx_desc = virt_addr as *mut VirtqDesc;
        let rx_avail = (virt_addr + 4096) as *mut VirtqAvail;
        let rx_used = (virt_addr + 8192) as *mut VirtqUsed;
        
        // TX Queue structures
        let tx_desc = (virt_addr + 12288) as *mut VirtqDesc;
        let tx_avail = (virt_addr + 16384) as *mut VirtqAvail;
        let tx_used = (virt_addr + 20480) as *mut VirtqUsed;
        
        // Buffers
        let rx_buffers = (virt_addr + 24576) as *mut u8;
        let tx_buffer = (virt_addr + 548864) as *mut u8;
        
        // Configure RX Queue
        let rx_pfn = (phys_addr / 4096) as u32;
        unsafe {
            Port::<u16>::new(io_base + REG_QUEUE_SELECT).write(0);
            Port::<u32>::new(io_base + REG_QUEUE_ADDRESS).write(rx_pfn);
        }
        
        // Configure TX Queue
        let tx_pfn = ((phys_addr + 12288) / 4096) as u32;
        unsafe {
            Port::<u16>::new(io_base + REG_QUEUE_SELECT).write(1);
            Port::<u32>::new(io_base + REG_QUEUE_ADDRESS).write(tx_pfn);
        }
        
        // Pre-fill RX queue with 128 packets (chained descriptors: hdr + data)
        for i in 0..128 {
            let hdr_idx = i * 2;
            let data_idx = i * 2 + 1;
            
            let buf_phys_addr = phys_addr + 24576 + (i as u64 * 2048);
            unsafe {
                // Header descriptor
                (*rx_desc.add(hdr_idx)).addr = buf_phys_addr;
                (*rx_desc.add(hdr_idx)).len = 10; // VirtioNetHdr
                (*rx_desc.add(hdr_idx)).flags = VIRTQ_DESC_F_WRITE | 1; // 1 = VIRTQ_DESC_F_NEXT
                (*rx_desc.add(hdr_idx)).next = data_idx as u16;
                
                // Data descriptor
                (*rx_desc.add(data_idx)).addr = buf_phys_addr + 10;
                (*rx_desc.add(data_idx)).len = 2038;
                (*rx_desc.add(data_idx)).flags = VIRTQ_DESC_F_WRITE;
                
                // Put head in avail ring
                (*rx_avail).ring[i] = hdr_idx as u16;
            }
        }
        unsafe {
            (*rx_avail).idx = 128;
            compiler_fence(Ordering::SeqCst);
        }
        
        // Notify device that RX buffers are available
        unsafe { Port::<u16>::new(io_base + REG_QUEUE_NOTIFY).write(0) };
        
        // Tell device we are ready
        status |= STATUS_DRIVER_OK;
        unsafe { Port::<u8>::new(io_base + REG_DEVICE_STATUS).write(status) };
        
        crate::interrupts::unmask_irq(irq);
        
        Ok(Self {
            io_base,
            irq,
            mac,
            rx_desc,
            rx_avail,
            rx_used,
            rx_last_used_idx: 0,
            rx_buffers,
            tx_desc,
            tx_avail,
            tx_used,
            tx_avail_idx: 0,
            tx_buffer,
            rx_pkts: 0,
            tx_pkts: 0,
            rx_bytes: 0,
            tx_bytes: 0,
        })
    }
}

// VirtIO Net packet header (10 bytes for legacy VirtIO)
#[repr(C)]
struct VirtioNetHdr {
    flags: u8,
    gso_type: u8,
    hdr_len: u16,
    gso_size: u16,
    csum_start: u16,
    csum_offset: u16,
}

impl NetworkDevice for VirtioNet {
    fn mac_address(&self) -> [u8; 6] {
        self.mac
    }
    
    fn is_up(&self) -> bool {
        let status = unsafe { Port::<u8>::new(self.io_base + REG_DEVICE_STATUS).read() };
        (status & STATUS_DRIVER_OK) != 0
    }
    
    fn send_packet(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if data.len() > 1514 {
            return Err("Packet too large");
        }
        
        let hdr_size = core::mem::size_of::<VirtioNetHdr>();
        let _total_len = hdr_size + data.len();
        
        // We need a desc index that advances by 2. We'll use self.tx_pkts * 2.
        let base_desc = ((self.tx_pkts * 2) % 256) as usize;
        let hdr_desc_idx = base_desc;
        let data_desc_idx = (base_desc + 1) % 256;
        
        let tx_slot = (self.tx_pkts % 128) as usize;
        let pkt_buffer = unsafe { self.tx_buffer.add(tx_slot * 2048) };
        
        let phys_offset = crate::memory::get_phys_offset().unwrap();
        let hdr_phys_addr = (pkt_buffer as u64) - phys_offset;
        let data_phys_addr = unsafe { (pkt_buffer.add(hdr_size) as u64) - phys_offset };
        
        unsafe {
            // Zero header
            core::ptr::write_bytes(pkt_buffer, 0, hdr_size);
            // Copy packet data
            core::ptr::copy_nonoverlapping(data.as_ptr(), pkt_buffer.add(hdr_size), data.len());
        }
        
        unsafe {
            // Header descriptor
            (*self.tx_desc.add(hdr_desc_idx)).addr = hdr_phys_addr;
            (*self.tx_desc.add(hdr_desc_idx)).len = hdr_size as u32;
            (*self.tx_desc.add(hdr_desc_idx)).flags = 1; // VRING_DESC_F_NEXT
            (*self.tx_desc.add(hdr_desc_idx)).next = data_desc_idx as u16;
            
            // Data descriptor
            (*self.tx_desc.add(data_desc_idx)).addr = data_phys_addr;
            (*self.tx_desc.add(data_desc_idx)).len = data.len() as u32;
            (*self.tx_desc.add(data_desc_idx)).flags = 0; // End of chain
            
            // The avail ring only stores the head descriptor
            let avail_idx = (self.tx_avail_idx % 256) as usize;
            (*self.tx_avail).ring[avail_idx] = hdr_desc_idx as u16;
            
            compiler_fence(Ordering::SeqCst);
            self.tx_avail_idx = self.tx_avail_idx.wrapping_add(1);
            (*self.tx_avail).idx = self.tx_avail_idx;
            
            compiler_fence(Ordering::SeqCst);
            Port::<u16>::new(self.io_base + REG_QUEUE_NOTIFY).write(1);
        }
        
        self.tx_pkts += 1;
        self.tx_bytes += data.len() as u64;
        
        // Wait a tiny bit and check used index
        for _ in 0..1000 {
            core::hint::spin_loop();
        }
        let used_idx = unsafe { (*self.tx_used).idx };
        crate::drivers::storage::serial_print(&alloc::format!("TX Used idx: {}\n", used_idx));
        
        Ok(())
    }
    
    fn receive_packet(&mut self) -> Option<Vec<u8>> {
        let used_idx = unsafe { (*self.rx_used).idx };
        if self.rx_last_used_idx == used_idx {
            return None; // No new packets
        }
        
        compiler_fence(Ordering::SeqCst);
        
        let last_used = (self.rx_last_used_idx % 256) as usize;
        let hdr_idx = unsafe { (*self.rx_used).ring[last_used].id as usize };
        // The total length written includes the header length
        let total_len = unsafe { (*self.rx_used).ring[last_used].len as usize };
        
        let hdr_size = 10; // VirtioNetHdr
        if total_len <= hdr_size {
            self.rx_last_used_idx = self.rx_last_used_idx.wrapping_add(1);
            return None; // Drop invalid packet
        }
        
        let data_len = total_len - hdr_size;
        
        // Recover original buffer address from the header descriptor
        let phys_offset = crate::memory::get_phys_offset().unwrap();
        let buf_phys = unsafe { (*self.rx_desc.add(hdr_idx)).addr };
        let buf_virt = buf_phys + phys_offset;
        
        // Data starts at buf_virt + 10
        let data_ptr = (buf_virt + hdr_size as u64) as *const u8;
        
        let mut packet = Vec::with_capacity(data_len);
        unsafe {
            let slice = core::slice::from_raw_parts(data_ptr, data_len);
            packet.extend_from_slice(slice);
        }
        
        crate::drivers::storage::serial_print(&alloc::format!("RX packet len={}\n", data_len));
        
        self.rx_last_used_idx = self.rx_last_used_idx.wrapping_add(1);
        
        // Put the descriptors back into the avail ring
        let avail_idx = unsafe { (*self.rx_avail).idx % 256 } as usize;
        unsafe {
            (*self.rx_avail).ring[avail_idx] = hdr_idx as u16;
            compiler_fence(Ordering::SeqCst);
            (*self.rx_avail).idx = (*self.rx_avail).idx.wrapping_add(1);
            compiler_fence(Ordering::SeqCst);
            Port::<u16>::new(self.io_base + REG_QUEUE_NOTIFY).write(0);
        }
        
        self.rx_pkts += 1;
        self.rx_bytes += data_len as u64;    
        
        Some(packet)
    }
    
    fn ack_interrupt(&mut self) -> bool {
        let status = unsafe { Port::<u8>::new(self.io_base + crate::drivers::virtio::virtq::REG_ISR_STATUS).read() };
        status != 0
    }
    
    fn rx_packets(&self) -> u64 { self.rx_pkts }
    fn tx_packets(&self) -> u64 { self.tx_pkts }
    fn rx_bytes(&self) -> u64 { self.rx_bytes }
    fn tx_bytes(&self) -> u64 { self.tx_bytes }
}
