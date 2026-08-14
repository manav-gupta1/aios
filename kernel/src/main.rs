#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

mod graphics;
mod interrupts;

use bootloader_api::{
    entry_point,
    info::BootInfo,
};

use graphics::framebuffer::FrameBufferWriter;
use graphics::text::TextWriter;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        let framebuffer = FrameBufferWriter::new(framebuffer);

        let mut text = TextWriter::new(framebuffer);

        // Clear the bootloader output.
        text.set_background(18, 18, 22);
        text.set_position(60, 60);
        text.set_scale(4);
        text.set_color(80, 160, 255);
        text.write_str("NOVA OS");

        text.set_position(60, 130);
        text.set_scale(2);
        text.set_color(235, 235, 245);

        text.write_str(
            "KERNEL INITIALIZED\n\
             ARCHITECTURE: X86_64\n\
             DISPLAY: 1280X720\n\
             INTERRUPTS: INITIALIZING...\n\
             IDT: LOADING...\n",
        );

        // Initialize the Interrupt Descriptor Table.
        interrupts::init();

        // Trigger CPU breakpoint exception #3.
        x86_64::instructions::interrupts::int3();

        // If execution reaches this line, the IDT handler worked.
        text.set_color(80, 220, 120);
        text.write_str("INTERRUPT TEST: PASSED\n");

        text.set_color(235, 235, 245);
        text.write_str("NOVA KERNEL IS ALIVE\n");
    }

    loop {
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
