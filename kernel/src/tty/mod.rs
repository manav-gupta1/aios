pub mod line;

use alloc::string::String;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use spin::Mutex;
use crate::tty::line::LineEditor;

pub struct Tty {
    foreground_pgid: usize,
    editor: LineEditor,
    completed_lines: VecDeque<String>,
    readers_to_wake: Vec<usize>,
}

impl Tty {
    pub const fn new() -> Self {
        Self {
            foreground_pgid: 1, // Default shell
            editor: LineEditor::new(),
            completed_lines: VecDeque::new(),
            readers_to_wake: Vec::new(),
        }
    }

    pub fn set_foreground_pgid(&mut self, pgid: usize) {
        self.foreground_pgid = pgid;
    }

    #[allow(dead_code)]
    pub fn foreground_pgid(&self) -> usize {
        self.foreground_pgid
    }

    pub fn tty_input(&mut self, c: char) -> TtyEvent {
        if c == '\x03' {
            return TtyEvent::SignalForeground(crate::process::signal::SIGINT);
        } else if c == '\x1A' {
            return TtyEvent::SignalForeground(crate::process::signal::SIGTSTP);
        }

        if let Some(line) = self.editor.handle_char(c) {
            self.completed_lines.push_back(line);
            let to_wake = core::mem::take(&mut self.readers_to_wake);
            return TtyEvent::WakeReaders(to_wake);
        }

        TtyEvent::None
    }

    pub fn try_read_line_for_kernel(&mut self) -> Option<String> {
        self.completed_lines.pop_front()
    }

    pub fn try_push_line(&mut self, s: &str) {
        self.completed_lines.push_back(String::from(s));
    }

    pub fn read_user_buffer(&mut self, caller_pid: usize, buf: &mut [u8]) -> Result<usize, ()> {
        if let Some(mut line) = self.completed_lines.pop_front() {
            let to_copy = core::cmp::min(line.len(), buf.len());
            buf[..to_copy].copy_from_slice(&line.as_bytes()[..to_copy]);
            if to_copy < line.len() {
                let remainder = line.split_off(to_copy);
                self.completed_lines.push_front(remainder);
            }
            Ok(to_copy)
        } else {
            if !self.readers_to_wake.contains(&caller_pid) {
                self.readers_to_wake.push(caller_pid);
            }
            Err(())
        }
    }

    pub fn write_user_buffer(&mut self, _caller_pid: usize, buf: &[u8]) -> Result<usize, ()> {
        if let Ok(s) = core::str::from_utf8(buf) {
            crate::syscall::USER_OUTPUT_RECEIVED.store(true, core::sync::atomic::Ordering::Release);
            crate::console::write_str(s);
        }
        Ok(buf.len())
    }
}

pub enum TtyEvent {
    None,
    WakeReaders(Vec<usize>),
    SignalForeground(usize),
}

pub static TTY: Mutex<Tty> = Mutex::new(Tty::new());
