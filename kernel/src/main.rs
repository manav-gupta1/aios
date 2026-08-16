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
mod keyboard;
mod memory;
mod process;
mod shell;
mod syscall;
mod task;
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

    // Initialize and mount persistent filesystem.
    fs::init();

    // Verify IPC subsystem
    let _ = ipc::run_ipc_tests();

    let mut shell = Shell::new();
    shell.print_banner(&mut text);

    loop {
        let mut had_work = false;

        // Output any pending characters from syscalls / user space
        while let Some(c) = console::read_out_char() {
            text.write_char(c);
            had_work = true;
        }

        // Move decoded keyboard characters from the interrupt-side
        // queue into the shell.
        while let Some(character) = console::read_char() {
            shell.handle_char(character, &mut text);
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
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
