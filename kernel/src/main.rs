#![no_std]
#![no_main]

use bootloader_api::{
    entry_point,
    info::{BootInfo, PixelFormat},
};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        let info = framebuffer.info();
        let buffer = framebuffer.buffer_mut();

        for y in 0..info.height {
            for x in 0..info.width {
                let pixel_start =
                    (y * info.stride + x) * info.bytes_per_pixel;

                match info.pixel_format {
                    PixelFormat::Rgb | PixelFormat::Bgr => {
                        buffer[pixel_start] = 0x20;
                        buffer[pixel_start + 1] = 0x20;
                        buffer[pixel_start + 2] = 0x20;
                    }

                    PixelFormat::U8 | PixelFormat::Unknown { .. } => {
                        // Unsupported framebuffer format for now.
                    }

                    _ => {
                        // Handle any future PixelFormat variants.
                    }
                }
            }
        }
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
