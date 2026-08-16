#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::arch::asm;

pub const SYS_WRITE: usize = 1;
pub const SYS_EXIT: usize = 2;
pub const SYS_MMAP: usize = 11;
pub const SYS_MUNMAP: usize = 12;
pub const SYS_OPEN: usize = 13;

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

unsafe fn syscall2(num: usize, arg1: usize, arg2: usize) -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") num,
            in("rdi") arg1,
            in("rsi") arg2,
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

fn sys_open(path: &str) -> usize {
    unsafe { syscall2(SYS_OPEN, path.as_ptr() as usize, path.len()) }
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

fn sys_mmap(length: usize, prot: usize, flags: usize, fd: usize, offset: usize) -> usize {
    unsafe { syscall6(SYS_MMAP, length, prot, flags, fd, offset, 0) }
}

fn sys_munmap(addr: usize, length: usize) -> usize {
    unsafe { syscall2(SYS_MUNMAP, addr, length) }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    print("mmap-file-test: Starting test...\n");

    let fd = sys_open("/bin/hello");
    if fd == usize::MAX {
        print("mmap-file-test: Failed to open file!\n");
        sys_exit(1);
    }

    let size = 4096;
    let prot = PROT_READ | PROT_WRITE;
    let flags = MAP_PRIVATE;

    let addr = sys_mmap(size, prot, flags, fd, 0);
    
    if addr == usize::MAX {
        print("mmap-file-test: mmap failed!\n");
        sys_exit(1);
    }

    print("mmap-file-test: Mapped file successfully.\n");
    
    let ptr = addr as *mut u8;
    
    // Verify ELF header (first byte is 0x7F, 2nd is 'E', etc)
    unsafe {
        if *ptr.add(1) == b'E' && *ptr.add(2) == b'L' && *ptr.add(3) == b'F' {
            print("mmap-file-test: ELF magic found.\n");
        } else {
            print("mmap-file-test: ELF magic NOT found!\n");
            sys_exit(1);
        }
    }

    print("mmap-file-test: Modifying mapping (private)...\n");
    unsafe {
        *ptr.add(1) = b'A';
        *ptr.add(2) = b'B';
        *ptr.add(3) = b'C';
    }

    let res = sys_munmap(addr, size);
    if res == usize::MAX {
        print("mmap-file-test: munmap failed!\n");
        sys_exit(1);
    }

    // Map again to verify it didn't change the underlying file
    let addr2 = sys_mmap(size, prot, flags, fd, 0);
    if addr2 == usize::MAX {
        print("mmap-file-test: second mmap failed!\n");
        sys_exit(1);
    }

    unsafe {
        let ptr2 = addr2 as *const u8;
        if *ptr2.add(1) == b'A' {
            print("mmap-file-test: Error! Underlying file was modified!\n");
            sys_exit(1);
        } else {
            print("mmap-file-test: Underlying file is unchanged. Private map works!\n");
        }
    }

    sys_munmap(addr2, size);
    print("mmap-file-test: Success.\n");
    sys_exit(0);
    loop {}
}
