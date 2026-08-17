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

#[inline(always)]
fn sys_socket(domain: usize, type_: usize, protocol: usize) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") 19usize,
            in("rdi") domain,
            in("rsi") type_,
            in("rdx") protocol,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
fn sys_bind(fd: usize, port: u16) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") 20usize,
            in("rdi") fd,
            in("rsi") port as usize,
            in("rdx") 0usize,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
fn sys_sendto(fd: usize, buf: &[u8], dest_ip: u32, dest_port: u16) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") 21usize,
            in("rdi") fd,
            in("rsi") buf.as_ptr() as usize,
            in("rdx") buf.len(),
            in("rcx") dest_ip as usize,
            in("r8") dest_port as usize,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
fn sys_recvfrom(fd: usize, buf: &mut [u8], src_ip: &mut u32, src_port: &mut u16) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") 22usize,
            in("rdi") fd,
            in("rsi") buf.as_mut_ptr() as usize,
            in("rdx") buf.len(),
            in("rcx") src_ip as *mut u32 as usize,
            in("r8") src_port as *mut u16 as usize,
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

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    sys_write("UDP Test starting...\n");
    
    // AF_INET = 2, SOCK_DGRAM = 2, IPPROTO_UDP = 0
    let fd = sys_socket(2, 2, 0);
    if fd < 0 {
        sys_write("Failed to create socket\n");
        sys_exit(1);
    }
    
    // Bind to local port 8080
    if sys_bind(fd as usize, 8080) < 0 {
        sys_write("Failed to bind socket\n");
        sys_exit(1);
    }
    sys_write("Socket bound to port 8080\n");
    
    let ip = u32::from_be_bytes([10, 0, 2, 2]);
    
    let msg = b"Hello from UDP test!\n";
    let sent = sys_sendto(fd as usize, msg, ip, 1234);
    if sent < 0 {
        sys_write("Failed to sendto\n");
        sys_exit(1);
    }
    sys_write("Sent UDP packet\n");
    
    let mut buf = [0u8; 128];
    let mut src_ip = 0u32;
    let mut src_port = 0u16;
    
    sys_write("Waiting for reply...\n");
    let recv = sys_recvfrom(fd as usize, &mut buf, &mut src_ip, &mut src_port);
    if recv < 0 {
        sys_write("Failed to recvfrom\n");
        sys_exit(1);
    }
    
    sys_write("Received UDP packet: ");
    
    let recv_len = recv as usize;
    if let Ok(s) = core::str::from_utf8(&buf[..recv_len]) {
        sys_write(s);
        sys_write("\n");
    } else {
        sys_write("<binary data>\n");
    }
    
    sys_close(fd as usize);
    sys_write("UDP Test finished successfully.\n");
    sys_exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    sys_exit(1);
}
