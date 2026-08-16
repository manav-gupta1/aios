#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::arch::asm;

pub const SIGTERM: usize = 15;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys_exit(1);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut pid = sys_fork();
    if pid == 0 {
        // Child: infinite loop
        loop {
            unsafe { asm!("pause") }
        }
    } else if pid > 0 {
        // Parent: wait a bit, then kill
        for _ in 0..1000000 {
            unsafe { asm!("pause") }
        }
        
        let kill_res = sys_kill(pid as usize, SIGTERM);
        if kill_res != 0 {
            print("signal-test: kill failed.\n");
            sys_exit(1);
        }
        
        let mut status: i32 = 0;
        let w = sys_waitpid(pid as usize, &mut status);
        if w == pid {
            // expected exit status for SIGTERM is 128 + 15 = 143
            if status == 143 {
                print("signal-test: Success.\n");
                sys_exit(0);
            } else {
                print("signal-test: Bad status.\n");
                sys_exit(1);
            }
        }
    }
    
    sys_exit(1);
}

fn print(s: &str) {
    let buf = s.as_bytes();
    unsafe {
        asm!(
            "syscall",
            in("rax") 1, // sys_write
            in("rdi") 1, // stdout
            in("rsi") buf.as_ptr(),
            in("rdx") buf.len(),
            out("rcx") _,
            out("r11") _,
        );
    }
}

fn sys_fork() -> isize {
    let res: isize;
    unsafe {
        asm!(
            "syscall",
            inout("rax") 10isize => res,
            out("rcx") _,
            out("r11") _,
        );
    }
    res
}

fn sys_kill(pid: usize, sig: usize) -> isize {
    let res: isize;
    unsafe {
        asm!(
            "syscall",
            inout("rax") 14isize => res,
            in("rdi") pid,
            in("rsi") sig,
            out("rcx") _,
            out("r11") _,
        );
    }
    res
}

fn sys_waitpid(pid: usize, status: *mut i32) -> isize {
    let res: isize;
    unsafe {
        asm!(
            "syscall",
            inout("rax") 5isize => res,
            in("rdi") pid,
            in("rsi") status as usize,
            out("rcx") _,
            out("r11") _,
        );
    }
    res
}

fn sys_exit(code: i32) -> ! {
    unsafe {
        asm!(
            "syscall",
            in("rax") 2,
            in("rdi") code,
            options(noreturn)
        );
    }
}
