pub fn resolve(domain: &str) -> Option<u32> {
    crate::drivers::storage::serial_print("[DNS] query start via smoltcp\n");
    crate::drivers::storage::serial_print(&alloc::format!("[DNS] query started for {}\n", domain));
    
    let query = match crate::net::smoltcp::with_smoltcp_sockets(|sockets| {
        let handle = crate::net::smoltcp::SMOLTCP_DNS_SOCKET_HANDLE.lock().unwrap();
        let socket = sockets.get_mut::<smoltcp::socket::dns::Socket>(handle);
        crate::net::smoltcp::with_smoltcp_context(|ctx| {
            socket.start_query(ctx, domain, smoltcp::wire::DnsQueryType::A)
        })
    }) {
        Ok(q) => q,
        Err(_) => {
            crate::drivers::storage::serial_print("[DNS] query start failed\n");
            return None;
        }
    };
    
    let start_ticks = crate::drivers::timer::TimerDriver::get_ticks();
    
    loop {
        crate::net::smoltcp::poll_smoltcp();
        
        let result = crate::net::smoltcp::with_smoltcp_sockets(|sockets| {
            let handle = crate::net::smoltcp::SMOLTCP_DNS_SOCKET_HANDLE.lock().unwrap();
            let socket = sockets.get_mut::<smoltcp::socket::dns::Socket>(handle);
            socket.get_query_result(query)
        });
        
        match result {
            Ok(addrs) => {
                for addr in addrs {
                    #[allow(irrefutable_let_patterns)]
                    if let smoltcp::wire::IpAddress::Ipv4(ipv4) = addr {
                        crate::drivers::storage::serial_print(&alloc::format!("[DNS] resolved {} to {}\n", domain, ipv4));
                        crate::drivers::storage::serial_print("[DNS] final result=SUCCESS\n");
                        let bytes = ipv4.octets();
                        let ip = ((bytes[0] as u32) << 24) | ((bytes[1] as u32) << 16) | ((bytes[2] as u32) << 8) | (bytes[3] as u32);
                        return Some(ip);
                    }
                }
                return None;
            }
            Err(smoltcp::socket::dns::GetQueryResultError::Pending) => {
                let current_ticks = crate::drivers::timer::TimerDriver::get_ticks();
                if current_ticks - start_ticks > 300 { // 3 seconds timeout
                    crate::drivers::storage::serial_print("[DNS] query timeout\n");
                    crate::net::smoltcp::with_smoltcp_sockets(|sockets| {
                        let handle = crate::net::smoltcp::SMOLTCP_DNS_SOCKET_HANDLE.lock().unwrap();
                        let socket = sockets.get_mut::<smoltcp::socket::dns::Socket>(handle);
                        socket.cancel_query(query);
                    });
                    return None;
                }
                // Wait for interrupts to reduce busy looping
                x86_64::instructions::interrupts::enable_and_hlt();
                x86_64::instructions::interrupts::disable();
            }
            Err(e) => {
                crate::drivers::storage::serial_print(&alloc::format!("[DNS] query failed: {:?}\n", e));
                return None;
            }
        }
    }
}
