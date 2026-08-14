use super::framebuffer::FrameBufferWriter;

const FONT_WIDTH: usize = 5;
const FONT_HEIGHT: usize = 7;

pub struct TextWriter<'a> {
    framebuffer: FrameBufferWriter<'a>,
    x: usize,
    y: usize,
    scale: usize,
    foreground: (u8, u8, u8),
    background: (u8, u8, u8),
}

impl<'a> TextWriter<'a> {
    pub fn new(framebuffer: FrameBufferWriter<'a>) -> Self {
        Self {
            framebuffer,
            x: 0,
            y: 0,
            scale: 3,
            foreground: (235, 235, 245),
            background: (18, 18, 22),
        }
    }

    pub fn set_position(&mut self, x: usize, y: usize) {
        self.x = x;
        self.y = y;
    }

    pub fn set_scale(&mut self, scale: usize) {
        self.scale = scale.max(1);
    }

    pub fn set_color(&mut self, r: u8, g: u8, b: u8) {
        self.foreground = (r, g, b);
    }

    pub fn set_background(&mut self, r: u8, g: u8, b: u8) {
        self.background = (r, g, b);
    }

    pub fn write_str(&mut self, text: &str) {
        for byte in text.bytes() {
            match byte {
                b'\n' => self.new_line(),

                b'\r' => {
                    self.x = 0;
                }

                b' ' => {
                    self.x += self.char_width();
                }

                byte => {
                    self.draw_char(byte as char);
                    self.x += self.char_width();
                }
            }
        }
    }

    fn new_line(&mut self) {
        self.x = 0;
        self.y += self.char_height();
    }

    fn char_width(&self) -> usize {
        (FONT_WIDTH + 1) * self.scale
    }

    fn char_height(&self) -> usize {
        (FONT_HEIGHT + 2) * self.scale
    }

    fn draw_char(&mut self, character: char) {
        let glyph = glyph(character);

        for row in 0..FONT_HEIGHT {
            for column in 0..FONT_WIDTH {
                let bit = glyph[row] & (1 << (FONT_WIDTH - 1 - column));

                let px = self.x + column * self.scale;
                let py = self.y + row * self.scale;

                if bit != 0 {
                    self.framebuffer.draw_rect(
                        px,
                        py,
                        self.scale,
                        self.scale,
                        self.foreground.0,
                        self.foreground.1,
                        self.foreground.2,
                    );
                } else {
                    self.framebuffer.draw_rect(
                        px,
                        py,
                        self.scale,
                        self.scale,
                        self.background.0,
                        self.background.1,
                        self.background.2,
                    );
                }
            }
        }
    }
}

fn glyph(character: char) -> [u8; FONT_HEIGHT] {
    match character.to_ascii_uppercase() {
        'A' => [
            0b01110,
            0b10001,
            0b10001,
            0b11111,
            0b10001,
            0b10001,
            0b10001,
        ],

        'B' => [
            0b11110,
            0b10001,
            0b10001,
            0b11110,
            0b10001,
            0b10001,
            0b11110,
        ],

        'C' => [
            0b01111,
            0b10000,
            0b10000,
            0b10000,
            0b10000,
            0b10000,
            0b01111,
        ],

        'D' => [
            0b11110,
            0b10001,
            0b10001,
            0b10001,
            0b10001,
            0b10001,
            0b11110,
        ],

        'E' => [
            0b11111,
            0b10000,
            0b10000,
            0b11110,
            0b10000,
            0b10000,
            0b11111,
        ],

        'F' => [
            0b11111,
            0b10000,
            0b10000,
            0b11110,
            0b10000,
            0b10000,
            0b10000,
        ],

        'G' => [
            0b01111,
            0b10000,
            0b10000,
            0b10111,
            0b10001,
            0b10001,
            0b01111,
        ],

        'H' => [
            0b10001,
            0b10001,
            0b10001,
            0b11111,
            0b10001,
            0b10001,
            0b10001,
        ],

        'I' => [
            0b11111,
            0b00100,
            0b00100,
            0b00100,
            0b00100,
            0b00100,
            0b11111,
        ],

        'J' => [
            0b00111,
            0b00010,
            0b00010,
            0b00010,
            0b00010,
            0b10010,
            0b01100,
        ],

        'K' => [
            0b10001,
            0b10010,
            0b10100,
            0b11000,
            0b10100,
            0b10010,
            0b10001,
        ],

        'L' => [
            0b10000,
            0b10000,
            0b10000,
            0b10000,
            0b10000,
            0b10000,
            0b11111,
        ],

        'M' => [
            0b10001,
            0b11011,
            0b10101,
            0b10101,
            0b10001,
            0b10001,
            0b10001,
        ],

        'N' => [
            0b10001,
            0b11001,
            0b10101,
            0b10011,
            0b10001,
            0b10001,
            0b10001,
        ],

        'O' => [
            0b01110,
            0b10001,
            0b10001,
            0b10001,
            0b10001,
            0b10001,
            0b01110,
        ],

        'P' => [
            0b11110,
            0b10001,
            0b10001,
            0b11110,
            0b10000,
            0b10000,
            0b10000,
        ],

        'Q' => [
            0b01110,
            0b10001,
            0b10001,
            0b10001,
            0b10101,
            0b10010,
            0b01101,
        ],

        'R' => [
            0b11110,
            0b10001,
            0b10001,
            0b11110,
            0b10100,
            0b10010,
            0b10001,
        ],

        'S' => [
            0b01111,
            0b10000,
            0b10000,
            0b01110,
            0b00001,
            0b00001,
            0b11110,
        ],

        'T' => [
            0b11111,
            0b00100,
            0b00100,
            0b00100,
            0b00100,
            0b00100,
            0b00100,
        ],

        'U' => [
            0b10001,
            0b10001,
            0b10001,
            0b10001,
            0b10001,
            0b10001,
            0b01110,
        ],

        'V' => [
            0b10001,
            0b10001,
            0b10001,
            0b10001,
            0b10001,
            0b01010,
            0b00100,
        ],

        'W' => [
            0b10001,
            0b10001,
            0b10001,
            0b10101,
            0b10101,
            0b11011,
            0b10001,
        ],

        'X' => [
            0b10001,
            0b10001,
            0b01010,
            0b00100,
            0b01010,
            0b10001,
            0b10001,
        ],

        'Y' => [
            0b10001,
            0b10001,
            0b01010,
            0b00100,
            0b00100,
            0b00100,
            0b00100,
        ],

        'Z' => [
            0b11111,
            0b00001,
            0b00010,
            0b00100,
            0b01000,
            0b10000,
            0b11111,
        ],

        '0' => [
            0b01110,
            0b10001,
            0b10011,
            0b10101,
            0b11001,
            0b10001,
            0b01110,
        ],

        '1' => [
            0b00100,
            0b01100,
            0b00100,
            0b00100,
            0b00100,
            0b00100,
            0b01110,
        ],

        '2' => [
            0b01110,
            0b10001,
            0b00001,
            0b00010,
            0b00100,
            0b01000,
            0b11111,
        ],

        '3' => [
            0b11110,
            0b00001,
            0b00001,
            0b01110,
            0b00001,
            0b00001,
            0b11110,
        ],

        '4' => [
            0b00010,
            0b00110,
            0b01010,
            0b10010,
            0b11111,
            0b00010,
            0b00010,
        ],

        '5' => [
            0b11111,
            0b10000,
            0b10000,
            0b11110,
            0b00001,
            0b00001,
            0b11110,
        ],

        '6' => [
            0b01110,
            0b10000,
            0b10000,
            0b11110,
            0b10001,
            0b10001,
            0b01110,
        ],

        '7' => [
            0b11111,
            0b00001,
            0b00010,
            0b00100,
            0b01000,
            0b01000,
            0b01000,
        ],

        '8' => [
            0b01110,
            0b10001,
            0b10001,
            0b01110,
            0b10001,
            0b10001,
            0b01110,
        ],

        '9' => [
            0b01110,
            0b10001,
            0b10001,
            0b01111,
            0b00001,
            0b00001,
            0b01110,
        ],

        '-' => [
            0b00000,
            0b00000,
            0b00000,
            0b11111,
            0b00000,
            0b00000,
            0b00000,
        ],

        ':' => [
            0b00000,
            0b00100,
            0b00100,
            0b00000,
            0b00100,
            0b00100,
            0b00000,
        ],

        '.' => [
            0b00000,
            0b00000,
            0b00000,
            0b00000,
            0b00000,
            0b00110,
            0b00110,
        ],

        '>' => [
            0b10000,
            0b01000,
            0b00100,
            0b00010,
            0b00100,
            0b01000,
            0b10000,
        ],

        '_' => [
            0b00000,
            0b00000,
            0b00000,
            0b00000,
            0b00000,
            0b00000,
            0b11111,
        ],

        _ => [
            0b00000,
            0b00000,
            0b00000,
            0b00100,
            0b00000,
            0b00000,
            0b00000,
        ],
    }
}
