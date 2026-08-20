#![allow(warnings)]
#![no_std]
#![no_main]
use core::panic::PanicInfo;
extern crate nova_net;
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { nova_net::sys_exit(1); }
#[no_mangle]
pub extern "C" fn _start() -> ! {
    let sock = nova_net::sys_socket(2, 1, 6) as usize;
    let ip = (10<<24)|(0<<16)|(2<<8)|2;
    nova_net::sys_connect(sock, ip, 8000);
    nova_net::sys_exit(0);
}
