#!/bin/bash
cargo build --release -p nova-boot || exit 1
img=$(ls -t ./target/release/build/nova-boot/*/out/nova-os-bios.img | head -n 1)
killall qemu-system-x86_64 2>/dev/null
qemu-system-x86_64 -drive format=raw,file=$img -display none -serial file:qemu_serial_all5.log -monitor tcp:127.0.0.1:4446,server,nowait -netdev user,id=net0 -device virtio-net,netdev=net0 > qemu_err.log 2>&1 &
QEMU_PID=$!
sleep 4

echo -e "sendkey n\nsendkey s\nsendkey l\nsendkey o\nsendkey o\nsendkey k\nsendkey u\nsendkey p\nsendkey spc\nsendkey e\nsendkey x\nsendkey a\nsendkey m\nsendkey p\nsendkey l\nsendkey e\nsendkey dot\nsendkey c\nsendkey o\nsendkey m\nsendkey ret" | nc 127.0.0.1 4446
sleep 5

echo -e "sendkey c\nsendkey u\nsendkey r\nsendkey l\nsendkey spc\nsendkey h\nsendkey t\nsendkey t\nsendkey p\nsendkey shift-semicolon\nsendkey slash\nsendkey slash\nsendkey e\nsendkey x\nsendkey a\nsendkey m\nsendkey p\nsendkey l\nsendkey e\nsendkey dot\nsendkey c\nsendkey o\nsendkey m\nsendkey slash\nsendkey ret" | nc 127.0.0.1 4446
sleep 8

echo -e "sendkey s\nsendkey l\nsendkey a\nsendkey s\nsendkey h\nsendkey b\nsendkey i\nsendkey n\nsendkey s\nsendkey l\nsendkey a\nsendkey s\nsendkey h\nsendkey h\nsendkey e\nsendkey l\nsendkey l\nsendkey o\nsendkey ret" | nc 127.0.0.1 4446
sleep 2

kill -9 $QEMU_PID
cat qemu_serial_all5.log
