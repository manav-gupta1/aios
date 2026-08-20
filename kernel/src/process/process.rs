use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;
use x86_64::structures::paging::{Page, Size4KiB};
use crate::ipc::Pipe;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Ready,
    Running,
    Blocked,
    Zombie,
    Stopped,
}

impl ProcessState {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            ProcessState::Ready => "READY",
            ProcessState::Running => "RUNNING",
            ProcessState::Blocked => "BLOCKED",
            ProcessState::Zombie => "ZOMBIE",
            ProcessState::Stopped => "STOPPED",
        }
    }
}

#[derive(Clone)]
pub enum FileDescriptor {
    Stdin,
    Stdout,
    Stderr,
    PipeRead(Arc<Mutex<Pipe>>),
    PipeWrite(Arc<Mutex<Pipe>>),
    File(String, usize), // path, current offset
    Socket(usize),       // socket id in global SOCKET_TABLE
}

#[derive(Clone)]
pub struct MmapRegion {
    pub start: usize,
    pub length: usize,
    pub prot: usize,
    pub flags: usize,
    pub file_path: Option<String>,
}

pub struct ProcessAddressSpace {
    pub pages: Vec<Page<Size4KiB>>,
    pub cow_pages: Vec<Page<Size4KiB>>,
    pub mmap_regions: Vec<MmapRegion>,
}

impl ProcessAddressSpace {
    pub const fn new() -> Self {
        ProcessAddressSpace {
            pages: Vec::new(),
            cow_pages: Vec::new(),
            mmap_regions: Vec::new(),
        }
    }

    pub fn add_page(&mut self, page: Page<Size4KiB>) {
        if !self.pages.contains(&page) {
            self.pages.push(page);
        }
    }

    pub fn remove_page(&mut self, page: Page<Size4KiB>) {
        if let Some(pos) = self.pages.iter().position(|p| *p == page) {
            self.pages.remove(pos);
        }
    }

    pub fn is_cow(&self, page: Page<Size4KiB>) -> bool {
        self.cow_pages.contains(&page)
    }

    pub fn mark_cow(&mut self, page: Page<Size4KiB>) {
        if !self.cow_pages.contains(&page) {
            self.cow_pages.push(page);
        }
    }

    pub fn unmark_cow(&mut self, page: Page<Size4KiB>) {
        if let Some(pos) = self.cow_pages.iter().position(|p| *p == page) {
            self.cow_pages.remove(pos);
        }
    }

    pub fn unmap_all(&mut self) {
        self.cow_pages.clear();
        for region in &self.mmap_regions {
            if (region.flags & 0x01) != 0 { // MAP_SHARED
                if let Some(ref path) = region.file_path {
                    let slice = unsafe { core::slice::from_raw_parts(region.start as *const u8, region.length) };
                    let mut fs = crate::fs::FILESYSTEM.lock();
                    let _ = fs.write_file(path, slice);
                }
            }
        }
        self.mmap_regions.clear();
        for page in self.pages.drain(..) {
            let _ = crate::memory::unmap_user_page(page);
        }
    }
}

pub struct Process {
    pub pid: usize,
    pub ppid: usize,
    pub pgid: usize,
    pub sid: usize,
    pub state: ProcessState,
    pub name: String,
    pub main_task_id: usize,
    pub exit_status: Option<i32>,
    pub address_space: ProcessAddressSpace,
    pub waiting_target_pid: Option<Option<usize>>,
    pub fd_table: Vec<Option<FileDescriptor>>,
    pub pending_signals: u64,
    pub blocked_signals: u64,
    pub sig_actions: [usize; 64],
    pub is_stopped: bool,
    pub is_orphan: bool,
}

impl Process {
    fn default_fd_table() -> Vec<Option<FileDescriptor>> {
        let mut fds = Vec::with_capacity(8);
        fds.push(Some(FileDescriptor::Stdin));
        fds.push(Some(FileDescriptor::Stdout));
        fds.push(Some(FileDescriptor::Stderr));
        fds
    }

    pub fn new_init() -> Self {
        Process {
            pid: 1,
            ppid: 0,
            pgid: 1,
            sid: 1,
            state: ProcessState::Running,
            name: String::from("init"),
            main_task_id: 1,
            exit_status: None,
            address_space: ProcessAddressSpace::new(),
            waiting_target_pid: None,
            fd_table: Self::default_fd_table(),
            pending_signals: 0,
            blocked_signals: 0,
            sig_actions: [0; 64],
            is_stopped: false,
            is_orphan: false,
        }
    }

    pub fn new_user(
        pid: usize,
        ppid: usize,
        name: &str,
        main_task_id: usize,
        address_space: ProcessAddressSpace,
    ) -> Self {
        Process {
            pid,
            ppid,
            pgid: ppid, // By default, inherit parent's pgid
            sid: ppid,  // and sid (simplification)
            state: ProcessState::Ready,
            name: String::from(name),
            main_task_id,
            exit_status: None,
            address_space,
            waiting_target_pid: None,
            fd_table: Self::default_fd_table(),
            pending_signals: 0,
            blocked_signals: 0,
            sig_actions: [0; 64],
            is_stopped: false,
            is_orphan: false,
        }
    }

    pub fn alloc_fd(&mut self, desc: FileDescriptor) -> usize {
        // Look for existing free slot starting at index 3
        for idx in 3..self.fd_table.len() {
            if self.fd_table[idx].is_none() {
                self.fd_table[idx] = Some(desc);
                return idx;
            }
        }
        // If no slot available, push
        let idx = self.fd_table.len();
        self.fd_table.push(Some(desc));
        idx
    }

    pub fn get_fd(&self, fd: usize) -> Option<FileDescriptor> {
        if fd < self.fd_table.len() {
            self.fd_table[fd].clone()
        } else {
            None
        }
    }

    pub fn close_fd(&mut self, fd: usize) -> Option<FileDescriptor> {
        if fd < self.fd_table.len() {
            self.fd_table[fd].take()
        } else {
            None
        }
    }
}
