use alloc::vec::Vec;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessId(pub usize);

pub struct PidAllocator {
    next_pid: usize,
    free_pids: Vec<usize>,
}

impl PidAllocator {
    pub const fn new() -> Self {
        PidAllocator {
            next_pid: 2, // PID 1 is reserved for init / kernel
            free_pids: Vec::new(),
        }
    }

    pub fn allocate(&mut self) -> usize {
        if let Some(pid) = self.free_pids.pop() {
            pid
        } else {
            let pid = self.next_pid;
            self.next_pid += 1;
            pid
        }
    }

    pub fn deallocate(&mut self, pid: usize) {
        if pid > 1 && !self.free_pids.contains(&pid) {
            self.free_pids.push(pid);
        }
    }
}
