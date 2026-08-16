#!/usr/bin/expect -f
set timeout 10
spawn cargo run --release -p nova-boot
expect "NOVA>"
send "run /bin/hello &\r"
expect "NOVA>"
send "jobs\r"
expect "NOVA>"
send "echo alive\r"
expect "alive"
