#!/bin/bash
cargo build --release -p nova-boot || exit 1
IMAGE=$(ls -t ./target/release/build/nova-boot/*/out/nova-os-bios.img | head -n 1)

echo "[TEST] image path = $IMAGE"
echo "[TEST] image hash = $(md5sum $IMAGE | awk '{print $1}')"
echo "[TEST] QEMU network args = -netdev user,id=net0 -device virtio-net,netdev=net0"
echo "Launching QEMU (GUI)..."

qemu-system-x86_64 \
  -drive format=raw,file="$IMAGE" \
  -netdev user,id=net0 \
  -device virtio-net,netdev=net0
