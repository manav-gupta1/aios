use crate::fs::{FsError, NodeKind, FILESYSTEM, MAX_PATH_LEN};
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
        let mut cmd_buf = [0u8; MAX_COMMAND_LENGTH];
        cmd_buf[..self.len].copy_from_slice(&self.buffer[..self.len]);

        let raw_input = match core::str::from_utf8(&cmd_buf[..self.len]) {
            Ok(s) => s,
            Err(_) => {
                self.print_prompt(text);
                return;
            }
        };

        let (command, args) = parse_command(raw_input);

        if command.is_empty() {
            self.print_prompt(text);
            return;
        }

        if command.eq_ignore_ascii_case("help") {
            self.cmd_help(text);
        } else if command.eq_ignore_ascii_case("version") {
            self.cmd_version(text);
        } else if command.eq_ignore_ascii_case("clear") {
            self.cmd_clear(text);
            return;
        } else if command.eq_ignore_ascii_case("echo") {
            self.cmd_echo(args, text);
        } else if command.eq_ignore_ascii_case("pwd") {
            self.cmd_pwd(text);
        } else if command.eq_ignore_ascii_case("ls") {
            self.cmd_ls(args, text);
        } else if command.eq_ignore_ascii_case("cd") {
            self.cmd_cd(args, text);
        } else if command.eq_ignore_ascii_case("mkdir") {
            self.cmd_mkdir(args, text);
        } else if command.eq_ignore_ascii_case("touch") {
            self.cmd_touch(args, text);
        } else if command.eq_ignore_ascii_case("cat") {
            self.cmd_cat(args, text);
        } else if command.eq_ignore_ascii_case("write") {
            self.cmd_write(args, text);
        } else {
            self.cmd_unknown(command, text);
        }

        self.print_prompt(text);
    }

    fn cmd_help(&self, text: &mut TextWriter) {
        text.set_color(80, 160, 255);
        text.write_str("Available commands:\n");
        text.set_color(235, 235, 245);
        text.write_str("  help     - Display available commands\n");
        text.write_str("  version  - Display OS version information\n");
        text.write_str("  clear    - Clear the console screen\n");
        text.write_str("  echo     - Print text to console\n");
        text.write_str("  pwd      - Print working directory\n");
        text.write_str("  ls       - List directory contents\n");
        text.write_str("  cd       - Change working directory\n");
        text.write_str("  mkdir    - Create a new directory\n");
        text.write_str("  touch    - Create an empty file\n");
        text.write_str("  cat      - Display file contents\n");
        text.write_str("  write    - Write text to a file\n\n");
    }

    fn cmd_version(&self, text: &mut TextWriter) {
        text.set_color(80, 160, 255);
        text.write_str("NOVA OS\n");
        text.set_color(235, 235, 245);
        text.write_str("Version: 0.1.0\n");
        text.write_str("Architecture: x86_64\n\n");
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

    fn cmd_pwd(&self, text: &mut TextWriter) {
        let mut path_buf = [0u8; MAX_PATH_LEN];
        let len = FILESYSTEM.lock().get_pwd(&mut path_buf);
        let pwd_str = core::str::from_utf8(&path_buf[..len]).unwrap_or("/");
        text.set_color(235, 235, 245);
        text.write_str(pwd_str);
        text.write_str("\n\n");
    }

    fn cmd_ls(&self, args: &str, text: &mut TextWriter) {
        let target_path = if args.is_empty() { "." } else { args };
        let mut count = 0;

        let res = FILESYSTEM.lock().list_dir(target_path, |name, kind| {
            count += 1;
            match kind {
                NodeKind::Directory => {
                    text.set_color(80, 160, 255);
                    text.write_str(name);
                    text.write_str("/\n");
                }
                NodeKind::File => {
                    text.set_color(235, 235, 245);
                    text.write_str(name);
                    text.write_str("\n");
                }
            }
        });

        match res {
            Ok(()) => {
                if count > 0 {
                    text.write_str("\n");
                } else {
                    text.write_str("\n");
                }
            }
            Err(e) => {
                self.print_error(e, target_path, text);
            }
        }
    }

    fn cmd_cd(&mut self, args: &str, text: &mut TextWriter) {
        let target_path = if args.is_empty() { "/" } else { args };
        if let Err(e) = FILESYSTEM.lock().change_dir(target_path) {
            self.print_error(e, target_path, text);
        }
    }

    fn cmd_mkdir(&mut self, args: &str, text: &mut TextWriter) {
        if args.is_empty() {
            text.set_color(240, 80, 80);
            text.write_str("Usage: mkdir <name>\n\n");
            return;
        }

        if let Err(e) = FILESYSTEM.lock().create_dir(args) {
            self.print_error(e, args, text);
        }
    }

    fn cmd_touch(&mut self, args: &str, text: &mut TextWriter) {
        if args.is_empty() {
            text.set_color(240, 80, 80);
            text.write_str("Usage: touch <name>\n\n");
            return;
        }

        if let Err(e) = FILESYSTEM.lock().create_file(args) {
            self.print_error(e, args, text);
        }
    }

    fn cmd_cat(&self, args: &str, text: &mut TextWriter) {
        if args.is_empty() {
            text.set_color(240, 80, 80);
            text.write_str("Usage: cat <name>\n\n");
            return;
        }

        let fs = FILESYSTEM.lock();
        match fs.read_file(args) {
            Ok(content) => {
                text.set_color(235, 235, 245);
                if !content.is_empty() {
                    text.write_str(content);
                    if !content.ends_with('\n') {
                        text.write_str("\n");
                    }
                }
                text.write_str("\n");
            }
            Err(e) => {
                drop(fs);
                self.print_error(e, args, text);
            }
        }
    }

    fn cmd_write(&mut self, args: &str, text: &mut TextWriter) {
        let (filename, content) = parse_command(args);
        if filename.is_empty() {
            text.set_color(240, 80, 80);
            text.write_str("Usage: write <name> <text>\n\n");
            return;
        }

        if let Err(e) = FILESYSTEM.lock().write_file(filename, content.as_bytes()) {
            self.print_error(e, filename, text);
        }
    }

    fn cmd_unknown(&self, command: &str, text: &mut TextWriter) {
        text.set_color(240, 80, 80);
        text.write_str("Unknown command: ");
        text.write_str(command);
        text.write_str("\n\n");
    }

    fn print_error(&self, error: FsError, path: &str, text: &mut TextWriter) {
        text.set_color(240, 80, 80);
        match error {
            FsError::NotFound => {
                text.write_str("Not found: ");
                text.write_str(path);
            }
            FsError::AlreadyExists => {
                text.write_str("Already exists: ");
                text.write_str(path);
            }
            FsError::NotADirectory => {
                text.write_str("Not a directory: ");
                text.write_str(path);
            }
            FsError::IsADirectory => {
                text.write_str("Is a directory: ");
                text.write_str(path);
            }
            FsError::StorageFull => {
                text.write_str("Storage full");
            }
            FsError::NameTooLong => {
                text.write_str("Name too long: ");
                text.write_str(path);
            }
            FsError::FileTooLarge => {
                text.write_str("File too large");
            }
            FsError::InvalidPath => {
                text.write_str("Invalid path: ");
                text.write_str(path);
            }
        }
        text.write_str("\n\n");
    }
}

fn parse_command(input: &str) -> (&str, &str) {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return ("", "");
    }

    if let Some(space_idx) = trimmed.find(' ') {
        let command = &trimmed[..space_idx];
        let args = trimmed[space_idx..].trim_start();
        (command, args)
    } else {
        (trimmed.trim_end(), "")
    }
}
