use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};

pub struct FrameBufferWriter<'a> {
    framebuffer: &'a mut FrameBuffer,
}

impl<'a> FrameBufferWriter<'a> {
    pub fn new(framebuffer: &'a mut FrameBuffer) -> Self {
        Self { framebuffer }
    }

    pub fn width(&self) -> usize {
        self.framebuffer.info().width
    }

    pub fn height(&self) -> usize {
        self.framebuffer.info().height
    }

    pub fn clear(&mut self, r: u8, g: u8, b: u8) {
        let info = self.framebuffer.info();
        let buffer = self.framebuffer.buffer_mut();

        for y in 0..info.height {
            for x in 0..info.width {
                write_pixel(buffer, &info, x, y, r, g, b);
            }
        }
    }

    pub fn draw_pixel(
        &mut self,
        x: usize,
        y: usize,
        r: u8,
        g: u8,
        b: u8,
    ) {
        let info = self.framebuffer.info();

        if x >= info.width || y >= info.height {
            return;
        }

        let buffer = self.framebuffer.buffer_mut();

        write_pixel(buffer, &info, x, y, r, g, b);
    }

    pub fn draw_rect(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        r: u8,
        g: u8,
        b: u8,
    ) {
        let max_x = x.saturating_add(width).min(self.width());
        let max_y = y.saturating_add(height).min(self.height());

        for py in y..max_y {
            for px in x..max_x {
                self.draw_pixel(px, py, r, g, b);
            }
        }
    }
}

fn write_pixel(
    buffer: &mut [u8],
    info: &FrameBufferInfo,
    x: usize,
    y: usize,
    r: u8,
    g: u8,
    b: u8,
) {
    let pixel_start = (y * info.stride + x) * info.bytes_per_pixel;

    if pixel_start + info.bytes_per_pixel > buffer.len() {
        return;
    }

    match info.pixel_format {
        PixelFormat::Rgb => {
            buffer[pixel_start] = r;
            buffer[pixel_start + 1] = g;
            buffer[pixel_start + 2] = b;
        }

        PixelFormat::Bgr => {
            buffer[pixel_start] = b;
            buffer[pixel_start + 1] = g;
            buffer[pixel_start + 2] = r;
        }

        PixelFormat::U8 => {
            buffer[pixel_start] = r;
        }

        PixelFormat::Unknown { .. } => {
            // Unsupported framebuffer format.
        }

        _ => {
            // Future PixelFormat variants.
        }
    }
}
