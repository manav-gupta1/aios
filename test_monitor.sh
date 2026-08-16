#!/bin/bash
img=$(ls -t ./target/release/build/nova-boot/*/out/nova-os-bios.img | head -n 1)

qemu-system-x86_64 -drive format=raw,file=$img -display none -serial stdio -monitor tcp:127.0.0.1:4444,server,nowait &
QEMU_PID=$!

sleep 2

# Type 'run /bin/hello &'
echo -e "sendkey r\nsendkey u\nsendkey n\nsendkey spc\nsendkey slash\nsendkey b\nsendkey i\nsendkey n\nsendkey slash\nsendkey h\nsendkey e\nsendkey l\nsendkey l\nsendkey o\nsendkey spc\nsendkey shift-7\nsendkey ret" | nc 127.0.0.1 4444

sleep 2

# Now try to type another command 'ls'
echo -e "sendkey l\nsendkey s\nsendkey ret" | nc 127.0.0.1 4444

sleep 2

echo "screendump /tmp/qemu_screen_fixed.ppm" | nc 127.0.0.1 4444

sleep 1

kill $QEMU_PID
