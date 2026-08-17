#!/usr/bin/expect -f
set timeout 10
set img [exec sh -c "ls -t ./target/release/build/nova-boot/*/out/nova-os-bios.img | head -n 1"]

spawn qemu-system-x86_64 -drive format=raw,file=$img -display none -serial stdio -netdev user,id=net0 -device virtio-net,netdev=net0
expect "NOVA>"
send "curl http://example.com/\r"
expect "NOVA>"
send "echo alive\r"
expect "alive"
