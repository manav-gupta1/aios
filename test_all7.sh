#!/bin/bash
cargo build --release -p nova-boot || exit 1
img=$(ls -t ./target/release/build/nova-boot/*/out/nova-os-bios.img | head -n 1)
killall qemu-system-x86_64 2>/dev/null
qemu-system-x86_64 -drive format=raw,file=$img -display none -serial file:qemu_serial_all7.log -monitor tcp:127.0.0.1:4446,server,nowait -netdev user,id=net0 -device virtio-net,netdev=net0 > qemu_err.log 2>&1 &
QEMU_PID=$!
sleep 15

kill -9 $QEMU_PID
cat qemu_serial_all7.log
