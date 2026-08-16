#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::arch::asm;

pub const SYS_WRITE: usize = 1;
pub const SYS_EXIT: usize = 2;
pub const SYS_MMAP: usize = 11;
pub const SYS_MUNMAP: usize = 12;

pub const PROT_READ: usize = 1;
pub const PROT_WRITE: usize = 2;
pub const PROT_EXEC: usize = 4;
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
    asm!(
        "int 0x80",
        in("rax") num,
        in("rdi") arg1,
        lateout("rax") ret,
        options(nostack, preserves_flags)
    );
    ret
}

unsafe fn syscall2(num: usize, arg1: usize, arg2: usize) -> usize {
    let ret: usize;
    asm!(
        "int 0x80",
        in("rax") num,
        in("rdi") arg1,
        in("rsi") arg2,
        lateout("rax") ret,
        options(nostack, preserves_flags)
    );
    ret
}

unsafe fn syscall3(num: usize, arg1: usize, arg2: usize, arg3: usize) -> usize {
    let ret: usize;
    asm!(
        "int 0x80",
        in("rax") num,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        lateout("rax") ret,
        options(nostack, preserves_flags)
    );
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

fn sys_mmap(length: usize, prot: usize, flags: usize) -> usize {
    unsafe { syscall3(SYS_MMAP, length, prot, flags) }
}

fn sys_munmap(addr: usize, length: usize) -> usize {
    unsafe { syscall2(SYS_MUNMAP, addr, length) }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    print("mmap-test: Starting test...\n");

    let size = 4096;
    let prot = PROT_READ | PROT_WRITE;
    let flags = MAP_PRIVATE | MAP_ANONYMOUS;

    let addr = sys_mmap(size, prot, flags);
    
    if addr == usize::MAX {
        print("mmap-test: mmap failed!\n");
        sys_exit(1);
    }

    print("mmap-test: Mapped region successfully.\n");
    print("mmap-test: Writing to mapped region...\n");
    
    let ptr = addr as *mut u8;
    unsafe {
        for i in 0..10 {
            *ptr.add(i) = b'A' + i as u8;
        }
    }

    print("mmap-test: Read back: ");
    
    let mut buf = [0u8; 10];
    unsafe {
        for i in 0..10 {
            buf[i] = *ptr.add(i);
        }
    }
    
    sys_write(1, &buf);
    print("\n");

    print("mmap-test: Unmapping region...\n");
    let res = sys_munmap(addr, size);
    
    if res == usize::MAX {
        print("mmap-test: munmap failed!\n");
        sys_exit(1);
    }
    
    print("mmap-test: Success.\n");
    sys_exit(0);
    loop {}
}
