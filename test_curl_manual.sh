#!/bin/bash
img=$(ls -t ./target/release/build/nova-boot/*/out/nova-os-bios.img | head -n 1)
qemu-system-x86_64 -drive format=raw,file=$img -display none -serial stdio -netdev user,id=net0 -device virtio-net,netdev=net0 > qemu.log 2>&1 &
QEMU_PID=$!
sleep 5
echo -e "curl http://example.com/\r" > /dev/fd/0
sleep 3
kill -9 $QEMU_PID
cat qemu.log
