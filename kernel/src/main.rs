#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

mod console;
mod elf;
mod fs;
mod gdt;
mod graphics;
mod interrupts;
mod ipc;
pub mod drivers;
mod memory;
mod process;
mod shell;
mod syscall;
mod task;
pub mod net;
pub mod tty;
mod userspace;

use bootloader_api::{
    entry_point,
    info::BootInfo,
};

use graphics::framebuffer::FrameBufferWriter;
use graphics::text::TextWriter;
use shell::Shell;

pub static BOOTLOADER_CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.kernel_stack_size = 100 * 1024;
    config.mappings.physical_memory = Some(bootloader_api::config::Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // Initialize physical memory, paging, and kernel heap allocator.
    memory::init(&boot_info.memory_regions, boot_info.physical_memory_offset.into_option());

    let framebuffer = boot_info
        .framebuffer
        .as_mut()
        .expect("NOVA requires a framebuffer");

    let mut framebuffer = FrameBufferWriter::new(framebuffer);
    framebuffer.clear(18, 18, 22);

    let mut text = TextWriter::new(framebuffer);

    // Background.
    text.set_background(18, 18, 22);

    // Initialize GDT and TSS (Ring 0/3 segments, privilege stack table).
    gdt::init();

    // Initialize task management and preemptive scheduler (registers main thread as Task 1).
    task::init();

    // Initialize process table (registers init process as PID 1).
    process::init();

    // Initialize IDT, PIC, PIT timer (100 Hz), syscalls, and enable interrupts.
    interrupts::init();

    crate::drivers::storage::init(&mut text);
    crate::drivers::storage::serial_print("[MAIN] storage init done\n");

    crate::drivers::network::init(&mut text);
    crate::net::init();
    crate::drivers::storage::serial_print("[MAIN] network init done\n");

    // Initialize and mount persistent filesystem.
    fs::init();
    crate::drivers::storage::serial_print("[MAIN] fs init done\n");

    // Verify IPC subsystem
    let _ = ipc::run_ipc_tests();
    crate::drivers::storage::serial_print("[MAIN] IPC tests done\n");

    let mut shell = Shell::new();
    shell.print_banner(&mut text);
    crate::drivers::storage::serial_print("[MAIN] banner printed\n");

    // Drop into interactive shell

    loop {
        let mut had_work = false;

        // Output any pending characters from syscalls / user space
        while let Some(c) = console::read_out_char() {
            text.write_char(c);
            had_work = true;
        }

        while let Some(line) = x86_64::instructions::interrupts::without_interrupts(|| crate::tty::TTY.lock().try_read_line_for_kernel()) {
            shell.execute_line(&line, &mut text);
            had_work = true;
        }

        if !had_work {
            x86_64::instructions::interrupts::enable();
            x86_64::instructions::hlt();
        }
    }
}

use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let msg = alloc::format!("PANIC: {}\n", info);
    crate::drivers::storage::serial_print(&msg);
    loop {
        core::hint::spin_loop();
    }
}
