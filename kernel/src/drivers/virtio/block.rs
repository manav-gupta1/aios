use core::sync::atomic::{compiler_fence, Ordering};
use x86_64::instructions::port::Port;
use crate::drivers::storage::BlockDevice;

const REG_DEVICE_FEATURES: u16 = 0x00;
const REG_GUEST_FEATURES: u16 = 0x04;
const REG_QUEUE_ADDRESS: u16 = 0x08;
const REG_QUEUE_SIZE: u16 = 0x0C;
const REG_QUEUE_SELECT: u16 = 0x0E;
const REG_QUEUE_NOTIFY: u16 = 0x10;
const REG_DEVICE_STATUS: u16 = 0x12;
const REG_ISR_STATUS: u16 = 0x13;
const REG_CONFIG: u16 = 0x14;

const STATUS_ACKNOWLEDGE: u8 = 1;
const STATUS_DRIVER: u8 = 2;
const STATUS_DRIVER_OK: u8 = 4;
const STATUS_FEATURES_OK: u8 = 8;
const STATUS_FAILED: u8 = 128;

const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

#[repr(C, align(16))]
struct VirtqDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
struct VirtqAvail {
    flags: u16,
    idx: u16,
    ring: [u16; 256],
}

#[repr(C)]
struct VirtqUsedElem {
    id: u32,
    len: u32,
}

#[repr(C, align(4096))]
struct VirtqUsed {
    flags: u16,
    idx: u16,
    ring: [VirtqUsedElem; 256],
}

#[repr(C)]
pub struct BlkReqHeader {
    type_: u32,
    ioprio: u32,
    sector: u64,
}

const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;

pub struct VirtioBlock {
    io_base: u16,
    irq: u8,
    queue_size: u16,
    capacity: u64,
    
    desc_table: *mut VirtqDesc,
    avail_ring: *mut VirtqAvail,
    used_ring: *mut VirtqUsed,
    
    last_used_idx: u16,
    avail_idx: u16,
    dma_header: *mut BlkReqHeader,
    dma_status: *mut u8,
}

static WAKER_PID: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
static VIRTIO_IO_BASE: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(0);

pub fn handle_irq() {
    let io_base = VIRTIO_IO_BASE.load(Ordering::SeqCst);
    if io_base == 0 { return; }
    
    let isr = unsafe { Port::<u8>::new(io_base + REG_ISR_STATUS).read() };
    if (isr & 1) != 0 {
        let pid = WAKER_PID.swap(0, Ordering::SeqCst);
        if pid != 0 {
            crate::process::PROCESS_TABLE.lock().wake_process_by_pid(pid);
        }
    }
}

// Ensure thread safety since we'll put it in a Mutex
unsafe impl Send for VirtioBlock {}
unsafe impl Sync for VirtioBlock {}

impl VirtioBlock {
    pub fn new(bus: u8, device: u8, function: u8, text: &mut crate::graphics::text::TextWriter) -> Result<Self, &'static str> {
        use crate::drivers::pci::config::*;
        
        let vendor = read_config_word(bus, device, function, 0x00);
        let device_id = read_config_word(bus, device, function, 0x02);
        
        if vendor != 0x1AF4 || (device_id != 0x1001 && device_id != 0x1042) {
            return Err("Not a VirtIO Block device");
        }
        
        let bar0 = read_config_dword(bus, device, function, 0x10);
        if (bar0 & 1) == 0 {
            return Err("BAR0 is not I/O space");
        }
        let io_base = (bar0 & !3) as u16;
        let irq = read_config_byte(bus, device, function, 0x3C);
        
        crate::drivers::storage::serial_print("[VIRTIO] BAR0 and IRQ found\n");
        
        unsafe { Port::<u8>::new(io_base + REG_DEVICE_STATUS).write(0) };
        crate::drivers::storage::serial_print("[VIRTIO] device reset\n");
        
        let mut status = STATUS_ACKNOWLEDGE | STATUS_DRIVER;
        unsafe { Port::<u8>::new(io_base + REG_DEVICE_STATUS).write(status) };
        
        let features = unsafe { Port::<u32>::new(io_base + REG_DEVICE_FEATURES).read() };
        unsafe { Port::<u32>::new(io_base + REG_GUEST_FEATURES).write(features & !(1 << 28)) }; // No INDIRECT_DESC
        
        unsafe { Port::<u16>::new(io_base + REG_QUEUE_SELECT).write(0) };
        let queue_size = unsafe { Port::<u16>::new(io_base + REG_QUEUE_SIZE).read() };
        
        if queue_size == 0 || queue_size > 256 {
            return Err("Unsupported queue size");
        }
        
        crate::drivers::storage::serial_print("[VIRTIO] queue checked\n");
        
        // Allocate contiguous physical frames for virtqueue. 
        // queue_size = 256. 
        // desc = 4096 bytes (offset 0). 
        // avail = 518 bytes (offset 4096).
        // used must be aligned to 4096, so offset 8192.
        // used = 2054 bytes. Total = 10246 bytes.
        // We need 3 contiguous frames (12288 bytes).
        let phys_frame = crate::memory::allocate_contiguous_frames(3).ok_or("Failed to alloc frames")?;
        let phys_addr = phys_frame.start_address().as_u64();
        let phys_offset = crate::memory::get_phys_offset().unwrap();
        let virt_addr = phys_addr + phys_offset;
        
        let desc_table = virt_addr as *mut VirtqDesc;
        let avail_ring = (virt_addr + 4096) as *mut VirtqAvail;
        let used_ring = (virt_addr + 8192) as *mut VirtqUsed;
        
        // We have free space between avail (ends at 4096+518=4614) and used (starts at 8192).
        let dma_header = (virt_addr + 5000) as *mut BlkReqHeader;
        let dma_status = (virt_addr + 5100) as *mut u8;
        
        // Zero out the memory
        unsafe {
            core::ptr::write_bytes(virt_addr as *mut u8, 0, 12288);
        }
        
        let pfn = (phys_addr / 4096) as u32;
        unsafe { Port::<u32>::new(io_base + REG_QUEUE_ADDRESS).write(pfn) };
        
        crate::drivers::storage::serial_print("[VIRTIO] queue initialized\n");
        
        // Read capacity from config space (sectors)
        // Virtio block config space starts at REG_CONFIG
        // capacity is at offset 0, 64-bit
        let cap_low = unsafe { Port::<u32>::new(io_base + REG_CONFIG).read() };
        let cap_high = unsafe { Port::<u32>::new(io_base + REG_CONFIG + 4).read() };
        let capacity = (cap_high as u64) << 32 | (cap_low as u64);
        
        VIRTIO_IO_BASE.store(io_base, Ordering::SeqCst);
        
        // Unmask the assigned IRQ using the new interrupt function
        crate::interrupts::unmask_irq(irq);
        
        // Tell device we are ready
        status |= STATUS_DRIVER_OK;
        unsafe { Port::<u8>::new(io_base + REG_DEVICE_STATUS).write(status) };
        
        crate::drivers::storage::serial_print("[VIRTIO] registered\n");
        Ok(Self {
            io_base,
            irq,
            queue_size,
            capacity,
            desc_table,
            avail_ring,
            used_ring,
            last_used_idx: 0,
            avail_idx: 0,
            dma_header,
            dma_status,
        })
    }


    fn wait_for_completion(&mut self) {
        let pid = crate::process::current_pid();
        WAKER_PID.store(pid, Ordering::SeqCst);
        let used = unsafe { &*self.used_ring };
        loop {
            compiler_fence(Ordering::SeqCst);
            let current = unsafe { core::ptr::read_volatile(&used.idx) };
            if current != self.last_used_idx {
                self.last_used_idx = self.last_used_idx.wrapping_add(1);
                WAKER_PID.store(0, Ordering::SeqCst);
                break;
            }
            crate::task::scheduler::block_current_task();
        }
    }
}

impl BlockDevice for spin::Mutex<VirtioBlock> {
    fn read_sector(&self, lba: u32, buf: &mut [u8]) -> Result<(), ()> {
        let mut blk = self.lock();
        if buf.len() != 512 { return Err(()); }
        
        unsafe {
            (*blk.dma_header).type_ = VIRTIO_BLK_T_IN;
            (*blk.dma_header).ioprio = 0;
            (*blk.dma_header).sector = lba as u64;
            *blk.dma_status = 255;
        }
        
        // Convert to physical addresses
        let phys_offset = crate::memory::get_phys_offset().unwrap();
        let header_phys = (blk.dma_header as u64) - phys_offset;
        let buf_phys = (buf.as_ptr() as u64) - phys_offset;
        let status_phys = (blk.dma_status as u64) - phys_offset;
        
        let desc = unsafe { core::slice::from_raw_parts_mut(blk.desc_table, 3) };
        desc[0] = VirtqDesc { addr: header_phys, len: 16, flags: VIRTQ_DESC_F_NEXT, next: 1 };
        desc[1] = VirtqDesc { addr: buf_phys, len: 512, flags: VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE, next: 2 };
        desc[2] = VirtqDesc { addr: status_phys, len: 1, flags: VIRTQ_DESC_F_WRITE, next: 0 };
        
        let head_idx = 0;
        let avail_idx = blk.avail_idx % blk.queue_size;
        unsafe {
            let avail = &mut *blk.avail_ring;
            avail.ring[avail_idx as usize] = head_idx;
            compiler_fence(Ordering::SeqCst);
            blk.avail_idx = blk.avail_idx.wrapping_add(1);
            core::ptr::write_volatile(&mut avail.idx, blk.avail_idx);
            compiler_fence(Ordering::SeqCst);
        }
        
        unsafe { Port::<u16>::new(blk.io_base + REG_QUEUE_NOTIFY).write(0) };
        
        blk.wait_for_completion();
        
        let status = unsafe { core::ptr::read_volatile(blk.dma_status) };
        if status == 0 { Ok(()) } else { Err(()) }
    }

    fn write_sector(&self, lba: u32, buf: &[u8]) -> Result<(), ()> {
        let mut blk = self.lock();
        if buf.len() != 512 { return Err(()); }
        
        unsafe {
            (*blk.dma_header).type_ = VIRTIO_BLK_T_OUT;
            (*blk.dma_header).ioprio = 0;
            (*blk.dma_header).sector = lba as u64;
            *blk.dma_status = 255;
        }
        
        // Convert to physical addresses
        let phys_offset = crate::memory::get_phys_offset().unwrap();
        let header_phys = (blk.dma_header as u64) - phys_offset;
        let buf_phys = (buf.as_ptr() as u64) - phys_offset;
        let status_phys = (blk.dma_status as u64) - phys_offset;
        
        let desc = unsafe { core::slice::from_raw_parts_mut(blk.desc_table, 3) };
        desc[0] = VirtqDesc { addr: header_phys, len: 16, flags: VIRTQ_DESC_F_NEXT, next: 1 };
        desc[1] = VirtqDesc { addr: buf_phys, len: 512, flags: VIRTQ_DESC_F_NEXT, next: 2 };
        desc[2] = VirtqDesc { addr: status_phys, len: 1, flags: VIRTQ_DESC_F_WRITE, next: 0 };
        
        let head_idx = 0;
        let avail_idx = blk.avail_idx % blk.queue_size;
        unsafe {
            let avail = &mut *blk.avail_ring;
            avail.ring[avail_idx as usize] = head_idx;
            compiler_fence(Ordering::SeqCst);
            blk.avail_idx = blk.avail_idx.wrapping_add(1);
            core::ptr::write_volatile(&mut avail.idx, blk.avail_idx);
            compiler_fence(Ordering::SeqCst);
        }
        
        unsafe { Port::<u16>::new(blk.io_base + REG_QUEUE_NOTIFY).write(0) };
        
        blk.wait_for_completion();
        
        let status = unsafe { core::ptr::read_volatile(blk.dma_status) };
        if status == 0 { Ok(()) } else { Err(()) }
    }

    fn sector_size(&self) -> usize {
        512
    }

    fn capacity_sectors(&self) -> u64 {
        self.lock().capacity
    }
    
    fn backend_name(&self) -> &'static str {
        "VirtIO Block (Legacy)"
    }
}
