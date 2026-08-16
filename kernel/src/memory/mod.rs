pub mod frame_allocator;
pub mod heap;
pub mod mmap;

use bootloader_api::info::MemoryRegions;
use frame_allocator::BootInfoFrameAllocator;
use heap::{HEAP_SIZE, HEAP_START};
use spin::Mutex;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
    Translate,
};
use x86_64::VirtAddr;

pub struct MemoryInfo {
    pub usable_ram_bytes: u64,
    pub allocated_frames: usize,
    pub shared_frames: usize,
    pub frame_size_bytes: usize,
    pub heap_start: usize,
    pub heap_size: usize,
    pub heap_initialized: bool,
    pub heap_test_passed: bool,
}

static MEM_INFO: Mutex<Option<MemoryInfo>> = Mutex::new(None);
static FRAME_ALLOCATOR: Mutex<Option<BootInfoFrameAllocator>> = Mutex::new(None);
static PHYS_OFFSET: Mutex<Option<VirtAddr>> = Mutex::new(None);

pub fn init(memory_regions: &'static MemoryRegions, physical_memory_offset: Option<u64>) -> bool {
    let phys_mem_offset = match physical_memory_offset {
        Some(offset) => VirtAddr::new(offset),
        None => return false,
    };

    *PHYS_OFFSET.lock() = Some(phys_mem_offset);

    let mut mapper = unsafe { init_page_table(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(memory_regions) };

    let usable_ram_bytes = frame_allocator.usable_memory_bytes();

    let heap_ok = heap::init_heap(&mut mapper, &mut frame_allocator).is_ok();
    let test_ok = if heap_ok { test_heap_allocation() } else { false };

    let allocated_frames = frame_allocator.allocated_frames_count();
    let shared_frames = frame_allocator.shared_frames_count();

    let mut mem_info = MEM_INFO.lock();
    *mem_info = Some(MemoryInfo {
        usable_ram_bytes,
        allocated_frames,
        shared_frames,
        frame_size_bytes: 4096,
        heap_start: HEAP_START,
        heap_size: HEAP_SIZE,
        heap_initialized: heap_ok,
        heap_test_passed: test_ok,
    });

    *FRAME_ALLOCATOR.lock() = Some(frame_allocator);

    heap_ok && test_ok
}

pub unsafe fn init_page_table(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    let level_4_table = unsafe { &mut *page_table_ptr };
    unsafe { OffsetPageTable::new(level_4_table, physical_memory_offset) }
}

#[allow(dead_code)]
pub fn allocate_frame() -> Option<PhysFrame> {
    let mut alloc = FRAME_ALLOCATOR.lock();
    if let Some(ref mut allocator) = *alloc {
        allocator.allocate_frame()
    } else {
        None
    }
}

pub fn inc_frame_ref(frame: PhysFrame) -> u16 {
    let mut alloc = FRAME_ALLOCATOR.lock();
    if let Some(ref mut allocator) = *alloc {
        allocator.inc_ref(frame)
    } else {
        1
    }
}

#[allow(dead_code)]
pub fn dec_frame_ref(frame: PhysFrame) -> u16 {
    let mut alloc = FRAME_ALLOCATOR.lock();
    if let Some(ref mut allocator) = *alloc {
        allocator.dec_ref(frame)
    } else {
        0
    }
}

#[allow(dead_code)]
pub fn get_frame_ref(frame: PhysFrame) -> u16 {
    let alloc = FRAME_ALLOCATOR.lock();
    if let Some(ref allocator) = *alloc {
        allocator.get_ref(frame)
    } else {
        1
    }
}

#[allow(dead_code)]
pub fn shared_frames_count() -> usize {
    let alloc = FRAME_ALLOCATOR.lock();
    if let Some(ref allocator) = *alloc {
        allocator.shared_frames_count()
    } else {
        0
    }
}

pub fn map_user_page(
    page: Page<Size4KiB>,
    flags: PageTableFlags,
) -> Result<(), &'static str> {
    let phys_offset = match *PHYS_OFFSET.lock() {
        Some(offset) => offset,
        None => return Err("Physical memory offset not initialized"),
    };

    let mut mapper = unsafe { init_page_table(phys_offset) };

    if mapper.translate_addr(page.start_address()).is_some() {
        unsafe {
            if let Ok(flush) = mapper.update_flags(page, flags) {
                flush.flush();
            }
        }
        return Ok(());
    }

    let mut alloc_lock = FRAME_ALLOCATOR.lock();
    let frame_allocator = match *alloc_lock {
        Some(ref mut alloc) => alloc,
        None => return Err("Frame allocator not initialized"),
    };

    let frame = frame_allocator
        .allocate_frame()
        .ok_or("Failed to allocate physical frame")?;

    unsafe {
        mapper
            .map_to(page, frame, flags, frame_allocator)
            .map_err(|_| "Failed to map user page")?
            .flush();
    }

    Ok(())
}

#[allow(dead_code)]
pub fn copy_page_physical(src_page: Page<Size4KiB>, dst_page: Page<Size4KiB>) -> bool {
    let phys_offset = match *PHYS_OFFSET.lock() {
        Some(offset) => offset,
        None => return false,
    };

    let mapper = unsafe { init_page_table(phys_offset) };
    let src_frame: PhysFrame<Size4KiB> = match mapper.translate_addr(src_page.start_address()) {
        Some(addr) => PhysFrame::containing_address(addr),
        None => return false,
    };
    let dst_frame: PhysFrame<Size4KiB> = match mapper.translate_addr(dst_page.start_address()) {
        Some(addr) => PhysFrame::containing_address(addr),
        None => return false,
    };

    let src_virt = phys_offset + src_frame.start_address().as_u64();
    let dst_virt = phys_offset + dst_frame.start_address().as_u64();
    unsafe {
        core::ptr::copy_nonoverlapping(
            src_virt.as_ptr::<u8>(),
            dst_virt.as_mut_ptr::<u8>(),
            4096,
        );
    }
    true
}

#[allow(dead_code)]
pub fn map_page_to_frame(
    page: Page<Size4KiB>,
    frame: PhysFrame,
    flags: PageTableFlags,
) -> Result<(), &'static str> {
    let phys_offset = match *PHYS_OFFSET.lock() {
        Some(offset) => offset,
        None => return Err("Physical memory offset not initialized"),
    };

    let mut mapper = unsafe { init_page_table(phys_offset) };

    if mapper.translate_addr(page.start_address()).is_some() {
        unsafe {
            if let Ok(flush) = mapper.update_flags(page, flags) {
                flush.flush();
            }
        }
        return Ok(());
    }

    let mut alloc_lock = FRAME_ALLOCATOR.lock();
    let frame_allocator = match *alloc_lock {
        Some(ref mut alloc) => alloc,
        None => return Err("Frame allocator not initialized"),
    };

    unsafe {
        mapper
            .map_to(page, frame, flags, frame_allocator)
            .map_err(|_| "Failed to map page to frame")?
            .flush();
    }

    Ok(())
}

pub fn update_page_flags(
    page: Page<Size4KiB>,
    flags: PageTableFlags,
) -> Result<(), &'static str> {
    let phys_offset = match *PHYS_OFFSET.lock() {
        Some(offset) => offset,
        None => return Err("Physical memory offset not initialized"),
    };

    let mut mapper = unsafe { init_page_table(phys_offset) };

    unsafe {
        let flush = mapper.update_flags(page, flags).map_err(|_| "Failed to update flags")?;
        flush.flush();
    }

    Ok(())
}

pub fn translate_page(page: Page<Size4KiB>) -> Option<PhysFrame> {
    let phys_offset = (*PHYS_OFFSET.lock())?;
    let mapper = unsafe { init_page_table(phys_offset) };
    let phys_addr = mapper.translate_addr(page.start_address())?;
    Some(PhysFrame::containing_address(phys_addr))
}

pub fn resolve_cow_page(page: Page<Size4KiB>) -> Result<(), &'static str> {
    let phys_offset = match *PHYS_OFFSET.lock() {
        Some(offset) => offset,
        None => return Err("Physical memory offset not initialized"),
    };

    let mut mapper = unsafe { init_page_table(phys_offset) };
    let phys_addr = mapper
        .translate_addr(page.start_address())
        .ok_or("Page not mapped")?;
    let old_frame = PhysFrame::containing_address(phys_addr);

    let mut alloc_lock = FRAME_ALLOCATOR.lock();
    let frame_allocator = match *alloc_lock {
        Some(ref mut alloc) => alloc,
        None => return Err("Frame allocator not initialized"),
    };

    let ref_count = frame_allocator.get_ref(old_frame);
    if ref_count <= 1 {
        // Sole owner: simply mark writable
        drop(alloc_lock);
        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::USER_ACCESSIBLE;
        unsafe {
            let flush = mapper.update_flags(page, flags).map_err(|_| "Failed to update flags")?;
            flush.flush();
        }
        Ok(())
    } else {
        // Shared: allocate new frame and copy data
        let new_frame = frame_allocator
            .allocate_frame()
            .ok_or("Out of memory for COW copy")?;

        // Copy 4096 bytes from old frame to new frame
        let src_virt = phys_offset + old_frame.start_address().as_u64();
        let dst_virt = phys_offset + new_frame.start_address().as_u64();
        unsafe {
            core::ptr::copy_nonoverlapping(
                src_virt.as_ptr::<u8>(),
                dst_virt.as_mut_ptr::<u8>(),
                4096,
            );
        }

        // Remap page to new_frame with WRITABLE
        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::USER_ACCESSIBLE;

        // Unmap old page from mapper without freeing physical frame
        let (_, flush) = mapper.unmap(page).map_err(|_| "Failed to unmap old COW page")?;
        flush.flush();

        // Decrement old frame refcount
        frame_allocator.dec_ref(old_frame);

        // Map new frame
        unsafe {
            mapper
                .map_to(page, new_frame, flags, frame_allocator)
                .map_err(|_| "Failed to map new COW frame")?
                .flush();
        }

        Ok(())
    }
}

#[allow(dead_code)]
pub fn deallocate_frame(frame: PhysFrame) {
    let mut alloc = FRAME_ALLOCATOR.lock();
    if let Some(ref mut allocator) = *alloc {
        allocator.deallocate_frame(frame);
    }
}

pub fn unmap_user_page(page: Page<Size4KiB>) -> Result<(), &'static str> {
    let phys_offset = match *PHYS_OFFSET.lock() {
        Some(offset) => offset,
        None => return Err("Physical memory offset not initialized"),
    };

    let mut mapper = unsafe { init_page_table(phys_offset) };

    let phys_addr = match mapper.translate_addr(page.start_address()) {
        Some(addr) => addr,
        None => return Ok(()),
    };
    let frame = PhysFrame::containing_address(phys_addr);

    let mut alloc_lock = FRAME_ALLOCATOR.lock();
    let frame_allocator = match *alloc_lock {
        Some(ref mut alloc) => alloc,
        None => return Err("Frame allocator not initialized"),
    };

    // Always unmap from current page table
    match mapper.unmap(page) {
        Ok((_unmapped_frame, flush)) => {
            flush.flush();
            frame_allocator.dec_ref(frame);
            Ok(())
        }
        Err(_) => Err("Failed to unmap page"),
    }
}

pub fn validate_user_buffer(ptr: *const u8, len: usize) -> bool {
    if ptr.is_null() || len == 0 {
        return false;
    }

    let start_addr = ptr as usize;
    let end_addr = match start_addr.checked_add(len) {
        Some(end) => end,
        None => return false,
    };

    // User space boundaries: above 2 MiB, below kernel heap (0x4444_4444_0000)
    const USER_MIN: usize = 0x0000_0000_0020_0000;
    const USER_MAX: usize = 0x0000_4000_0000_0000;
    if start_addr < USER_MIN || end_addr > USER_MAX {
        return false;
    }

    let phys_offset = match *PHYS_OFFSET.lock() {
        Some(offset) => offset,
        None => return false,
    };

    let mapper = unsafe { init_page_table(phys_offset) };

    let start_page = Page::<Size4KiB>::containing_address(VirtAddr::new(start_addr as u64));
    let end_page = Page::<Size4KiB>::containing_address(VirtAddr::new((end_addr - 1) as u64));

    for page in Page::range_inclusive(start_page, end_page) {
        if mapper.translate_addr(page.start_address()).is_none() {
            return false;
        }
    }

    true
}

fn test_heap_allocation() -> bool {
    use alloc::boxed::Box;
    use alloc::vec::Vec;

    let val1 = Box::new(42);
    let val2 = Box::new(1337);

    if *val1 != 42 || *val2 != 1337 {
        return false;
    }

    let mut vec = Vec::new();
    for i in 0..100 {
        vec.push(i * 3);
    }

    for (i, &val) in vec.iter().enumerate() {
        if val != (i * 3) as i32 {
            return false;
        }
    }

    true
}

pub fn get_memory_info() -> Option<MemoryInfo> {
    let info = MEM_INFO.lock();
    let alloc = FRAME_ALLOCATOR.lock();
    let (allocated, shared) = if let Some(ref a) = *alloc {
        (a.allocated_frames_count(), a.shared_frames_count())
    } else {
        (0, 0)
    };

    match *info {
        Some(ref inf) => Some(MemoryInfo {
            usable_ram_bytes: inf.usable_ram_bytes,
            allocated_frames: allocated,
            shared_frames: shared,
            frame_size_bytes: inf.frame_size_bytes,
            heap_start: inf.heap_start,
            heap_size: inf.heap_size,
            heap_initialized: inf.heap_initialized,
            heap_test_passed: inf.heap_test_passed,
        }),
        None => None,
    }
}
