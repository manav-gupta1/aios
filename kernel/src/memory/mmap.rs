use x86_64::structures::paging::{Page, Size4KiB, PageTableFlags};
use x86_64::VirtAddr;
use crate::process::{MmapRegion, PROCESS_TABLE, current_pid};
use alloc::vec::Vec;

#[allow(dead_code)]
pub const PROT_NONE: usize = 0;
pub const PROT_READ: usize = 1;
pub const PROT_WRITE: usize = 2;
pub const PROT_EXEC: usize = 4;

#[allow(dead_code)]
pub const MAP_SHARED: usize = 0x01;
pub const MAP_PRIVATE: usize = 0x02;
pub const MAP_ANONYMOUS: usize = 0x20;

const MAX_MMAP_SIZE: usize = 1024 * 1024 * 1024; // 1 GB max size per mapping

pub fn do_mmap(
    addr: usize,
    length: usize,
    prot: usize,
    flags: usize,
    fd: usize,
    offset: usize,
) -> Result<usize, &'static str> {
    if length == 0 || length > MAX_MMAP_SIZE {
        return Err("Invalid mmap length");
    }

    let is_anonymous = (flags & MAP_ANONYMOUS) != 0;

    if (flags & MAP_PRIVATE) == 0 && (flags & MAP_SHARED) == 0 {
        return Err("Must specify MAP_PRIVATE or MAP_SHARED");
    }
    if (flags & MAP_PRIVATE) != 0 && (flags & MAP_SHARED) != 0 {
        return Err("Cannot specify both MAP_PRIVATE and MAP_SHARED");
    }

    // No RWX allowed
    let _is_read = (prot & PROT_READ) != 0;
    let is_write = (prot & PROT_WRITE) != 0;
    let is_exec = (prot & PROT_EXEC) != 0;
    
    if is_write && is_exec {
        return Err("RWX mappings not allowed");
    }

    // Map protections to page table flags
    let mut page_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
    if is_write {
        page_flags |= PageTableFlags::WRITABLE;
    }
    if !is_exec {
        page_flags |= PageTableFlags::NO_EXECUTE;
    }

    if addr != 0 {
        return Err("Fixed mmap not supported");
    }

    let pid = current_pid();
    let mut table = PROCESS_TABLE.lock();
    let process = table.processes.iter_mut().find(|p| p.pid == pid).ok_or("No process")?;

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut tracked_file_path: Option<alloc::string::String> = None;
    if !is_anonymous {
        let fd_desc = process.get_fd(fd).ok_or("Bad fd for mmap")?;
        if let crate::process::FileDescriptor::File(ref path, _) = fd_desc {
            let fs = crate::fs::FILESYSTEM.lock();
            let bytes = fs.read_file_bytes(path).map_err(|_| "Failed to read file for mmap")?;
            if offset >= bytes.len() {
                file_bytes = Some(Vec::new());
            } else {
                let mut data = Vec::with_capacity(bytes.len() - offset);
                data.extend_from_slice(&bytes[offset..]);
                file_bytes = Some(data);
            }
            tracked_file_path = Some(path.clone());
        } else {
            return Err("fd is not a file");
        }
    }

    let length_aligned = (length + 4095) & !4095;
    
    let mut search_addr = 0x0000_1000_0000_0000;
    loop {
        let overlaps = process.address_space.mmap_regions.iter().any(|r| {
            let r_start = r.start;
            let r_end = r.start + r.length;
            let s_start = search_addr;
            let s_end = search_addr + length_aligned;
            s_start < r_end && s_end > r_start
        });

        if !overlaps {
            break;
        }
        search_addr += length_aligned;
        if search_addr >= 0x0000_4000_0000_0000 {
            return Err("Out of virtual address space");
        }
    }

    let start_page = Page::<Size4KiB>::containing_address(VirtAddr::new(search_addr as u64));
    let num_pages = length_aligned / 4096;

    for i in 0..num_pages {
        let page = start_page + i as u64;
        
        if let Err(_) = crate::memory::map_user_page(page, page_flags) {
            for j in 0..i {
                let p = start_page + j as u64;
                process.address_space.remove_page(p);
                let _ = crate::memory::unmap_user_page(p);
            }
            return Err("Failed to map user page during mmap");
        }
        
        process.address_space.add_page(page);
    }
    
    unsafe {
        core::ptr::write_bytes(search_addr as *mut u8, 0, length_aligned);
        if let Some(bytes) = file_bytes {
            let to_copy = core::cmp::min(bytes.len(), length);
            if to_copy > 0 {
                core::ptr::copy_nonoverlapping(
                    bytes.as_ptr(),
                    search_addr as *mut u8,
                    to_copy,
                );
            }
        }
    }

    process.address_space.mmap_regions.push(MmapRegion {
        start: search_addr,
        length: length_aligned,
        prot,
        flags,
        file_path: tracked_file_path,
    });

    Ok(search_addr)
}

pub fn do_munmap(addr: usize, length: usize) -> Result<(), &'static str> {
    if addr % 4096 != 0 {
        return Err("munmap address not page-aligned");
    }
    if length == 0 {
        return Err("Invalid munmap length");
    }

    let length_aligned = (length + 4095) & !4095;

    let pid = current_pid();
    let mut table = PROCESS_TABLE.lock();
    let process = table.processes.iter_mut().find(|p| p.pid == pid).ok_or("No process")?;
    
    let pos = process.address_space.mmap_regions.iter().position(|r| r.start == addr);
    if let Some(idx) = pos {
        let region = process.address_space.mmap_regions[idx].clone();
        
        if region.length != length_aligned {
            return Err("Partial munmap not fully supported yet");
        }

        if (region.flags & MAP_SHARED) != 0 {
            if let Some(ref path) = region.file_path {
                let slice = unsafe { core::slice::from_raw_parts(region.start as *const u8, region.length) };
                let mut fs = crate::fs::FILESYSTEM.lock();
                let _ = fs.write_file(path, slice);
            }
        }

        let start_page = Page::<Size4KiB>::containing_address(VirtAddr::new(addr as u64));
        let num_pages = length_aligned / 4096;

        for i in 0..num_pages {
            let page = start_page + i as u64;
            let _ = crate::memory::unmap_user_page(page);
            process.address_space.remove_page(page);
        }

        process.address_space.mmap_regions.remove(idx);
        Ok(())
    } else {
        Err("Region not found")
    }
}
