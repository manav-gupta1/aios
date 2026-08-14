#![no_std]
#![no_main]

mod graphics;

use bootloader_api::{
    entry_point,
    info::BootInfo,
};

use graphics::framebuffer::FrameBufferWriter;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        let mut writer = FrameBufferWriter::new(framebuffer);

        // NOVA background.
        writer.clear(18, 18, 22);

        // Test rectangle.
        writer.draw_rect(
            100,
            100,
            400,
            200,
            40,
            80,
            180,
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
