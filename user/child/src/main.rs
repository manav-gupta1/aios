#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

#[inline(always)]
fn sys_write(msg: &str) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") 1usize,
            in("rdi") msg.as_ptr() as usize,
            in("rsi") msg.len(),
            in("rdx") 0usize,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
fn sys_getpid() -> usize {
    let ret: usize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") 3usize,
            in("rdi") 0usize,
            in("rsi") 0usize,
            in("rdx") 0usize,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
fn sys_exit(code: i32) -> ! {
    unsafe {
        asm!(
            "int 0x80",
            in("rax") 2usize,
            in("rdi") code as usize,
            in("rsi") 0usize,
            in("rdx") 0usize,
            options(noreturn)
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let _pid = sys_getpid();
    sys_write("Hello from child process\n");
    sys_exit(7);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys_exit(1);
}
