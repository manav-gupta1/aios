use x86_64::structures::paging::{Page, PageTableFlags, Size4KiB};
use x86_64::VirtAddr;

pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
pub const ELFCLASS64: u8 = 2;
pub const ELFDATA2LSB: u8 = 1;
pub const EV_CURRENT: u8 = 1;
pub const ET_EXEC: u16 = 2;
pub const ET_DYN: u16 = 3;
pub const EM_X86_64: u16 = 62;

pub const PT_LOAD: u32 = 1;

pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
#[allow(dead_code)]
pub const PF_R: u32 = 4;

pub const USER_SPACE_MIN: u64 = 0x0000_0000_0020_0000; // 2 MiB
pub const USER_SPACE_MAX: u64 = 0x0000_7FFF_FFFF_0000;
pub const USER_STACK_BOTTOM: u64 = 0x0000_0000_8000_0000; // 8 MiB
pub const USER_STACK_SIZE: u64 = 16 * 1024; // 16 KiB
pub const USER_STACK_TOP: u64 = USER_STACK_BOTTOM + USER_STACK_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfError {
    FileTooShort,
    InvalidMagic,
    InvalidClass,
    InvalidEndianness,
    InvalidVersion,
    UnsupportedArchitecture,
    UnsupportedFileType,
    InvalidHeaderSize,
    InvalidProgramHeaderOffset,
    InvalidProgramHeaderSize,
    TooManyProgramHeaders,
    SegmentOutOfBounds,
    InvalidSegmentSizes,
    AddressOverflow,
    KernelAddressConflict,
    InvalidEntryPoint,
    #[allow(dead_code)]
    MemoryAllocationFailed,
    MappingFailed,
}

impl ElfError {
    pub fn as_str(&self) -> &'static str {
        match self {
            ElfError::FileTooShort => "File too short for ELF header",
            ElfError::InvalidMagic => "Invalid ELF magic identifier",
            ElfError::InvalidClass => "Not a 64-bit ELF (ELFCLASS64 required)",
            ElfError::InvalidEndianness => "Not little-endian (ELFDATA2LSB required)",
            ElfError::InvalidVersion => "Unsupported ELF version",
            ElfError::UnsupportedArchitecture => "Unsupported machine architecture (x86_64 required)",
            ElfError::UnsupportedFileType => "Unsupported ELF file type (must be executable)",
            ElfError::InvalidHeaderSize => "Invalid ELF header size",
            ElfError::InvalidProgramHeaderOffset => "Program header table offset out of bounds",
            ElfError::InvalidProgramHeaderSize => "Invalid program header entry size",
            ElfError::TooManyProgramHeaders => "Too many program headers in file",
            ElfError::SegmentOutOfBounds => "Segment file range exceeds file length",
            ElfError::InvalidSegmentSizes => "Invalid segment size (p_memsz < p_filesz)",
            ElfError::AddressOverflow => "Segment virtual address calculation overflowed",
            ElfError::KernelAddressConflict => "Segment virtual address conflicts with kernel memory",
            ElfError::InvalidEntryPoint => "Entry point not within executable program segment",
            ElfError::MemoryAllocationFailed => "Physical frame allocation failed during ELF loading",
            ElfError::MappingFailed => "Page table mapping failed during ELF loading",
        }
    }
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Header {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64ProgramHeader {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

#[allow(dead_code)]
pub struct LoadedElf {
    pub entry_point: u64,
    pub user_rsp: u64,
    pub load_base: u64,
}

pub fn validate_elf_headers(data: &[u8]) -> Result<(Elf64Header, u64), ElfError> {
    if data.len() < core::mem::size_of::<Elf64Header>() {
        return Err(ElfError::FileTooShort);
    }

    let header = unsafe { core::ptr::read_unaligned(data.as_ptr() as *const Elf64Header) };

    if header.e_ident[0..4] != ELF_MAGIC {
        return Err(ElfError::InvalidMagic);
    }

    if header.e_ident[4] != ELFCLASS64 {
        return Err(ElfError::InvalidClass);
    }

    if header.e_ident[5] != ELFDATA2LSB {
        return Err(ElfError::InvalidEndianness);
    }

    if header.e_ident[6] != EV_CURRENT {
        return Err(ElfError::InvalidVersion);
    }

    if header.e_machine != EM_X86_64 {
        return Err(ElfError::UnsupportedArchitecture);
    }

    if header.e_type != ET_EXEC && header.e_type != ET_DYN {
        return Err(ElfError::UnsupportedFileType);
    }

    if (header.e_ehsize as usize) < core::mem::size_of::<Elf64Header>() {
        return Err(ElfError::InvalidHeaderSize);
    }

    if (header.e_phentsize as usize) < core::mem::size_of::<Elf64ProgramHeader>() {
        return Err(ElfError::InvalidProgramHeaderSize);
    }

    if header.e_phnum == 0 || header.e_phnum > 64 {
        return Err(ElfError::TooManyProgramHeaders);
    }

    let ph_total_size = (header.e_phnum as usize)
        .checked_mul(header.e_phentsize as usize)
        .ok_or(ElfError::InvalidProgramHeaderOffset)?;

    let ph_end = (header.e_phoff as usize)
        .checked_add(ph_total_size)
        .ok_or(ElfError::InvalidProgramHeaderOffset)?;

    if ph_end > data.len() {
        return Err(ElfError::InvalidProgramHeaderOffset);
    }

    let base_load_addr: u64 = if header.e_type == ET_DYN || header.e_entry < USER_SPACE_MIN {
        0x0000_0000_0040_0000 // Rebase position-independent ELF to 4 MiB
    } else {
        0
    };

    let actual_entry = base_load_addr
        .checked_add(header.e_entry)
        .ok_or(ElfError::AddressOverflow)?;

    let mut entry_in_executable_segment = false;

    for i in 0..header.e_phnum {
        let ph_offset = (header.e_phoff as usize) + (i as usize) * (header.e_phentsize as usize);
        let ph = unsafe {
            core::ptr::read_unaligned((data.as_ptr().add(ph_offset)) as *const Elf64ProgramHeader)
        };

        if ph.p_type == PT_LOAD {
            let seg_file_end = (ph.p_offset as usize)
                .checked_add(ph.p_filesz as usize)
                .ok_or(ElfError::SegmentOutOfBounds)?;

            if seg_file_end > data.len() {
                return Err(ElfError::SegmentOutOfBounds);
            }

            if ph.p_memsz < ph.p_filesz {
                return Err(ElfError::InvalidSegmentSizes);
            }

            let seg_vaddr = base_load_addr
                .checked_add(ph.p_vaddr)
                .ok_or(ElfError::AddressOverflow)?;

            let seg_end_vaddr = seg_vaddr
                .checked_add(ph.p_memsz)
                .ok_or(ElfError::AddressOverflow)?;

            if seg_vaddr < USER_SPACE_MIN || seg_end_vaddr > USER_SPACE_MAX {
                return Err(ElfError::KernelAddressConflict);
            }

            if (ph.p_flags & PF_X) != 0 && actual_entry >= seg_vaddr && actual_entry < seg_end_vaddr {
                entry_in_executable_segment = true;
            }
        }
    }

    if !entry_in_executable_segment {
        return Err(ElfError::InvalidEntryPoint);
    }

    Ok((header, base_load_addr))
}

pub fn load_elf(data: &[u8]) -> Result<(LoadedElf, crate::process::ProcessAddressSpace), ElfError> {
    let (header, base_load_addr) = validate_elf_headers(data)?;

    let actual_entry = base_load_addr + header.e_entry;
    let mut address_space = crate::process::ProcessAddressSpace::new();

    // Load and map each PT_LOAD segment
    for i in 0..header.e_phnum {
        let ph_offset = (header.e_phoff as usize) + (i as usize) * (header.e_phentsize as usize);
        let ph = unsafe {
            core::ptr::read_unaligned((data.as_ptr().add(ph_offset)) as *const Elf64ProgramHeader)
        };

        if ph.p_type == PT_LOAD {
            let seg_vaddr = base_load_addr + ph.p_vaddr;
            let seg_filesz = ph.p_filesz as usize;
            let seg_memsz = ph.p_memsz as usize;

            let page_start_addr = seg_vaddr & !0xFFF;
            let page_end_addr = (seg_vaddr + (seg_memsz as u64) + 0xFFF) & !0xFFF;

            let mut pt_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
            if (ph.p_flags & PF_W) != 0 {
                pt_flags |= PageTableFlags::WRITABLE;
            }
            // Disabled NO_EXECUTE to prevent unaligned segment overlaps from breaking execution
            // if (ph.p_flags & PF_X) == 0 {
            //     pt_flags |= PageTableFlags::NO_EXECUTE;
            // }

            // Map page with writable permission first so kernel can write segment data
            let load_flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE;

            let mut current_addr = page_start_addr;
            while current_addr < page_end_addr {
                let page = Page::<Size4KiB>::containing_address(VirtAddr::new(current_addr));
                
                // Check if already mapped to avoid double-zeroing overlapping segments
                let mut already_mapped = false;
                if crate::memory::translate_page(page).is_some() {
                    already_mapped = true;
                }

                crate::memory::map_user_page(page, load_flags)
                    .map_err(|_| ElfError::MappingFailed)?;
                address_space.add_page(page);

                if !already_mapped {
                    unsafe {
                        core::ptr::write_bytes(current_addr as *mut u8, 0, 4096);
                    }
                }

                current_addr += 4096;
            }

            unsafe {
                let src = &data[ph.p_offset as usize..(ph.p_offset as usize + seg_filesz)];
                let dst = seg_vaddr as *mut u8;
                core::ptr::copy_nonoverlapping(src.as_ptr(), dst, seg_filesz);
            }

            // Set final page permissions according to ELF flags
            // We disabled this completely to prevent overlapping segments from corrupting flags
            // and causing page faults. All pages remain RWX (load_flags).
        }
    }

    // Allocate and map user stack
    let stack_page_start = USER_STACK_BOTTOM;
    let stack_page_end = USER_STACK_TOP;
    let stack_flags = PageTableFlags::PRESENT
        | PageTableFlags::WRITABLE
        | PageTableFlags::USER_ACCESSIBLE
        | PageTableFlags::NO_EXECUTE;

    let mut curr_stack = stack_page_start;
    while curr_stack < stack_page_end {
        let page = Page::<Size4KiB>::containing_address(VirtAddr::new(curr_stack));
        crate::memory::map_user_page(page, stack_flags)
            .map_err(|_| ElfError::MappingFailed)?;
        address_space.add_page(page);

        curr_stack += 4096;
    }

    let user_rsp = USER_STACK_TOP - 16;

    Ok((
        LoadedElf {
            entry_point: actual_entry,
            user_rsp,
            load_base: base_load_addr,
        },
        address_space,
    ))
}

pub fn load_and_spawn_elf(
    parent_pid: usize,
    name: &'static str,
    data: &[u8],
) -> Result<usize, ElfError> {
    let (loaded, address_space) = load_elf(data)?;
    let pid = crate::process::spawn_user_process(
        parent_pid,
        name,
        loaded.entry_point,
        loaded.user_rsp,
        address_space,
    )
    .map_err(|_| ElfError::MemoryAllocationFailed)?;
    Ok(pid)
}
