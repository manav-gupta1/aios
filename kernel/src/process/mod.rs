pub mod fork;
pub mod pid;
pub mod process;
pub mod table;
pub mod signal;

use alloc::vec::Vec;
#[allow(unused_imports)]
pub use pid::{PidAllocator, ProcessId};
#[allow(unused_imports)]
pub use process::{FileDescriptor, MmapRegion, Process, ProcessAddressSpace, ProcessState};
#[allow(unused_imports)]
pub use table::{
    PipeReadResult, PipeWriteResult, ProcessSnapshot, ProcessTable, WaitError, PROCESS_TABLE,
};
#[allow(unused_imports)]
pub use signal::{SIGINT, SIGKILL, SIGTERM, SIGSTOP, SIGCONT, SIGCHLD, SIGTSTP, SIG_DFL, SIG_IGN};

pub fn init() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        PROCESS_TABLE.lock().init();
    });
}

pub fn spawn_user_process(
    parent_pid: usize,
    name: &'static str,
    rip: u64,
    rsp: u64,
    address_space: ProcessAddressSpace,
) -> Result<usize, &'static str> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        PROCESS_TABLE.lock().spawn_user_process(parent_pid, name, rip, rsp, address_space)
    })
}

pub fn current_pid() -> usize {
    crate::task::current_process_id()
}

pub fn waitpid(caller_pid: usize, target: Option<usize>) -> Result<(usize, i32), WaitError> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        PROCESS_TABLE.lock().waitpid(caller_pid, target)
    })
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

    x86_64::instructions::interrupts::without_interrupts(|| {
        PROCESS_TABLE.lock().exec_current_process(caller_pid, path, &elf_buf[..len])
    })
}

pub fn create_pipe(caller_pid: usize) -> Result<(usize, usize), &'static str> {
    x86_64::instructions::interrupts::without_interrupts(|| PROCESS_TABLE.lock().create_pipe(caller_pid))
}

pub fn open_file(caller_pid: usize, path: &str) -> Result<usize, &'static str> {
    x86_64::instructions::interrupts::without_interrupts(|| PROCESS_TABLE.lock().open_file(caller_pid, path))
}

pub fn read_fd(
    caller_pid: usize,
    fd: usize,
    buf: &mut [u8],
) -> Result<usize, PipeReadResult> {
    x86_64::instructions::interrupts::without_interrupts(|| PROCESS_TABLE.lock().read_fd(caller_pid, fd, buf))
}

pub fn write_fd(
    caller_pid: usize,
    fd: usize,
    buf: &[u8],
) -> Result<usize, PipeWriteResult> {
    x86_64::instructions::interrupts::without_interrupts(|| PROCESS_TABLE.lock().write_fd(caller_pid, fd, buf))
}

pub fn close_fd(caller_pid: usize, fd: usize) -> Result<(), &'static str> {
    x86_64::instructions::interrupts::without_interrupts(|| PROCESS_TABLE.lock().close_fd(caller_pid, fd))
}

pub fn fork_current_process(caller_pid: usize, frame_ptr: *mut u64) -> Result<usize, &'static str> {
    x86_64::instructions::interrupts::without_interrupts(|| PROCESS_TABLE.lock().fork_process(caller_pid, frame_ptr))
}

pub fn handle_cow_fault(caller_pid: usize, fault_addr: x86_64::VirtAddr) -> Result<(), &'static str> {
    x86_64::instructions::interrupts::without_interrupts(|| PROCESS_TABLE.lock().handle_cow_fault(caller_pid, fault_addr))
}

pub fn get_process_snapshots() -> Vec<ProcessSnapshot> {
    x86_64::instructions::interrupts::without_interrupts(|| PROCESS_TABLE.lock().snapshots())
}

pub fn sys_kill(caller_pid: usize, target_pid: usize, signum: usize) -> Result<(), &'static str> {
    x86_64::instructions::interrupts::without_interrupts(|| PROCESS_TABLE.lock().sys_kill(caller_pid, target_pid, signum))
}

pub fn sys_sigaction(caller_pid: usize, signum: usize, act_ptr: usize, oldact_ptr: usize) -> Result<(), &'static str> {
    x86_64::instructions::interrupts::without_interrupts(|| PROCESS_TABLE.lock().sys_sigaction(caller_pid, signum, act_ptr, oldact_ptr))
}

pub fn sys_setpgid(caller_pid: usize, target_pid: usize, pgid: usize) -> Result<(), &'static str> {
    x86_64::instructions::interrupts::without_interrupts(|| PROCESS_TABLE.lock().sys_setpgid(caller_pid, target_pid, pgid))
}

pub fn sys_killpg(caller_pid: usize, target_pgid: usize, signum: usize) -> Result<(), &'static str> {
    x86_64::instructions::interrupts::without_interrupts(|| PROCESS_TABLE.lock().sys_killpg(caller_pid, target_pgid, signum))
}

pub fn sys_tcsetpgrp(caller_pid: usize, pgid: usize) -> Result<(), &'static str> {
    x86_64::instructions::interrupts::without_interrupts(|| PROCESS_TABLE.lock().sys_tcsetpgrp(caller_pid, pgid))
}

