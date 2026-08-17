#![no_std]
#![no_main]

use core::str;
use core::panic::PanicInfo;

extern crate nova_net;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    nova_net::print("Panic occurred!\n");
    nova_net::sys_exit(1);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    nova_net::print("HTTP-GET STARTED!\n");
    let mut url_buf = [0u8; 128];
    
    nova_net::print("Calling sys_open...\n");
    let fd = nova_net::sys_open("/tmp_curl_url");
    if fd < 0 {
        nova_net::print("Failed to open /tmp_curl_url\n");
        nova_net::sys_exit(1);
    }
    
    nova_net::print("Calling sys_read...\n");
    let n = nova_net::sys_read(fd as usize, &mut url_buf);
    nova_net::print("Returned from sys_read!\n");
    nova_net::print("Calling sys_close...\n");
    nova_net::sys_close(fd as usize);
    nova_net::print("Returned from sys_close!\n");
    
    if n <= 0 {
        nova_net::print("Failed to read url\n");
        nova_net::sys_exit(1);
    }
    nova_net::print("Parsing utf8...\n");
    let mut url = core::str::from_utf8(&url_buf[..n as usize]).unwrap_or("http://10.0.2.2:8080/");
    
    nova_net::print("Trimming whitespace...\n");
    // Trim whitespace/newlines
    let mut len = url.len();
    while len > 0 && (url.as_bytes()[len - 1] == b'\n' || url.as_bytes()[len - 1] == b'\r' || url.as_bytes()[len - 1] == b' ' || url.as_bytes()[len - 1] == b'\0') {
        len -= 1;
    }
    url = &url[..len];
    nova_net::print("Checking https...\n");
    if url.len() >= 8 {
        let mut is_https = true;
        let https = b"https://";
        for i in 0..8 {
            if url.as_bytes()[i] != https[i] {
                is_https = false;
                break;
            }
        }
        if is_https {
            nova_net::print("HTTPS is not supported yet.\n");
            nova_net::sys_exit(1);
        }
    }
    
    let mut url_without_proto = url;
    if url.len() >= 7 {
        let mut is_http = true;
        let http = b"http://";
        for i in 0..7 {
            if url.as_bytes()[i] != http[i] {
                is_http = false;
                break;
            }
        }
        if is_http {
            url_without_proto = &url[7..];
        }
    }
    
    let mut slash_idx = None;
    for (i, &b) in url_without_proto.as_bytes().iter().enumerate() {
        if b == b'/' {
            slash_idx = Some(i);
            break;
        }
    }
    
    let (host, path) = if let Some(idx) = slash_idx {
        (&url_without_proto[..idx], &url_without_proto[idx..])
    } else {
        (url_without_proto, "/")
    };
    
    nova_net::print("HTTP-GET: Starting request to: ");
    nova_net::print(url);
    nova_net::print("\nResolving ");
    nova_net::print(host);
    nova_net::print("...\n");
    
    let host_only = if let Some(colon_idx) = host.find(':') {
        &host[..colon_idx]
    } else {
        host
    };
    
    let mut parts = host_only.split('.');
    let is_ip = host_only.chars().all(|c| c.is_ascii_digit() || c == '.') && parts.clone().count() == 4;
    
    nova_net::print("HTTP: DNS lookup start\n");
    let ip = if is_ip {
        let a = parts.next().unwrap().parse::<u32>().unwrap();
        let b = parts.next().unwrap().parse::<u32>().unwrap();
        let c = parts.next().unwrap().parse::<u32>().unwrap();
        let d = parts.next().unwrap().parse::<u32>().unwrap();
        (a << 24) | (b << 16) | (c << 8) | d
    } else {
        match nova_net::sys_dns_resolve(host) {
            Some(ip) => {
                nova_net::print("HTTP: DNS lookup return code: OK\n");
                let mut buf = [0u8; 32];
                // Simple IP print
                // not strictly necessary, we can just say "resolved"
                nova_net::print("HTTP: resolved address OK\n");
                ip
            },
            None => {
                nova_net::print("HTTP: DNS lookup return code: ERROR\n");
                nova_net::print("Failed to resolve host\n");
                nova_net::sys_exit(1);
            }
        }
    };
    
    let port = if let Some(colon_idx) = host.find(':') {
        host[colon_idx + 1..].parse::<u16>().unwrap_or(80)
    } else {
        80
    };
    
    nova_net::print("Connecting to server...\n");
    let sock_res = nova_net::sys_socket(2, 1, 6);
    if sock_res < 0 {
        nova_net::print("Failed to create socket\n");
        nova_net::sys_exit(1);
    }
    let sock = sock_res as usize;
    
    if nova_net::sys_connect(sock, ip, port) < 0 {
        nova_net::print("Failed to connect\n");
        nova_net::sys_close(sock);
        nova_net::sys_exit(1);
    }
    
    nova_net::print("Sending GET request...\n");
    let mut req_buf = [0u8; 128];
    let req_len = nova_net::format_http_get(&mut req_buf, host, path);
    
    if nova_net::sys_send(sock, &req_buf[..req_len]) < 0 {
        nova_net::print("Failed to send request\n");
        nova_net::sys_close(sock);
        nova_net::sys_exit(1);
    }
    
    nova_net::print("Response:\n------------------------\n");
    let mut buf = [0u8; 128];
    loop {
        let recv_res = nova_net::sys_recv(sock, &mut buf);
        if recv_res < 0 {
            break;
        }
        let len = recv_res as usize;
        if len == 0 {
            break;
        }
        for i in 0..len {
            let c = buf[i];
            if c >= 32 && c <= 126 || c == b'\n' || c == b'\r' {
                let s = [c];
                if let Ok(st) = str::from_utf8(&s) {
                    nova_net::print(st);
                }
            } else {
                nova_net::print("?");
            }
        }
    }
    nova_net::print("\n------------------------\n");
    
    nova_net::sys_close(sock);
    nova_net::sys_exit(0);
}
