use crate::graphics::text::TextWriter;

pub const MAX_COMMAND_LENGTH: usize = 128;

pub struct Shell {
    buffer: [u8; MAX_COMMAND_LENGTH],
    len: usize,
}

impl Shell {
    pub const fn new() -> Self {
        Self {
            buffer: [0; MAX_COMMAND_LENGTH],
            len: 0,
        }
    }

    pub fn print_banner(&self, text: &mut TextWriter) {
        text.set_position(60, 60);
        text.set_scale(4);
        text.set_color(80, 160, 255);
        text.write_str("NOVA OS\n\n");

        text.set_scale(2);
        text.set_color(235, 235, 245);
        text.write_str(
            "KERNEL INITIALIZED\n\
             ARCHITECTURE: X86_64\n\
             DISPLAY: 1280X720\n",
        );

        text.set_color(80, 220, 120);
        text.write_str(
            "INTERRUPTS: INITIALIZED\n\
             KEYBOARD: READY\n\n",
        );

        self.print_prompt(text);
    }

    pub fn print_prompt(&self, text: &mut TextWriter) {
        text.set_color(80, 160, 255);
        text.write_str("NOVA> ");
        text.set_color(235, 235, 245);
    }

    pub fn handle_char(&mut self, character: char, text: &mut TextWriter) {
        match character {
            '\n' | '\r' => {
                text.write_char('\n');
                self.execute(text);
                self.len = 0;
            }

            '\u{8}' => {
                if self.len > 0 {
                    self.len -= 1;
                    text.write_char('\u{8}');
                }
            }

            c if c.is_ascii() && !c.is_ascii_control() => {
                if self.len < MAX_COMMAND_LENGTH {
                    self.buffer[self.len] = c as u8;
                    self.len += 1;
                    text.write_char(c);
                }
            }

            _ => {}
        }
    }

    fn execute(&mut self, text: &mut TextWriter) {
        let command_str = match core::str::from_utf8(&self.buffer[..self.len]) {
            Ok(s) => s.trim(),
            Err(_) => {
                self.print_prompt(text);
                return;
            }
        };

        if command_str.is_empty() {
            self.print_prompt(text);
            return;
        }

        let mut parts = command_str.splitn(2, ' ');
        let command = parts.next().unwrap_or("");
        let args = parts.next().unwrap_or("").trim();

        if command.eq_ignore_ascii_case("help") {
            self.cmd_help(text);
        } else if command.eq_ignore_ascii_case("version") {
            self.cmd_version(text);
        } else if command.eq_ignore_ascii_case("clear") {
            self.cmd_clear(text);
            return;
        } else if command.eq_ignore_ascii_case("echo") {
            self.cmd_echo(args, text);
        } else {
            self.cmd_unknown(command, text);
        }

        self.print_prompt(text);
    }

    fn cmd_help(&self, text: &mut TextWriter) {
        text.set_color(80, 160, 255);
        text.write_str("AVAILABLE COMMANDS:\n");
        text.set_color(235, 235, 245);
        text.write_str("  HELP     - DISPLAY AVAILABLE COMMANDS\n");
        text.write_str("  VERSION  - DISPLAY OS VERSION INFORMATION\n");
        text.write_str("  CLEAR    - CLEAR THE CONSOLE SCREEN\n");
        text.write_str("  ECHO     - PRINT TEXT TO CONSOLE\n\n");
    }

    fn cmd_version(&self, text: &mut TextWriter) {
        text.set_color(80, 160, 255);
        text.write_str("NOVA OS\n");
        text.set_color(235, 235, 245);
        text.write_str("VERSION: 0.1.0\n");
        text.write_str("ARCHITECTURE: X86_64\n\n");
    }

    fn cmd_clear(&self, text: &mut TextWriter) {
        text.clear();
        text.set_position(60, 60);
        text.set_scale(2);
        self.print_prompt(text);
    }

    fn cmd_echo(&self, args: &str, text: &mut TextWriter) {
        text.set_color(235, 235, 245);
        if !args.is_empty() {
            text.write_str(args);
        }
        text.write_str("\n\n");
    }

    fn cmd_unknown(&self, command: &str, text: &mut TextWriter) {
        text.set_color(240, 80, 80);
        text.write_str("UNKNOWN COMMAND: ");
        text.write_str(command);
        text.write_str("\n\n");
    }
}
