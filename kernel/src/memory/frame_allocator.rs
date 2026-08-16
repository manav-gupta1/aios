use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};
use x86_64::PhysAddr;

pub struct BootInfoFrameAllocator {
    memory_regions: &'static MemoryRegions,
    next: usize,
    allocated_frames: usize,
    freelist: Vec<PhysFrame>,
    ref_counts: BTreeMap<PhysFrame, u16>,
}

impl BootInfoFrameAllocator {
    pub unsafe fn init(memory_regions: &'static MemoryRegions) -> Self {
        BootInfoFrameAllocator {
            memory_regions,
            next: 0,
            allocated_frames: 0,
            freelist: Vec::new(),
            ref_counts: BTreeMap::new(),
        }
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        let regions = self.memory_regions.iter();
        let usable_regions = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
        let addr_ranges = usable_regions.map(|r| r.start..r.end);
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }

    pub fn usable_memory_bytes(&self) -> u64 {
        self.memory_regions
            .iter()
            .filter(|r| r.kind == MemoryRegionKind::Usable)
            .map(|r| r.end - r.start)
            .sum()
    }

    pub fn allocated_frames_count(&self) -> usize {
        self.allocated_frames
    }

    pub fn deallocate_frame(&mut self, frame: PhysFrame) {
        self.dec_ref(frame);
    }

    pub fn inc_ref(&mut self, frame: PhysFrame) -> u16 {
        let count = self.ref_counts.entry(frame).or_insert(1);
        *count = count.saturating_add(1);
        *count
    }

    pub fn dec_ref(&mut self, frame: PhysFrame) -> u16 {
        if let Some(count) = self.ref_counts.get_mut(&frame) {
            if *count > 2 {
                *count -= 1;
                *count
            } else if *count == 2 {
                self.ref_counts.remove(&frame);
                1
            } else {
                self.ref_counts.remove(&frame);
                self.freelist.push(frame);
                if self.allocated_frames > 0 {
                    self.allocated_frames -= 1;
                }
                0
            }
        } else {
            // Sole owner (implicit refcount was 1)
            self.freelist.push(frame);
            if self.allocated_frames > 0 {
                self.allocated_frames -= 1;
            }
            0
        }
    }

    pub fn get_ref(&self, frame: PhysFrame) -> u16 {
        self.ref_counts.get(&frame).copied().unwrap_or(1)
    }

    pub fn shared_frames_count(&self) -> usize {
        self.ref_counts.values().filter(|&&v| v > 1).count()
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = if let Some(frame) = self.freelist.pop() {
            Some(frame)
        } else {
            let frame = self.usable_frames().nth(self.next);
            if frame.is_some() {
                self.next += 1;
            }
            frame
        };

        if frame.is_some() {
            self.allocated_frames += 1;
        }
        frame
    }
}
