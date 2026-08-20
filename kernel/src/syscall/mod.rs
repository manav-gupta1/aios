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
pub const SYS_MMAP: usize = 11;
pub const SYS_MUNMAP: usize = 12;
pub const SYS_OPEN: usize = 13;
pub const SYS_KILL: usize = 14;
pub const SYS_SIGACTION: usize = 15;
pub const SYS_SETPGID: usize = 16;
pub const SYS_KILLPG: usize = 17;
pub const SYS_TCSETPGRP: usize = 18;
pub const SYS_SOCKET: usize = 19;
pub const SYS_BIND: usize = 20;
pub const SYS_SENDTO: usize = 21;
pub const SYS_RECVFROM: usize = 22;
pub const SYS_CONNECT: usize = 23;
pub const SYS_SEND: usize = 24;
pub const SYS_RECV: usize = 25;
pub const SYS_DNS_RESOLVE: usize = 26;
pub const SYS_PING: usize = 27;
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
            let status = arg1 as i32;
            crate::process::PROCESS_TABLE.lock().exit_process(pid, status);
            crate::task::scheduler::exit_current_task();
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
                match crate::process::waitpid(pid, target_pid, false) {
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

            crate::drivers::storage::serial_print("SYS_READ called\n");

            if !crate::memory::validate_user_buffer(ptr, len) {
                crate::drivers::storage::serial_print("SYS_READ: validate_user_buffer failed\n");
                return u64::MAX;
            }

            let slice = unsafe { core::slice::from_raw_parts_mut(ptr, len) };
            loop {
                match crate::process::read_fd(pid, fd as usize, slice) {
                    Ok(n) => {
                        crate::drivers::storage::serial_print("SYS_READ: read successful\n");
                        return n as u64;
                    }
                    Err(crate::process::PipeReadResult::WouldBlock) => {
                        x86_64::instructions::interrupts::enable();
                        x86_64::instructions::hlt();
                    }
                    Err(_) => {
                        crate::drivers::storage::serial_print("SYS_READ: read failed\n");
                        return u64::MAX;
                    }
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
            // Let the process table handle closing the FD
            match crate::process::close_fd(pid, fd) {
                Ok(()) => {
                    crate::net::smoltcp::poll_smoltcp();
                    0
                },
                Err(_) => u64::MAX,
            }
        }

        SYS_FORK => {
            match crate::process::fork_current_process(pid, frame_ptr) {
                Ok(child_pid) => child_pid as u64,
                Err(_) => u64::MAX,
            }
        }

        SYS_MMAP => {
            let length = arg1;
            let prot = arg2;
            let flags = arg3;
            let fd = unsafe { *frame_ptr.add(12) } as usize; // rcx (arg4)
            let offset = unsafe { *frame_ptr.add(7) } as usize; // r8 (arg5)
            
            match crate::memory::mmap::do_mmap(0, length, prot, flags, fd, offset) {
                Ok(addr) => addr as u64,
                Err(_) => u64::MAX,
            }
        }

        SYS_MUNMAP => {
            let addr = arg1;
            let length = arg2;
            match crate::memory::mmap::do_munmap(addr, length) {
                Ok(_) => 0,
                Err(_) => u64::MAX,
            }
        }

        SYS_OPEN => {
            let ptr = arg1 as *const u8;
            let len = arg2;

            if !crate::memory::validate_user_buffer(ptr, len) {
                crate::drivers::storage::serial_print("SYS_OPEN: validate_user_buffer failed!\n");
                return u64::MAX;
            }

            let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
            let path_str = match core::str::from_utf8(slice) {
                Ok(s) => s,
                Err(_) => {
                    crate::drivers::storage::serial_print("SYS_OPEN: invalid utf8!\n");
                    return u64::MAX;
                }
            };

            crate::drivers::storage::serial_print("SYS_OPEN: trying to open ");
            crate::drivers::storage::serial_print(path_str);
            crate::drivers::storage::serial_print("\n");

            match crate::process::open_file(pid, path_str) {
                Ok(fd) => fd as u64,
                Err(e) => {
                    crate::drivers::storage::serial_print("SYS_OPEN: open_file failed: ");
                    crate::drivers::storage::serial_print(e);
                    crate::drivers::storage::serial_print("\n");
                    u64::MAX
                }
            }
        }

        SYS_KILL => {
            let target_pid = arg1;
            let signum = arg2;
            match crate::process::sys_kill(pid, target_pid, signum) {
                Ok(_) => 0,
                Err(_) => u64::MAX,
            }
        }

        SYS_SIGACTION => {
            let signum = arg1;
            let act_ptr = arg2;
            let oldact_ptr = arg3;
            match crate::process::sys_sigaction(pid, signum, act_ptr, oldact_ptr) {
                Ok(_) => 0,
                Err(_) => u64::MAX,
            }
        }

        SYS_SETPGID => {
            let target_pid = arg1;
            let pgid = arg2;
            match crate::process::sys_setpgid(pid, target_pid, pgid) {
                Ok(_) => 0,
                Err(_) => u64::MAX,
            }
        }

        SYS_KILLPG => {
            let pgid = arg1;
            let signum = arg2;
            match crate::process::sys_killpg(pid, pgid, signum) {
                Ok(_) => 0,
                Err(_) => u64::MAX,
            }
        }

        SYS_TCSETPGRP => {
            let pgid = arg1;
            match crate::process::sys_tcsetpgrp(pid, pgid) {
                Ok(_) => 0,
                Err(_) => u64::MAX,
            }
        }

        SYS_SOCKET => {
            let domain = arg1;
            let type_ = arg2;
            let protocol = arg3;
            
            crate::drivers::storage::serial_print(&alloc::format!("SYS_SOCKET domain={}, type={}, proto={}\n", domain, type_, protocol));
            
            if domain != 2 {
                crate::drivers::storage::serial_print("SYS_SOCKET: domain != 2\n");
                return u64::MAX;
            }
            
            let sock_proto = if type_ == 2 && protocol == 0 {
                crate::net::socket::SocketProtocol::UDP
            } else if type_ == 1 && protocol == 6 {
                crate::net::socket::SocketProtocol::TCP
            } else {
                crate::drivers::storage::serial_print("SYS_SOCKET: bad type/proto\n");
                return u64::MAX;
            };
            
            let mut table = crate::net::socket::SOCKET_TABLE.lock();
            let sock_id = table.alloc_socket(sock_proto);
            drop(table);
            
            let mut ptable = crate::process::PROCESS_TABLE.lock();
            let proc = ptable.processes.iter_mut().find(|p| p.pid == pid);
            if let Some(p) = proc {
                let fd = p.alloc_fd(crate::process::FileDescriptor::Socket(sock_id));
                crate::drivers::storage::serial_print(&alloc::format!("SYS_SOCKET: success fd={}\n", fd));
                fd as u64
            } else {
                crate::net::socket::SOCKET_TABLE.lock().close_socket(sock_id);
                crate::drivers::storage::serial_print("SYS_SOCKET: proc not found\n");
                u64::MAX
            }
        }

        SYS_BIND => {
            let fd = arg1;
            let port = arg2 as u16;
            let ptable = crate::process::PROCESS_TABLE.lock();
            let proc = ptable.processes.iter().find(|p| p.pid == pid);
            if let Some(p) = proc {
                if let Some(crate::process::FileDescriptor::Socket(sock_id)) = p.get_fd(fd) {
                    drop(ptable);
                    match crate::net::socket::sys_bind(sock_id, port) {
                        Ok(_) => 0,
                        Err(_) => u64::MAX,
                    }
                } else {
                    u64::MAX
                }
            } else {
                u64::MAX
            }
        }

        SYS_SENDTO => {
            let fd = arg1;
            let buf_ptr = arg2 as *const u8;
            let len = arg3;
            let dest_ip = unsafe { *frame_ptr.add(12) } as u32; // rcx
            let dest_port = unsafe { *frame_ptr.add(7) } as u16; // r8
            
            if !crate::memory::validate_user_buffer(buf_ptr, len) {
                return u64::MAX;
            }
            let slice = unsafe { core::slice::from_raw_parts(buf_ptr, len) };
            
            let ptable = crate::process::PROCESS_TABLE.lock();
            let proc = ptable.processes.iter().find(|p| p.pid == pid);
            if let Some(p) = proc {
                if let Some(crate::process::FileDescriptor::Socket(sock_id)) = p.get_fd(fd) {
                    drop(ptable);
                    match crate::net::socket::sys_sendto(sock_id, dest_ip, dest_port, slice) {
                        Ok(n) => n as u64,
                        Err(_) => u64::MAX,
                    }
                } else {
                    u64::MAX
                }
            } else {
                u64::MAX
            }
        }

        SYS_RECVFROM => {
            let fd = arg1;
            let buf_ptr = arg2 as *mut u8;
            let len = arg3;
            let src_ip_ptr = unsafe { *frame_ptr.add(12) } as *mut u32; // rcx
            let src_port_ptr = unsafe { *frame_ptr.add(7) } as *mut u16; // r8
            
            if !crate::memory::validate_user_buffer(buf_ptr, len) {
                return u64::MAX;
            }
            if src_ip_ptr as usize != 0 && !crate::memory::validate_user_buffer(src_ip_ptr as *const u8, 4) {
                return u64::MAX;
            }
            if src_port_ptr as usize != 0 && !crate::memory::validate_user_buffer(src_port_ptr as *const u8, 2) {
                return u64::MAX;
            }
            let slice = unsafe { core::slice::from_raw_parts_mut(buf_ptr, len) };
            
            let ptable = crate::process::PROCESS_TABLE.lock();
            let proc = ptable.processes.iter().find(|p| p.pid == pid);
            if let Some(p) = proc {
                if let Some(crate::process::FileDescriptor::Socket(sock_id)) = p.get_fd(fd) {
                    let task_id = p.main_task_id;
                    drop(ptable);
                    
                    let start_ticks = crate::drivers::timer::TimerDriver::get_ticks();
                    let timeout_ticks = 500; // 5 seconds
                    
                    loop {
                        match crate::net::socket::sys_recvfrom(sock_id, slice, task_id) {
                            Ok(crate::net::socket::RecvResult::Data { src_ip, src_port, len }) => {
                                if src_ip_ptr as usize != 0 {
                                    unsafe { *src_ip_ptr = src_ip; }
                                }
                                if src_port_ptr as usize != 0 {
                                    unsafe { *src_port_ptr = src_port; }
                                }
                                return len as u64;
                            }
                            Ok(crate::net::socket::RecvResult::WouldBlock) => {
                                if crate::drivers::timer::TimerDriver::get_ticks() - start_ticks > timeout_ticks {
                                    return (-1i64) as u64;
                                }
                                crate::task::scheduler::yield_current_task();
                            }
                            Err(_) => return u64::MAX,
                        }
                    }
                } else {
                    u64::MAX
                }
            } else {
                u64::MAX
            }
        }

        SYS_CONNECT => {
            let fd = arg1;
            let dest_ip = arg2 as u32;
            let dest_port = arg3 as u16;
            
            let ptable = crate::process::PROCESS_TABLE.lock();
            let proc = ptable.processes.iter().find(|p| p.pid == pid);
            if let Some(p) = proc {
                if let Some(crate::process::FileDescriptor::Socket(sock_id)) = p.get_fd(fd) {
                    let task_id = p.main_task_id;
                    drop(ptable);
                    let start_ticks = crate::drivers::timer::TimerDriver::get_ticks();
                    let timeout_ticks = 500; // 5 seconds (100 HZ PIT)
                    let mut retries = 0;
                    loop {
                        match crate::net::socket::sys_connect(sock_id, dest_ip, dest_port, task_id) {
                            Ok(()) => {
                                return 0;
                            },
                            Err("ConnectionRefused") => {
                                return (-1i64) as u64;
                            },
                            Err(_) => {
                                crate::net::smoltcp::poll_smoltcp();
                                let current_ticks = crate::drivers::timer::TimerDriver::get_ticks();
                                if current_ticks - start_ticks > timeout_ticks {
                                    return (-1i64) as u64;
                                }
                                
                                // Only retry every 50 ticks (500ms)
                                if (current_ticks - start_ticks) > (retries * 50) {
                                    retries += 1;
                                }
                                
                                crate::task::scheduler::yield_current_task();
                            }
                        }
                    }
                } else {
                    u64::MAX
                }
            } else {
                u64::MAX
            }
        }
        
        SYS_SEND => {
            let fd = arg1;
            let buf_ptr = arg2 as *const u8;
            let len = arg3;
            if !crate::memory::validate_user_buffer(buf_ptr, len) {
                return u64::MAX;
            }
            let slice = unsafe { core::slice::from_raw_parts(buf_ptr, len) };
            
            let ptable = crate::process::PROCESS_TABLE.lock();
            let proc = ptable.processes.iter().find(|p| p.pid == pid);
            if let Some(p) = proc {
                if let Some(crate::process::FileDescriptor::Socket(sock_id)) = p.get_fd(fd) {
                    drop(ptable);
                    match crate::net::socket::sys_send(sock_id, slice) {
                        Ok(n) => {
                            crate::net::smoltcp::poll_smoltcp();
                            n as u64
                        },
                        Err(_) => u64::MAX,
                    }
                } else {
                    u64::MAX
                }
            } else {
                u64::MAX
            }
        }
        
        SYS_RECV => {
            let fd = arg1;
            let buf_ptr = arg2 as *mut u8;
            let len = arg3;
            if !crate::memory::validate_user_buffer(buf_ptr, len) {
                return u64::MAX;
            }
            let slice = unsafe { core::slice::from_raw_parts_mut(buf_ptr, len) };
            
            let ptable = crate::process::PROCESS_TABLE.lock();
            let proc = ptable.processes.iter().find(|p| p.pid == pid);
            if let Some(p) = proc {
                if let Some(crate::process::FileDescriptor::Socket(sock_id)) = p.get_fd(fd) {
                    let task_id = p.main_task_id;
                    drop(ptable);
                    loop {
                        match crate::net::socket::sys_recv(sock_id, slice, task_id) {
                            Ok(crate::net::socket::RecvResult::Data { len, .. }) => return len as u64,
                            Ok(crate::net::socket::RecvResult::WouldBlock) => {
                                crate::net::smoltcp::poll_smoltcp();
                                crate::task::scheduler::yield_current_task();
                            }
                            Err(_) => return u64::MAX,
                        }
                    }
                } else {
                    u64::MAX
                }
            } else {
                u64::MAX
            }
        }
        
        SYS_DNS_RESOLVE => {
            crate::drivers::storage::serial_print("[KDNS SYSCALL]\n");
            let ptr = arg1 as *const u8;
            let len = arg2;
            let out_ptr = arg3 as *mut u32;
            
            if !crate::memory::validate_user_buffer(ptr, len) {
                return (-1i64) as u64;
            }
            if !crate::memory::validate_user_buffer(out_ptr as *const u8, 4) {
                return (-1i64) as u64;
            }
            
            let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
            if let Ok(name) = core::str::from_utf8(slice) {
                if let Some(ip) = crate::net::dns::resolve(name) {
                    let ip_str = alloc::format!("{}.{}.{}.{}", (ip >> 24) & 0xFF, (ip >> 16) & 0xFF, (ip >> 8) & 0xFF, ip & 0xFF);
                    crate::drivers::storage::serial_print("resolver result = SUCCESS\n");
                    crate::drivers::storage::serial_print(&alloc::format!("resolved IPv4 = {}\n", ip_str));
                    crate::drivers::storage::serial_print("before copy_to_user = OK\n");
                    
                    unsafe { *out_ptr = ip; }
                    
                    crate::drivers::storage::serial_print("after copy_to_user = OK\n");
                    crate::drivers::storage::serial_print("final syscall return = 0\n");
                    return 0;
                }
            }
            (-1i64) as u64
        }

        SYS_PING => {
            let dest_ip = arg1 as u32;
            let sequence = arg2 as u16;
            
            // Clear any old replies in the socket queue
            while crate::net::smoltcp::smoltcp_ping_recv(1234).is_some() {}
            
            let start_ticks = crate::drivers::timer::TimerDriver::get_ticks();
            let timeout_ticks = 200; // 2 seconds at 100Hz
            
            loop {
                if crate::net::smoltcp::smoltcp_ping_send(dest_ip, 1234, sequence, b"ping").is_ok() {
                    // Send succeeded! Now wait for reply
                    loop {
                        crate::net::smoltcp::poll_smoltcp();
                        if matches!(crate::net::smoltcp::smoltcp_ping_recv(1234), Some((src_ip, ident, rx_seq)) if src_ip == dest_ip && ident == 1234 && rx_seq == sequence) {
                            return 0;
                        }
                        
                        let current_ticks = crate::drivers::timer::TimerDriver::get_ticks();
                        if current_ticks - start_ticks > timeout_ticks {
                            return (-1i64) as u64;
                        }
                        crate::task::scheduler::yield_current_task();
                    }
                }
                
                // ARP resolving or buffer full, wait a bit
                let current_ticks = crate::drivers::timer::TimerDriver::get_ticks();
                if current_ticks - start_ticks > timeout_ticks {
                    return (-1i64) as u64;
                }
                crate::task::scheduler::yield_current_task();
            }
        }

        _ => u64::MAX,
    }
}
