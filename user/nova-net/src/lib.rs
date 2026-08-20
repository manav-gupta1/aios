#![no_std]
#![allow(clippy::missing_safety_doc)]
use core::arch::asm;

pub const SYS_SOCKET: usize = 19;
pub const SYS_BIND: usize = 20;
pub const SYS_SENDTO: usize = 21;
pub const SYS_RECVFROM: usize = 22;
pub const SYS_CONNECT: usize = 23;
pub const SYS_SEND: usize = 24;
pub const SYS_RECV: usize = 25;
pub const SYS_DNS_RESOLVE: usize = 26;
pub const SYS_PING: usize = 27;
pub const SYS_CLOSE: usize = 9;

#[inline(always)]
pub fn sys_ping(dest_ip: u32, sequence: u16) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_PING,
            in("rdi") dest_ip as usize,
            in("rsi") sequence as usize,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
pub fn sys_socket(domain: usize, type_: usize, protocol: usize) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_SOCKET,
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
pub fn sys_connect(fd: usize, dest_ip: u32, dest_port: u16) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_CONNECT,
            in("rdi") fd,
            in("rsi") dest_ip as usize,
            in("rdx") dest_port as usize,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
pub fn sys_send(fd: usize, buf: &[u8]) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_SEND,
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
pub fn sys_recv(fd: usize, buf: &mut [u8]) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_RECV,
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
pub fn sys_bind(fd: usize, port: u16) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_BIND,
            in("rdi") fd,
            in("rsi") port as usize,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
pub fn sys_sendto(fd: usize, dest_ip: u32, dest_port: u16, buf: &[u8]) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_SENDTO,
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
pub fn sys_recvfrom(fd: usize, buf: &mut [u8], src_ip: &mut u32, src_port: &mut u16) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_RECVFROM,
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
pub fn sys_dns_resolve(name: &str) -> Option<u32> {
    let ret: isize;
    let mut ip: u32 = 0;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_DNS_RESOLVE,
            in("rdi") name.as_ptr() as usize,
            in("rsi") name.len(),
            in("rdx") &mut ip as *mut u32 as usize,
            lateout("rax") ret,
            options(nostack)
        );
    }
    if ret < 0 {
        None
    } else {
        Some(ip)
    }
}

#[inline(always)]
pub fn sys_close(fd: usize) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_CLOSE,
            in("rdi") fd,
            in("rsi") 0usize,
            in("rdx") 0usize,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

pub fn format_http_get(buf: &mut [u8], host: &str, path: &str) -> usize {
    let part1 = b"GET ";
    let part2 = b" HTTP/1.1\r\nHost: ";
    let part3 = b"\r\nConnection: close\r\n\r\n";
    
    let mut offset = 0;
    
    // Copy part1
    for &b in part1 {
        if offset < buf.len() { buf[offset] = b; offset += 1; }
    }
    // Copy path
    for &b in path.as_bytes() {
        if offset < buf.len() { buf[offset] = b; offset += 1; }
    }
    // Copy part2
    for &b in part2 {
        if offset < buf.len() { buf[offset] = b; offset += 1; }
    }
    // Copy host
    for &b in host.as_bytes() {
        if offset < buf.len() { buf[offset] = b; offset += 1; }
    }
    // Copy part3
    for &b in part3 {
        if offset < buf.len() { buf[offset] = b; offset += 1; }
    }
    
    offset
}

pub const SYS_WRITE: usize = 1;
pub const SYS_EXIT: usize = 2;
pub const SYS_READ: usize = 7;
pub const SYS_OPEN: usize = 13;

#[inline(always)]
pub fn sys_open(path: &str) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_OPEN,
            in("rdi") path.as_ptr() as usize,
            in("rsi") path.len(),
            in("rdx") 0usize,
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[inline(always)]
pub fn sys_read(fd: usize, buf: &mut [u8]) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_READ,
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
pub fn sys_write(msg: &str) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_WRITE,
            in("rdi") 1usize, // stdout
            in("rsi") msg.as_ptr() as usize,
            in("rdx") msg.len(),
            lateout("rax") ret,
            options(nostack)
        );
    }
    ret
}

#[no_mangle]
pub unsafe extern "C" fn bcmp(s1: *const core::ffi::c_void, s2: *const core::ffi::c_void, n: usize) -> i32 {
    let s1 = s1 as *const u8;
    let s2 = s2 as *const u8;
    let mut i = 0;
    while i < n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b {
            return (a as i32) - (b as i32);
        }
        i += 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const core::ffi::c_void, s2: *const core::ffi::c_void, n: usize) -> i32 {
    bcmp(s1, s2, n)
}

#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut core::ffi::c_void, c: i32, n: usize) -> *mut core::ffi::c_void {
    let s_u8 = s as *mut u8;
    let mut i = 0;
    while i < n {
        *s_u8.add(i) = c as u8;
        i += 1;
    }
    s
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut core::ffi::c_void, src: *const core::ffi::c_void, n: usize) -> *mut core::ffi::c_void {
    let dest_u8 = dest as *mut u8;
    let src_u8 = src as *const u8;
    let mut i = 0;
    while i < n {
        *dest_u8.add(i) = *src_u8.add(i);
        i += 1;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memchr(s: *const core::ffi::c_void, c: i32, n: usize) -> *const core::ffi::c_void {
    let s_u8 = s as *const u8;
    let mut i = 0;
    while i < n {
        if *s_u8.add(i) == c as u8 {
            return s_u8.add(i) as *const core::ffi::c_void;
        }
        i += 1;
    }
    core::ptr::null()
}

pub fn print(msg: &str) {
    sys_write(msg);
}

#[inline(always)]
pub fn sys_exit(code: usize) -> ! {
    unsafe {
        asm!(
            "int 0x80",
            in("rax") SYS_EXIT,
            in("rdi") code,
            options(noreturn)
        );
    }
}

