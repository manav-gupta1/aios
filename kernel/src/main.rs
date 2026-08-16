#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

mod console;
mod fs;
mod graphics;
mod interrupts;
mod keyboard;
mod shell;

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
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    let framebuffer = boot_info
        .framebuffer
        .as_mut()
        .expect("NOVA requires a framebuffer");

    let mut framebuffer = FrameBufferWriter::new(framebuffer);
    framebuffer.clear(18, 18, 22);

    let mut text = TextWriter::new(framebuffer);

    // Background.
    text.set_background(18, 18, 22);

    // Initialize IDT, PIC and keyboard interrupt handling.
    interrupts::init();

    // Trigger CPU breakpoint exception #3 to verify IDT exception handling.
    x86_64::instructions::interrupts::int3();

    let mut shell = Shell::new();
    shell.print_banner(&mut text);

    loop {
        // Move decoded keyboard characters from the interrupt-side
        // queue into the shell.
        while let Some(character) = console::read_char() {
            shell.handle_char(character, &mut text);
        }

        core::hint::spin_loop();
    }
}

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
