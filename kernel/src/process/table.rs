use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;
use super::pid::PidAllocator;
use super::process::{FileDescriptor, Process, ProcessAddressSpace, ProcessState};
use crate::ipc::{Pipe, PipeReadError, PipeWriteError};
use crate::task::TaskState;

#[derive(Debug, Clone)]
pub struct ProcessSnapshot {
    pub pid: usize,
    pub ppid: usize,
    pub state: ProcessState,
    pub name: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitError {
    NoChild,
    WouldBlock,
    #[allow(dead_code)]
    InvalidPid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeReadResult {
    WouldBlock,
    BadFd,
    NotReadable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeWriteResult {
    WouldBlock,
    BrokenPipe,
    BadFd,
    NotWritable,
}

pub struct ProcessTable {
    pub(crate) processes: Vec<Process>,
    pub(crate) pid_allocator: PidAllocator,
    initialized: bool,
}

impl ProcessTable {
    pub const fn new() -> Self {
        ProcessTable {
            processes: Vec::new(),
            pid_allocator: PidAllocator::new(),
            initialized: false,
        }
    }

    pub fn init(&mut self) {
        if self.initialized {
            return;
        }
        let init_proc = Process::new_init();
        self.processes.push(init_proc);
        self.initialized = true;
    }

    pub fn spawn_user_process(
        &mut self,
        parent_pid: usize,
        name: &'static str,
        rip: u64,
        rsp: u64,
        address_space: ProcessAddressSpace,
    ) -> Result<usize, &'static str> {
        let new_pid = self.pid_allocator.allocate();
        let main_task_id = crate::task::spawn_user(new_pid, name, rip, rsp);
        let proc = Process::new_user(new_pid, parent_pid, name, main_task_id, address_space);
        self.processes.push(proc);
        Ok(new_pid)
    }

    pub fn exit_process(&mut self, pid: usize, status: i32) {
        let mut parent_pid_to_wake = None;
        let mut parent_task_id_to_wake = None;
        let mut pids_to_wake_from_fds = Vec::new();

        // 1. Mark this process as Zombie and record exit status
        if let Some(proc) = self.processes.iter_mut().find(|p| p.pid == pid) {
            proc.state = ProcessState::Zombie;
            proc.exit_status = Some(status);
            let ppid = proc.ppid;

            // Close all open FDs for this exiting process and gather any tasks to wake
            for fd_opt in proc.fd_table.iter_mut() {
                if let Some(desc) = fd_opt.take() {
                    match desc {
                        FileDescriptor::PipeRead(pipe) => {
                            let (_, to_wake) = pipe.lock().close_read();
                            pids_to_wake_from_fds.extend(to_wake);
                        }
                        FileDescriptor::PipeWrite(pipe) => {
                            let (_, to_wake) = pipe.lock().close_write();
                            pids_to_wake_from_fds.extend(to_wake);
                        }
                        FileDescriptor::Socket(id) => {
                            crate::net::socket::SOCKET_TABLE.lock().close_socket(id);
                        }
                        _ => {}
                    }
                }
            }

            // Check if parent is Blocked waiting for this child
            if let Some(parent) = self.processes.iter_mut().find(|p| p.pid == ppid) {
                parent.pending_signals |= 1 << crate::process::signal::SIGCHLD;
                if parent.state == ProcessState::Blocked {
                    match parent.waiting_target_pid {
                        Some(None) => {
                            parent_pid_to_wake = Some(parent.pid);
                            parent_task_id_to_wake = Some(parent.main_task_id);
                        }
                        Some(Some(target_pid)) if target_pid == pid => {
                            parent_pid_to_wake = Some(parent.pid);
                            parent_task_id_to_wake = Some(parent.main_task_id);
                        }
                        _ => {}
                    }
                }
            }
        }

        // 2. Unblock parent if waiting
        if let Some(ppid) = parent_pid_to_wake {
            if let Some(parent) = self.processes.iter_mut().find(|p| p.pid == ppid) {
                parent.state = ProcessState::Ready;
                parent.waiting_target_pid = None;
            }
            if let Some(task_id) = parent_task_id_to_wake {
                crate::task::set_task_state(task_id, TaskState::Ready);
            }
        }

        // 3. Unblock any tasks waiting on closed FDs
        for wake_pid in pids_to_wake_from_fds {
            self.wake_process_by_pid(wake_pid);
        }

        // 4. Reparent orphan children of this exiting process to PID 1 (init)
        for proc in &mut self.processes {
            if proc.ppid == pid {
                proc.ppid = 1;
                proc.is_orphan = true;
            }
        }

        // 5. Mark scheduler task as Finished
        if let Some(proc) = self.processes.iter().find(|p| p.pid == pid) {
            crate::task::set_task_state(proc.main_task_id, TaskState::Finished);
        }
    }

    pub fn waitpid(
        &mut self,
        caller_pid: usize,
        target: Option<usize>,
        nohang: bool,
    ) -> Result<(usize, i32), WaitError> {
        if let Some(t) = target {
            // Find target child
            let child_idx = self
                .processes
                .iter()
                .position(|p| p.pid == t && p.ppid == caller_pid)
                .ok_or(WaitError::NoChild)?;

            if self.processes[child_idx].state == ProcessState::Zombie {
                let status = self.processes[child_idx].exit_status.unwrap_or(0);
                let main_task_id = self.processes[child_idx].main_task_id;
                self.processes[child_idx].address_space.unmap_all();
                self.processes.remove(child_idx);
                self.pid_allocator.deallocate(t);
                crate::task::scheduler::SCHEDULER.lock().remove_task(main_task_id);
                return Ok((t, status));
            } else {
                // Child is still running -> block caller unless nohang
                if !nohang {
                    if let Some(caller) = self.processes.iter_mut().find(|p| p.pid == caller_pid) {
                        caller.state = ProcessState::Blocked;
                        caller.waiting_target_pid = Some(Some(t));
                        crate::task::set_task_state(caller.main_task_id, TaskState::Blocked);
                    }
                }
                return Err(WaitError::WouldBlock);
            }
        } else {
            // Wait for ANY child
            let has_children = self.processes.iter().any(|p| p.ppid == caller_pid);
            if !has_children {
                return Err(WaitError::NoChild);
            }

            let zombie_idx = self
                .processes
                .iter()
                .position(|p| p.ppid == caller_pid && p.state == ProcessState::Zombie);

            if let Some(idx) = zombie_idx {
                let child_pid = self.processes[idx].pid;
                let status = self.processes[idx].exit_status.unwrap_or(0);
                let main_task_id = self.processes[idx].main_task_id;
                self.processes[idx].address_space.unmap_all();
                self.processes.remove(idx);
                self.pid_allocator.deallocate(child_pid);
                crate::task::scheduler::SCHEDULER.lock().remove_task(main_task_id);
                return Ok((child_pid, status));
            } else {
                // All children are still running -> block caller unless nohang
                if !nohang {
                    if let Some(caller) = self.processes.iter_mut().find(|p| p.pid == caller_pid) {
                        caller.state = ProcessState::Blocked;
                        caller.waiting_target_pid = Some(None);
                        crate::task::set_task_state(caller.main_task_id, TaskState::Blocked);
                    }
                }
                return Err(WaitError::WouldBlock);
            }
        }
    }

    pub fn reap_orphans(&mut self) {
        let mut to_remove = alloc::vec::Vec::new();
        
        for p in self.processes.iter() {
            if p.ppid == 1 && p.is_orphan && p.state == ProcessState::Zombie {
                to_remove.push(p.pid);
            }
        }
        
        for pid in to_remove {
            if let Some(idx) = self.processes.iter().position(|p| p.pid == pid) {
                let main_task_id = self.processes[idx].main_task_id;
                self.processes[idx].address_space.unmap_all();
                self.processes.remove(idx);
                self.pid_allocator.deallocate(pid);
                crate::task::scheduler::SCHEDULER.lock().remove_task(main_task_id);
            }
        }
    }

    pub fn exec_current_process(
        &mut self,
        caller_pid: usize,
        path: &str,
        elf_data: &[u8],
    ) -> Result<(u64, u64), &'static str> {
        let caller_idx = self
            .processes
            .iter()
            .position(|p| p.pid == caller_pid)
            .ok_or("Caller process not found")?;

        // 1. Unmap caller's old address space first
        self.processes[caller_idx].address_space.unmap_all();

        // 2. Load and map new ELF segments and stack
        let (loaded, new_address_space) =
            crate::elf::loader::load_elf(elf_data).map_err(|_| "Failed to load ELF")?;

        // 3. Assign new address space and update process name
        self.processes[caller_idx].address_space = new_address_space;
        let prog_name = path.rsplit('/').next().unwrap_or(path);
        self.processes[caller_idx].name = String::from(prog_name);

        // 4. Reset task execution context to the new entry point and user stack
        let task_id = self.processes[caller_idx].main_task_id;
        crate::task::reset_task_user_context(
            task_id,
            "user_exec",
            loaded.entry_point,
            loaded.user_rsp,
        );

        Ok((loaded.entry_point, loaded.user_rsp))
    }

    pub fn create_pipe(&mut self, caller_pid: usize) -> Result<(usize, usize), &'static str> {
        let proc = self
            .processes
            .iter_mut()
            .find(|p| p.pid == caller_pid)
            .ok_or("Process not found")?;

        let pipe = Arc::new(Mutex::new(Pipe::new()));
        let rfd = proc.alloc_fd(FileDescriptor::PipeRead(Arc::clone(&pipe)));
        let wfd = proc.alloc_fd(FileDescriptor::PipeWrite(pipe));

        Ok((rfd, wfd))
    }

    pub fn open_file(&mut self, caller_pid: usize, path: &str) -> Result<usize, &'static str> {
        let fs = crate::fs::FILESYSTEM.lock();
        if fs.read_file_bytes(path).is_err() {
            return Err("File not found");
        }
        
        let proc = self
            .processes
            .iter_mut()
            .find(|p| p.pid == caller_pid)
            .ok_or("Process not found")?;

        let fd = proc.alloc_fd(FileDescriptor::File(String::from(path), 0));
        Ok(fd)
    }

    pub fn read_fd(
        &mut self,
        caller_pid: usize,
        fd: usize,
        buf: &mut [u8],
    ) -> Result<usize, PipeReadResult> {
        let proc_opt = self.processes.iter().find(|p| p.pid == caller_pid);
        let mut fd_desc = match proc_opt {
            Some(p) => p.get_fd(fd).ok_or(PipeReadResult::BadFd)?,
            None => return Err(PipeReadResult::BadFd),
        };

        match fd_desc {
            FileDescriptor::PipeRead(pipe) => {
                let mut pipe_guard = pipe.lock();
                let res = pipe_guard.read(caller_pid, buf);
                match res {
                    Ok(n) => {
                        let to_wake = pipe_guard.take_writers_to_wake();
                        drop(pipe_guard);
                        for wake_pid in to_wake {
                            self.wake_process_by_pid(wake_pid);
                        }
                        Ok(n)
                    }
                    Err(PipeReadError::WouldBlock) => {
                        drop(pipe_guard);
                        if let Some(proc) = self.processes.iter_mut().find(|p| p.pid == caller_pid) {
                            proc.state = ProcessState::Blocked;
                            crate::task::set_task_state(proc.main_task_id, TaskState::Blocked);
                        }
                        Err(PipeReadResult::WouldBlock)
                    }
                }
            }
            FileDescriptor::Stdin => {
                let proc_pgid = self.processes.iter().find(|p| p.pid == caller_pid).map(|p| p.pgid).unwrap_or(0);
                if proc_pgid != crate::tty::TTY.lock().foreground_pgid() {
                    return Err(PipeReadResult::NotReadable);
                }

                match crate::tty::TTY.lock().read_user_buffer(caller_pid, buf) {
                    Ok(n) => Ok(n),
                    Err(()) => {
                        if let Some(proc) = self.processes.iter_mut().find(|p| p.pid == caller_pid) {
                            proc.state = ProcessState::Blocked;
                            crate::task::set_task_state(proc.main_task_id, TaskState::Blocked);
                        }
                        Err(PipeReadResult::WouldBlock)
                    }
                }
            }
            FileDescriptor::File(ref path, ref mut offset) => {
                let fs = crate::fs::FILESYSTEM.lock();
                match fs.read_file_bytes(path) {
                    Ok(file_bytes) => {
                        let available = file_bytes.len().saturating_sub(*offset);
                        let to_read = core::cmp::min(available, buf.len());
                        if to_read > 0 {
                            buf[..to_read].copy_from_slice(&file_bytes[*offset..*offset + to_read]);
                            *offset += to_read;
                            // Update the actual FD offset
                            if let Some(proc) = self.processes.iter_mut().find(|p| p.pid == caller_pid) {
                                proc.fd_table[fd] = Some(fd_desc.clone());
                            }
                        }
                        Ok(to_read)
                    }
                    Err(_) => Err(PipeReadResult::NotReadable),
                }
            }
            _ => Err(PipeReadResult::NotReadable),
        }
    }

    pub fn write_fd(
        &mut self,
        caller_pid: usize,
        fd: usize,
        buf: &[u8],
    ) -> Result<usize, PipeWriteResult> {
        let proc_opt = self.processes.iter().find(|p| p.pid == caller_pid);
        let fd_desc = match proc_opt {
            Some(p) => p.get_fd(fd).ok_or(PipeWriteResult::BadFd)?,
            None => return Err(PipeWriteResult::BadFd),
        };

        match fd_desc {
            FileDescriptor::PipeWrite(pipe) => {
                let mut pipe_guard = pipe.lock();
                let res = pipe_guard.write(caller_pid, buf);
                match res {
                    Ok(n) => {
                        let to_wake = pipe_guard.take_readers_to_wake();
                        drop(pipe_guard);
                        for wake_pid in to_wake {
                            self.wake_process_by_pid(wake_pid);
                        }
                        Ok(n)
                    }
                    Err(PipeWriteError::WouldBlock) => {
                        drop(pipe_guard);
                        if let Some(proc) = self.processes.iter_mut().find(|p| p.pid == caller_pid) {
                            proc.state = ProcessState::Blocked;
                            crate::task::set_task_state(proc.main_task_id, TaskState::Blocked);
                        }
                        Err(PipeWriteResult::WouldBlock)
                    }
                    Err(PipeWriteError::BrokenPipe) => {
                        drop(pipe_guard);
                        Err(PipeWriteResult::BrokenPipe)
                    }
                }
            }
            FileDescriptor::Stdout | FileDescriptor::Stderr => {
                match crate::tty::TTY.lock().write_user_buffer(caller_pid, buf) {
                    Ok(n) => Ok(n),
                    Err(()) => Err(PipeWriteResult::NotWritable),
                }
            }
            _ => Err(PipeWriteResult::NotWritable),
        }
    }

    pub fn close_fd(&mut self, caller_pid: usize, fd: usize) -> Result<(), &'static str> {
        let proc = self
            .processes
            .iter_mut()
            .find(|p| p.pid == caller_pid)
            .ok_or("Process not found")?;

        let desc = proc.close_fd(fd).ok_or("Bad file descriptor")?;
        let to_wake = match desc {
            FileDescriptor::PipeRead(pipe) => {
                let (_, list) = pipe.lock().close_read();
                list
            }
            FileDescriptor::PipeWrite(pipe) => {
                let (_, list) = pipe.lock().close_write();
                list
            }
            FileDescriptor::Socket(id) => {
                crate::net::socket::SOCKET_TABLE.lock().close_socket(id);
                Vec::new()
            }
            _ => Vec::new(),
        };

        for wake_pid in to_wake {
            self.wake_process_by_pid(wake_pid);
        }

        Ok(())
    }

    pub fn wake_process_by_pid(&mut self, pid: usize) {
        if let Some(proc) = self.processes.iter_mut().find(|p| p.pid == pid) {
            if proc.state == ProcessState::Blocked {
                proc.state = ProcessState::Ready;
                crate::task::set_task_state(proc.main_task_id, TaskState::Ready);
            }
        }
    }

    pub fn snapshots(&self) -> Vec<ProcessSnapshot> {
        let mut snaps = Vec::with_capacity(self.processes.len());
        for proc in &self.processes {
            snaps.push(ProcessSnapshot {
                pid: proc.pid,
                ppid: proc.ppid,
                state: proc.state,
                name: proc.name.clone(),
            });
        }
        snaps
    }

    pub fn fork_process(
        &mut self,
        caller_pid: usize,
        frame_ptr: *mut u64,
    ) -> Result<usize, &'static str> {
        crate::process::fork::do_fork(self, caller_pid, frame_ptr)
    }

    pub fn handle_cow_fault(&mut self, caller_pid: usize, fault_addr: x86_64::VirtAddr) -> Result<(), &'static str> {
        let page = x86_64::structures::paging::Page::<x86_64::structures::paging::Size4KiB>::containing_address(fault_addr);
        let proc = self
            .processes
            .iter_mut()
            .find(|p| p.pid == caller_pid)
            .ok_or("Process not found")?;

        if !proc.address_space.is_cow(page) {
            return Err("Not a COW page");
        }

        crate::memory::resolve_cow_page(page)?;
        proc.address_space.unmark_cow(page);
        Ok(())
    }
    pub fn get_process_mmap_regions(&self, pid: usize) -> Option<Vec<crate::process::MmapRegion>> {
        self.processes
            .iter()
            .find(|p| p.pid == pid)
            .map(|p| p.address_space.mmap_regions.clone())
    }

    pub fn sys_kill(&mut self, _caller_pid: usize, target_pid: usize, signum: usize) -> Result<(), &'static str> {
        if target_pid == 0 || signum == 0 || signum >= 64 {
            return Err("Invalid argument");
        }
        
        let target_idx = self.processes.iter().position(|p| p.pid == target_pid).ok_or("Process not found")?;
        let action = self.processes[target_idx].sig_actions[signum];
        let ppid = self.processes[target_idx].ppid;
        let main_task_id = self.processes[target_idx].main_task_id;
        let old_state = self.processes[target_idx].state;
        
        self.processes[target_idx].pending_signals |= 1 << signum;
        
        if signum != crate::process::signal::SIGKILL && action == crate::process::signal::SIG_IGN {
            return Ok(()); // Ignored
        }
        
        if action == crate::process::signal::SIG_DFL {
            match signum {
                crate::process::signal::SIGSTOP => {
                    self.processes[target_idx].is_stopped = true;
                    if old_state == ProcessState::Ready || old_state == ProcessState::Running {
                        self.processes[target_idx].state = ProcessState::Stopped;
                    }
                    crate::task::scheduler::SCHEDULER.lock().set_task_stopped(main_task_id, true);
                    
                    if let Some(parent) = self.processes.iter_mut().find(|p| p.pid == ppid) {
                        parent.pending_signals |= 1 << crate::process::signal::SIGCHLD;
                    }
                    return Ok(());
                },
                crate::process::signal::SIGCONT => {
                    self.processes[target_idx].is_stopped = false;
                    if old_state == ProcessState::Stopped {
                        self.processes[target_idx].state = ProcessState::Ready;
                    }
                    crate::task::scheduler::SCHEDULER.lock().set_task_stopped(main_task_id, false);
                    return Ok(());
                },
                crate::process::signal::SIGCHLD => {
                    return Ok(());
                },
                _ => {} // SIGKILL, SIGTERM, fall through to exit
            }
        } else {
            return Err("Not supported");
        }
        
        self.exit_process(target_pid, 128 + signum as i32);
        Ok(())
    }

    pub fn sys_sigaction(&mut self, caller_pid: usize, signum: usize, act_ptr: usize, oldact_ptr: usize) -> Result<(), &'static str> {
        if signum == 0 || signum >= 64 || signum == crate::process::signal::SIGKILL {
            return Err("Invalid argument");
        }
        
        let proc = self.processes.iter_mut().find(|p| p.pid == caller_pid).ok_or("Process not found")?;
        
        if oldact_ptr != 0 {
            if !crate::memory::validate_user_buffer(oldact_ptr as *const u8, 32) {
                return Err("Bad address");
            }
            unsafe {
                let oldact = oldact_ptr as *mut usize;
                *oldact = proc.sig_actions[signum];
                *oldact.add(1) = 0;
                *oldact.add(2) = 0;
                *oldact.add(3) = 0;
            }
        }
        if act_ptr != 0 {
            if !crate::memory::validate_user_buffer(act_ptr as *const u8, 32) {
                return Err("Bad address");
            }
            unsafe {
                let act = act_ptr as *const usize;
                let handler = *act;
                if handler != crate::process::signal::SIG_DFL && handler != crate::process::signal::SIG_IGN {
                    return Err("Custom handlers not supported");
                }
                proc.sig_actions[signum] = handler;
            }
        }
        Ok(())
    }

    pub fn sys_setpgid(&mut self, _caller_pid: usize, target_pid: usize, pgid: usize) -> Result<(), &'static str> {
        let p = self.processes.iter_mut().find(|p| p.pid == target_pid).ok_or("Process not found")?;
        // Minimal implementation: allow shell to set PGID freely
        p.pgid = if pgid == 0 { target_pid } else { pgid };
        Ok(())
    }

    pub fn sys_killpg(&mut self, _caller_pid: usize, target_pgid: usize, signum: usize) -> Result<(), &'static str> {
        if target_pgid == 0 || signum == 0 || signum >= 64 {
            return Err("Invalid argument");
        }
        
        // Find all pids in this pgid
        let mut pids_in_pg = Vec::new();
        for p in &self.processes {
            if p.pgid == target_pgid {
                pids_in_pg.push(p.pid);
            }
        }
        
        if pids_in_pg.is_empty() {
            return Err("Process group not found");
        }
        
        for pid in pids_in_pg {
            // We ignore errors for individual processes in killpg
            let _ = self.sys_kill(_caller_pid, pid, signum);
        }
        
        Ok(())
    }

    pub fn sys_tcsetpgrp(&mut self, _caller_pid: usize, pgid: usize) -> Result<(), &'static str> {
        // Find if this pgid exists
        let exists = self.processes.iter().any(|p| p.pgid == pgid);
        if !exists && pgid != 1 {
            return Err("Process group not found");
        }
        crate::tty::TTY.lock().set_foreground_pgid(pgid);
        Ok(())
    }
}

pub static PROCESS_TABLE: Mutex<ProcessTable> = Mutex::new(ProcessTable::new());
