pub mod fork;
pub mod pid;
pub mod process;
pub mod table;

use alloc::vec::Vec;
#[allow(unused_imports)]
pub use pid::{PidAllocator, ProcessId};
#[allow(unused_imports)]
pub use process::{FileDescriptor, Process, ProcessAddressSpace, ProcessState};
#[allow(unused_imports)]
pub use table::{
    PipeReadResult, PipeWriteResult, ProcessSnapshot, ProcessTable, WaitError, PROCESS_TABLE,
};

pub fn init() {
    PROCESS_TABLE.lock().init();
}

pub fn spawn_user_process(
    parent_pid: usize,
    name: &'static str,
    rip: u64,
    rsp: u64,
    address_space: ProcessAddressSpace,
) -> Result<usize, &'static str> {
    PROCESS_TABLE.lock().spawn_user_process(parent_pid, name, rip, rsp, address_space)
}

pub fn exit_current_process(status: i32) -> ! {
    let pid = current_pid();
    PROCESS_TABLE.lock().exit_process(pid, status);
    crate::task::scheduler::exit_current_task()
}

pub fn current_pid() -> usize {
    crate::task::current_process_id()
}

pub fn waitpid(caller_pid: usize, target: Option<usize>) -> Result<(usize, i32), WaitError> {
    PROCESS_TABLE.lock().waitpid(caller_pid, target)
}

pub fn exec_current(caller_pid: usize, path: &str) -> Result<(u64, u64), &'static str> {
    // Read ELF bytes from filesystem
    let fs = crate::fs::FILESYSTEM.lock();
    let file_bytes = match fs.read_file_bytes(path) {
        Ok(bytes) => bytes,
        Err(_) => {
            drop(fs);
            return Err("File not found");
        }
    };

    let mut elf_buf = [0u8; crate::fs::MAX_FILE_SIZE];
    let len = file_bytes.len();
    elf_buf[..len].copy_from_slice(file_bytes);
    drop(fs);

    PROCESS_TABLE.lock().exec_current_process(caller_pid, path, &elf_buf[..len])
}

pub fn create_pipe(caller_pid: usize) -> Result<(usize, usize), &'static str> {
    PROCESS_TABLE.lock().create_pipe(caller_pid)
}

pub fn read_fd(
    caller_pid: usize,
    fd: usize,
    buf: &mut [u8],
) -> Result<usize, PipeReadResult> {
    PROCESS_TABLE.lock().read_fd(caller_pid, fd, buf)
}

pub fn write_fd(
    caller_pid: usize,
    fd: usize,
    buf: &[u8],
) -> Result<usize, PipeWriteResult> {
    PROCESS_TABLE.lock().write_fd(caller_pid, fd, buf)
}

pub fn close_fd(caller_pid: usize, fd: usize) -> Result<(), &'static str> {
    PROCESS_TABLE.lock().close_fd(caller_pid, fd)
}

pub fn fork_current_process(caller_pid: usize, frame_ptr: *mut u64) -> Result<usize, &'static str> {
    PROCESS_TABLE.lock().fork_process(caller_pid, frame_ptr)
}

pub fn handle_cow_fault(caller_pid: usize, fault_addr: x86_64::VirtAddr) -> Result<(), &'static str> {
    PROCESS_TABLE.lock().handle_cow_fault(caller_pid, fault_addr)
}

pub fn get_process_snapshots() -> Vec<ProcessSnapshot> {
    PROCESS_TABLE.lock().snapshots()
}
