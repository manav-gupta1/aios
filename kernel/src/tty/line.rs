use alloc::string::String;

pub struct LineEditor {
    buffer: String,
}

impl LineEditor {
    pub const fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn handle_char(&mut self, c: char) -> Option<String> {
        match c {
            '\n' | '\r' => {
                crate::console::write_char('\n');
                let completed = self.buffer.clone();
                self.buffer.clear();
                Some(completed)
            }
            '\u{8}' => {
                if !self.buffer.is_empty() {
                    self.buffer.pop();
                    crate::console::write_char('\u{8}');
                }
                None
            }
            c if c.is_ascii() && !c.is_ascii_control() => {
                self.buffer.push(c);
                crate::console::write_char(c);
                None
            }
            _ => None,
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}
