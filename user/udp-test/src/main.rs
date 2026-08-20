#![allow(warnings)]
#![no_std]
#![no_main]

use core::panic::PanicInfo;
use nova_net;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    nova_net::print("udp-test panic!\n");
    nova_net::sys_exit(1);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    nova_net::print("UDP-TEST STARTED!\n");

    let sock_res = nova_net::sys_socket(2, 2, 17); // AF_INET, SOCK_DGRAM, IPPROTO_UDP
    if sock_res < 0 {
        nova_net::print("Failed to create UDP socket\n");
        nova_net::sys_exit(1);
    }
    let sock = sock_res as usize;

    if nova_net::sys_bind(sock, 12345) < 0 {
        nova_net::print("Failed to bind UDP socket\n");
        nova_net::sys_close(sock);
        nova_net::sys_exit(1);
    }
    
    // DNS request payload for example.com
    let dns_query: [u8; 29] = [
        0x12, 0x34, // Transaction ID
        0x01, 0x00, // Flags (Standard query)
        0x00, 0x01, // Questions: 1
        0x00, 0x00, // Answer RRs: 0
        0x00, 0x00, // Authority RRs: 0
        0x00, 0x00, // Additional RRs: 0
        // Queries
        0x07, b'e', b'x', b'a', b'm', b'p', b'l', b'e',
        0x03, b'c', b'o', b'm',
        0x00, // Name (example.com)
        0x00, 0x01, // Type: A
        0x00, 0x01, // Class: IN
    ];

    // QEMU DNS is at 10.0.2.3 (167772675 in decimal), port 53
    let dest_ip: u32 = (10 << 24) | (0 << 16) | (2 << 8) | 3;
    let dest_port: u16 = 53;

    nova_net::print("Sending UDP DNS query...\n");
    if nova_net::sys_sendto(sock, dest_ip, dest_port, &dns_query) < 0 {
        nova_net::print("Failed to send UDP packet\n");
        nova_net::sys_close(sock);
        nova_net::sys_exit(1);
    }

    nova_net::print("Waiting for UDP response...\n");
    let mut buf = [0u8; 512];
    let mut src_ip: u32 = 0;
    let mut src_port: u16 = 0;

    let recv_res = nova_net::sys_recvfrom(sock, &mut buf, &mut src_ip, &mut src_port);
    if recv_res < 0 {
        nova_net::print("Failed to receive UDP packet (timeout?)\n");
        nova_net::sys_close(sock);
        nova_net::sys_exit(1);
    }

    nova_net::print("UDP TEST: PASS\n");
    
    nova_net::sys_close(sock);
    nova_net::sys_exit(0);
}
