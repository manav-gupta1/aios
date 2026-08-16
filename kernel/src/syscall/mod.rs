use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub const SYS_WRITE: usize = 1;
pub const SYS_EXIT: usize = 2;
pub const SYS_GETPID: usize = 3;
pub const SYS_WHOAMI: usize = 4;
pub const SYS_WAITPID: usize = 5;
pub const SYS_EXEC: usize = 6;
pub const SYS_READ: usize = 7;
pub const SYS_PIPE: usize = 8;
pub const SYS_CLOSE: usize = 9;
pub const SYS_FORK: usize = 10;

pub static LAST_SYSCALL_NUM: AtomicUsize = AtomicUsize::new(0);
pub static LAST_SYSCALL_PID: AtomicUsize = AtomicUsize::new(0);
pub static LAST_SYSCALL_IS_RING3: AtomicBool = AtomicBool::new(false);
pub static USER_OUTPUT_RECEIVED: AtomicBool = AtomicBool::new(false);

pub fn syscall_dispatch(
    num: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    saved_cs: u64,
    frame_ptr: *mut u64,
) -> u64 {
    let is_ring3 = (saved_cs & 3) == 3;
    LAST_SYSCALL_NUM.store(num, Ordering::Release);
    LAST_SYSCALL_IS_RING3.store(is_ring3, Ordering::Release);

    let pid = crate::process::current_pid();
    LAST_SYSCALL_PID.store(pid, Ordering::Release);

    match num {
        SYS_WRITE => {
            // Case A: 3-arg format: sys_write(fd, buf, len) where arg3 > 0
            if arg3 > 0 {
                let fd = arg1;
                let ptr = arg2 as *const u8;
                let len = arg3;

                if !crate::memory::validate_user_buffer(ptr, len) {
                    return u64::MAX;
                }

                let slice = unsafe { core::slice::from_raw_parts(ptr, len) };

                loop {
                    match crate::process::write_fd(pid, fd, slice) {
                        Ok(n) => return n as u64,
                        Err(crate::process::PipeWriteResult::WouldBlock) => {
                            x86_64::instructions::interrupts::enable();
                            x86_64::instructions::hlt();
                        }
                        Err(crate::process::PipeWriteResult::BrokenPipe) => {
                            // -32 as u64 (-EPIPE)
                            return (-32isize) as u64;
                        }
                        Err(_) => return u64::MAX,
                    }
                }
            } else {
                // Case B: Legacy 2-arg format: sys_write(buf, len) -> writes to stdout
                let ptr = arg1 as *const u8;
                let len = arg2;

                if !crate::memory::validate_user_buffer(ptr, len) {
                    return u64::MAX;
                }

                let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
                if let Ok(s) = core::str::from_utf8(slice) {
                    USER_OUTPUT_RECEIVED.store(true, Ordering::Release);
                    crate::console::write_str(s);
                }

                len as u64
            }
        }

        SYS_EXIT => {
            crate::process::exit_current_process(arg1 as i32);
        }

        SYS_GETPID => pid as u64,

        SYS_WHOAMI => {
            if is_ring3 {
                3 // Ring 3
            } else {
                0 // Ring 0
            }
        }

        SYS_WAITPID => {
            let target_pid = if arg1 == 0 || arg1 == usize::MAX {
                None
            } else {
                Some(arg1)
            };

            loop {
                match crate::process::waitpid(pid, target_pid) {
                    Ok((child_pid, status)) => {
                        if arg2 != 0 {
                            let status_ptr = arg2 as *mut i32;
                            if crate::memory::validate_user_buffer(
                                status_ptr as *const u8,
                                core::mem::size_of::<i32>(),
                            ) {
                                unsafe {
                                    *status_ptr = status;
                                }
                            }
                        }
                        return child_pid as u64;
                    }
                    Err(crate::process::WaitError::NoChild)
                    | Err(crate::process::WaitError::InvalidPid) => {
                        return u64::MAX;
                    }
                    Err(crate::process::WaitError::WouldBlock) => {
                        // Enable interrupts and wait for timer / child exit notification
                        x86_64::instructions::interrupts::enable();
                        x86_64::instructions::hlt();
                    }
                }
            }
        }

        SYS_EXEC => {
            let ptr = arg1 as *const u8;
            let len = arg2;

            if !crate::memory::validate_user_buffer(ptr, len) {
                return u64::MAX;
            }

            let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
            let path_str = match core::str::from_utf8(slice) {
                Ok(s) => s,
                Err(_) => return u64::MAX,
            };

            match crate::process::exec_current(pid, path_str) {
                Ok((entry_point, user_rsp)) => {
                    if !frame_ptr.is_null() {
                        unsafe {
                            // Update saved RIP (offset 15) and saved RSP (offset 18)
                            *frame_ptr.add(15) = entry_point;
                            *frame_ptr.add(18) = user_rsp;
                        }
                    }
                    0
                }
                Err(_) => u64::MAX,
            }
        }

        SYS_READ => {
            let fd = arg1;
            let ptr = arg2 as *mut u8;
            let len = arg3;

            if !crate::memory::validate_user_buffer(ptr, len) {
                return u64::MAX;
            }

            let slice = unsafe { core::slice::from_raw_parts_mut(ptr, len) };

            loop {
                match crate::process::read_fd(pid, fd, slice) {
                    Ok(n) => return n as u64,
                    Err(crate::process::PipeReadResult::WouldBlock) => {
                        x86_64::instructions::interrupts::enable();
                        x86_64::instructions::hlt();
                    }
                    Err(_) => return u64::MAX,
                }
            }
        }

        SYS_PIPE => {
            let ptr = arg1 as *mut i32;

            if !crate::memory::validate_user_buffer(
                ptr as *const u8,
                2 * core::mem::size_of::<i32>(),
            ) {
                return u64::MAX;
            }

            match crate::process::create_pipe(pid) {
                Ok((rfd, wfd)) => {
                    unsafe {
                        *ptr = rfd as i32;
                        *ptr.add(1) = wfd as i32;
                    }
                    0
                }
                Err(_) => u64::MAX,
            }
        }

        SYS_CLOSE => {
            let fd = arg1;
            match crate::process::close_fd(pid, fd) {
                Ok(()) => 0,
                Err(_) => u64::MAX,
            }
        }

        SYS_FORK => {
            match crate::process::fork_current_process(pid, frame_ptr) {
                Ok(child_pid) => child_pid as u64,
                Err(_) => u64::MAX,
            }
        }

        _ => u64::MAX,
    }
}
