#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

#[inline(always)]
fn sys_write(fd: usize, buf: &[u8]) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") 1usize,
            in("rdi") fd,
            in("rsi") buf.as_ptr() as usize,
            in("rdx") buf.len(),
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
fn sys_read(fd: usize, buf: &mut [u8]) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") 7usize,
            in("rdi") fd,
            in("rsi") buf.as_mut_ptr() as usize,
            in("rdx") buf.len(),
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
fn sys_pipe(pipefd: &mut [i32; 2]) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") 8usize,
            in("rdi") pipefd.as_mut_ptr() as usize,
            in("rsi") 0usize,
            in("rdx") 0usize,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
fn sys_close(fd: usize) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") 9usize,
            in("rdi") fd,
            in("rsi") 0usize,
            in("rdx") 0usize,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
fn print(s: &str) {
    sys_write(1, s.as_bytes());
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
    print("=== NOVA OS IPC PIPE TEST ===\n\n");

    // ----------------------------------------------------
    // Test 1: Basic Write and Read through Anonymous Pipe
    // ----------------------------------------------------
    let mut fds = [0i32; 2];
    if sys_pipe(&mut fds) != 0 {
        print("FAIL: pipe creation failed\n");
        sys_exit(1);
    }

    let rfd = fds[0] as usize;
    let wfd = fds[1] as usize;

    let msg = b"Hello through pipe";
    let written = sys_write(wfd, msg);
    if written != msg.len() as isize {
        print("FAIL: pipe write failed\n");
        sys_exit(2);
    }

    let mut buf = [0u8; 64];
    let nread = sys_read(rfd, &mut buf);
    if nread != msg.len() as isize || &buf[..msg.len()] != msg {
        print("FAIL: pipe read mismatch\n");
        sys_exit(3);
    }

    print("[TEST 1] Basic Read/Write: PASSED\n");
    print("Received: Hello through pipe\n");

    sys_close(rfd);
    sys_close(wfd);

    // ----------------------------------------------------
    // Test 2: Producer/Consumer Ordering
    // ----------------------------------------------------
    if sys_pipe(&mut fds) != 0 {
        print("FAIL: pipe 2 creation failed\n");
        sys_exit(4);
    }

    let rfd2 = fds[0] as usize;
    let wfd2 = fds[1] as usize;

    sys_write(wfd2, b"Msg1;");
    sys_write(wfd2, b"Msg2;");
    sys_write(wfd2, b"Msg3;");

    let mut order_buf = [0u8; 32];
    let nread2 = sys_read(rfd2, &mut order_buf);
    let expected = b"Msg1;Msg2;Msg3;";
    if nread2 != expected.len() as isize || &order_buf[..expected.len()] != expected {
        print("FAIL: ordering test failed\n");
        sys_exit(5);
    }

    print("[TEST 2] Message Ordering: PASSED\n");
    sys_close(rfd2);
    sys_close(wfd2);

    // ----------------------------------------------------
    // Test 3: EOF on Closed Writer
    // ----------------------------------------------------
    if sys_pipe(&mut fds) != 0 {
        print("FAIL: pipe 3 creation failed\n");
        sys_exit(6);
    }

    let rfd3 = fds[0] as usize;
    let wfd3 = fds[1] as usize;

    sys_write(wfd3, b"Data");
    sys_close(wfd3); // Close the only writer

    let mut eof_buf = [0u8; 16];
    let r1 = sys_read(rfd3, &mut eof_buf);
    if r1 != 4 || &eof_buf[..4] != b"Data" {
        print("FAIL: read before EOF failed\n");
        sys_exit(7);
    }

    let r2 = sys_read(rfd3, &mut eof_buf);
    if r2 != 0 {
        print("FAIL: expected EOF (0 bytes) after writer closed\n");
        sys_exit(8);
    }

    print("[TEST 3] EOF on Closed Writer: PASSED\n");
    sys_close(rfd3);

    // ----------------------------------------------------
    // Test 4: Broken Pipe Error on Closed Reader
    // ----------------------------------------------------
    if sys_pipe(&mut fds) != 0 {
        print("FAIL: pipe 4 creation failed\n");
        sys_exit(9);
    }

    let rfd4 = fds[0] as usize;
    let wfd4 = fds[1] as usize;

    sys_close(rfd4); // Close the only reader

    let w_res = sys_write(wfd4, b"Broken");
    if w_res != -32 {
        print("FAIL: expected -32 (EPIPE) on closed reader write\n");
        sys_exit(10);
    }

    print("[TEST 4] Broken Pipe (EPIPE): PASSED\n");
    sys_close(wfd4);

    // ----------------------------------------------------
    // Test 5: File Descriptor Recycling and Cleanup
    // ----------------------------------------------------
    if sys_pipe(&mut fds) != 0 {
        print("FAIL: pipe 5 creation failed\n");
        sys_exit(11);
    }

    // FDs should be 3 and 4
    if fds[0] != 3 || fds[1] != 4 {
        print("FAIL: FD allocation mismatch\n");
        sys_exit(12);
    }

    sys_close(3); // Close FD 3
    if sys_pipe(&mut fds) != 0 {
        print("FAIL: pipe 6 creation failed\n");
        sys_exit(13);
    }

    // FD 3 should be recycled, next FD is 5
    if fds[0] != 3 || fds[1] != 5 {
        print("FAIL: FD recycling mismatch\n");
        sys_exit(14);
    }

    sys_close(3);
    sys_close(4);
    sys_close(5);

    print("[TEST 5] FD Allocation & Cleanup: PASSED\n");

    print("\nALL IPC PIPE TESTS PASSED\n");
    sys_exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    print("PANIC in user space\n");
    sys_exit(99);
}
