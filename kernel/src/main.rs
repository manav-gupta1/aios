#![no_std]
#![no_main]

mod graphics;

use bootloader_api::{
    entry_point,
    info::BootInfo,
};

use graphics::framebuffer::FrameBufferWriter;
use graphics::text::TextWriter;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        let mut framebuffer = FrameBufferWriter::new(framebuffer);

        // NOVA background.
        framebuffer.clear(18, 18, 22);

        // Give the text renderer ownership of the framebuffer.
        let mut text = TextWriter::new(framebuffer);

        // NOVA title.
        text.set_position(60, 60);
        text.set_scale(4);
        text.set_color(80, 160, 255);
        text.write_str("NOVA OS");

        // System information.
        text.set_position(60, 130);
        text.set_scale(2);
        text.set_color(235, 235, 245);

        text.write_str(
            "KERNEL INITIALIZED\n\
             ARCHITECTURE: X86_64\n\
             DISPLAY: 1280X720\n\
             STATUS: RUNNING\n\n\
             NOVA> _",
        );
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
