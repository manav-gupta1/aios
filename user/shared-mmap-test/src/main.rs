#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::arch::asm;

pub const SYS_WRITE: usize = 1;
pub const SYS_EXIT: usize = 2;
pub const SYS_WAITPID: usize = 5;
pub const SYS_FORK: usize = 10;
pub const SYS_MMAP: usize = 11;
pub const SYS_MUNMAP: usize = 12;

pub const PROT_READ: usize = 1;
pub const PROT_WRITE: usize = 2;
pub const MAP_SHARED: usize = 0x01;
pub const MAP_PRIVATE: usize = 0x02;
pub const MAP_ANONYMOUS: usize = 0x20;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys_exit(1);
    loop {}
}

unsafe fn syscall1(num: usize, arg1: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") num,
            in("rdi") arg1,
            lateout("rax") ret,
            options(nostack, preserves_flags)
        );
    }
    ret
}

unsafe fn syscall3(num: usize, arg1: usize, arg2: usize, arg3: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") num,
            in("rdi") arg1,
            in("rsi") arg2,
            in("rdx") arg3,
            lateout("rax") ret,
            options(nostack, preserves_flags)
        );
    }
    ret
}

unsafe fn syscall6(num: usize, arg1: usize, arg2: usize, arg3: usize, arg4: usize, arg5: usize, arg6: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") num,
            in("rdi") arg1,
            in("rsi") arg2,
            in("rdx") arg3,
            in("rcx") arg4,
            in("r8") arg5,
            in("r9") arg6,
            lateout("rax") ret,
            options(nostack, preserves_flags)
        );
    }
    ret
}

fn sys_write(fd: usize, buf: &[u8]) {
    unsafe { syscall3(SYS_WRITE, fd, buf.as_ptr() as usize, buf.len()) };
}

fn print(s: &str) {
    sys_write(1, s.as_bytes());
}

fn sys_exit(status: i32) {
    unsafe { syscall1(SYS_EXIT, status as usize) };
}

fn sys_fork() -> usize {
    unsafe {
        let ret: usize;
        asm!(
            "int 0x80",
            in("rax") SYS_FORK,
            lateout("rax") ret,
            options(nostack, preserves_flags)
        );
        ret
    }
}

fn sys_waitpid(pid: usize) -> usize {
    unsafe { syscall1(SYS_WAITPID, pid) }
}

fn sys_mmap(length: usize, prot: usize, flags: usize, fd: usize, offset: usize) -> usize {
    unsafe { syscall6(SYS_MMAP, length, prot, flags, fd, offset, 0) }
}

fn sys_munmap(addr: usize, length: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_MUNMAP,
            in("rdi") addr,
            in("rsi") length,
            lateout("rax") ret,
            options(nostack, preserves_flags)
        );
    }
    ret
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    print("shared-mmap-test: Starting test...\n");

    let size = 4096;
    let prot = PROT_READ | PROT_WRITE;
    let flags = MAP_SHARED | MAP_ANONYMOUS;

    let addr = sys_mmap(size, prot, flags, 0, 0);
    if addr == usize::MAX {
        print("shared-mmap-test: mmap failed!\n");
        sys_exit(1);
    }

    let ptr = addr as *mut u64;

    unsafe {
        core::ptr::write_volatile(ptr, 42);
    }

    let pid = sys_fork();

    if pid == 0 {
        // Child process
        unsafe {
            // Read initial value written by parent
            let val = core::ptr::read_volatile(ptr);
            if val != 42 {
                print("Child: Error: Did not read 42 from shared memory!\n");
                sys_exit(1);
            }
            print("Child: Read 42 from shared memory. Writing 1337...\n");
            
            core::ptr::write_volatile(ptr, 1337);
        }
        sys_exit(0);
    } else {
        // Parent process
        // Wait for child to exit
        sys_waitpid(pid);

        unsafe {
            let val = core::ptr::read_volatile(ptr);
            if val == 1337 {
                print("Parent: Read 1337 from shared memory. Test passed!\n");
            } else {
                print("Parent: Error: Expected 1337, got something else!\n");
                sys_exit(1);
            }
        }
    }

    let res = sys_munmap(addr, size);
    if res == usize::MAX {
        print("shared-mmap-test: munmap failed!\n");
        sys_exit(1);
    }

    print("shared-mmap-test: Success.\n");
    sys_exit(0);
    loop {}
}
