# AIOS / NOVA

A hobby operating-system project written in Rust for **x86_64**. The kernel is `no_std` and `no_main`, boots through the `bootloader_api`, and provides a command shell with kernel and user-space subsystems.

This is an experimental OS, not a general-purpose or production-secure operating system.

## Implemented areas

The kernel source is organized into modules for:

- **Boot and memory:** boot information, physical-frame allocation, paging, heap initialization, and memory mapping.
- **Scheduling and processes:** task scheduling, user tasks, process table, process lifecycle, and context switching.
- **System interfaces:** interrupt handling, GDT/TSS, syscalls, and user-space support.
- **Filesystem:** a kernel filesystem with path resolution, directories, files, and storage synchronization.
- **IPC and descriptors:** pipes, file descriptors, and process communication.
- **Networking:** VirtIO network support and a `smoltcp`-based stack, with user-space networking test programs.
- **Console:** framebuffer text output, TTY, and an interactive shell.

The workspace also contains small user programs used to exercise features such as process execution, pipes, memory mapping, signals, UDP, HTTP, and ping.

## Architecture at a glance

```text
Bootloader
    |
    v
x86_64 no_std Kernel
    |
    +-- Memory / Paging / Heap
    +-- GDT / IDT / Timer Interrupts
    +-- Scheduler / Process Table
    +-- Syscalls / User Space
    +-- Filesystem / Storage Driver
    +-- IPC / Pipes
    +-- VirtIO Network / smoltcp
    |
    v
Kernel Shell + User Programs
```

## Toolchain

The repository uses:

- Rust nightly
- Rust edition 2024
- Target: `x86_64-unknown-none`
- `bootloader_api`
- `x86_64`
- `spin`
- `smoltcp`

The project uses unstable Rust features and Cargo build-dependencies with binary artifacts, so a stable Rust toolchain is not sufficient.

## Build and run

The repository's build/run scripts and QEMU configuration are the source of truth for the exact commands. Start by installing the pinned nightly toolchain and target:

```bash
rustup toolchain install nightly
rustup target add x86_64-unknown-none --toolchain nightly
```

Then inspect the repository scripts for the supported build and QEMU invocation. The workspace has a kernel, bootloader, and multiple user-program crates.

## Current limitations

- This is an experimental learning project. Hardware support and behavior are currently scoped to the development environment.
- Networking initializes a static QEMU user-networking configuration; it is not a general DHCP/network configuration service.
- Some interfaces and subsystems are simplified and should not be assumed to provide full POSIX compatibility.
- Kernel code uses unsafe operations where required by low-level memory and hardware interfaces. Those paths need dedicated testing and review.
- A successful boot or demo is not equivalent to a security or correctness proof.

## Suggested next engineering milestones

- [ ] Add repeatable QEMU smoke tests to CI.
- [ ] Document supported boot and emulator configurations.
- [ ] Add targeted tests for scheduler transitions and process lifecycle.
- [ ] Add memory-management invariants and stress tests.
- [ ] Document syscall and user/kernel boundary contracts.
- [ ] Audit unsafe blocks and interrupt/lock interactions.

## License

No license has been added yet. All rights reserved unless and until a license is specified.
