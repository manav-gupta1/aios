#!/bin/bash
img=$(ls -t ./target/release/build/nova-boot/*/out/nova-os-bios.img | head -n 1)

qemu-system-x86_64 -drive format=raw,file=$img -display none -serial stdio -monitor tcp:127.0.0.1:4444,server,nowait -netdev user,id=net0 -device virtio-net,netdev=net0 > qemu.log 2>&1 &
QEMU_PID=$!

sleep 4

# Type 'curl http://example.com'
echo -e "sendkey c\nsendkey u\nsendkey r\nsendkey l\nsendkey spc\nsendkey h\nsendkey t\nsendkey t\nsendkey p\nsendkey shift-semicolon\nsendkey slash\nsendkey slash\nsendkey e\nsendkey x\nsendkey a\nsendkey m\nsendkey p\nsendkey l\nsendkey e\nsendkey dot\nsendkey c\nsendkey o\nsendkey m\nsendkey slash\nsendkey ret" | nc 127.0.0.1 4444

sleep 6

kill -9 $QEMU_PID
cat qemu.log
