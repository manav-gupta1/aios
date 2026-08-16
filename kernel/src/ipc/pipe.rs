use alloc::vec::Vec;

pub const PIPE_CAPACITY: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeReadError {
    WouldBlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeWriteError {
    WouldBlock,
    BrokenPipe,
}

pub struct Pipe {
    buffer: [u8; PIPE_CAPACITY],
    read_pos: usize,
    write_pos: usize,
    count: usize,
    readers_count: usize,
    writers_count: usize,
    pub blocked_readers: Vec<usize>,
    pub blocked_writers: Vec<usize>,
}

impl Pipe {
    pub fn new() -> Self {
        Pipe {
            buffer: [0u8; PIPE_CAPACITY],
            read_pos: 0,
            write_pos: 0,
            count: 0,
            readers_count: 1,
            writers_count: 1,
            blocked_readers: Vec::new(),
            blocked_writers: Vec::new(),
        }
    }

    pub fn read(&mut self, caller_pid: usize, buf: &mut [u8]) -> Result<usize, PipeReadError> {
        if buf.is_empty() {
            return Ok(0);
        }

        if self.count > 0 {
            let to_read = core::cmp::min(buf.len(), self.count);
            for i in 0..to_read {
                buf[i] = self.buffer[self.read_pos];
                self.read_pos = (self.read_pos + 1) % PIPE_CAPACITY;
            }
            self.count -= to_read;

            // Remove caller from blocked readers if present
            self.blocked_readers.retain(|&pid| pid != caller_pid);

            Ok(to_read)
        } else if self.writers_count == 0 {
            // All writers closed and no data remaining -> EOF
            self.blocked_readers.retain(|&pid| pid != caller_pid);
            Ok(0)
        } else {
            // Buffer empty and writers still exist -> Reader must block
            if !self.blocked_readers.contains(&caller_pid) {
                self.blocked_readers.push(caller_pid);
            }
            Err(PipeReadError::WouldBlock)
        }
    }

    pub fn write(&mut self, caller_pid: usize, buf: &[u8]) -> Result<usize, PipeWriteError> {
        if buf.is_empty() {
            return Ok(0);
        }

        if self.readers_count == 0 {
            // No readers open -> Broken pipe
            self.blocked_writers.retain(|&pid| pid != caller_pid);
            return Err(PipeWriteError::BrokenPipe);
        }

        let space_available = PIPE_CAPACITY - self.count;
        if space_available > 0 {
            let to_write = core::cmp::min(buf.len(), space_available);
            for i in 0..to_write {
                self.buffer[self.write_pos] = buf[i];
                self.write_pos = (self.write_pos + 1) % PIPE_CAPACITY;
            }
            self.count += to_write;

            // Remove caller from blocked writers if present
            self.blocked_writers.retain(|&pid| pid != caller_pid);

            Ok(to_write)
        } else {
            // Buffer full and readers exist -> Writer must block
            if !self.blocked_writers.contains(&caller_pid) {
                self.blocked_writers.push(caller_pid);
            }
            Err(PipeWriteError::WouldBlock)
        }
    }

    pub fn close_read(&mut self) -> (bool, Vec<usize>) {
        if self.readers_count > 0 {
            self.readers_count -= 1;
        }

        // If readers reached zero, wake all blocked writers so they receive BrokenPipe
        let to_wake = if self.readers_count == 0 {
            let list = self.blocked_writers.clone();
            self.blocked_writers.clear();
            list
        } else {
            Vec::new()
        };

        let is_empty = self.readers_count == 0 && self.writers_count == 0;
        (is_empty, to_wake)
    }

    pub fn close_write(&mut self) -> (bool, Vec<usize>) {
        if self.writers_count > 0 {
            self.writers_count -= 1;
        }

        // If writers reached zero, wake all blocked readers so they receive EOF
        let to_wake = if self.writers_count == 0 {
            let list = self.blocked_readers.clone();
            self.blocked_readers.clear();
            list
        } else {
            Vec::new()
        };

        let is_empty = self.readers_count == 0 && self.writers_count == 0;
        (is_empty, to_wake)
    }

    #[allow(dead_code)]
    pub fn add_reader(&mut self) {
        self.readers_count += 1;
    }

    #[allow(dead_code)]
    pub fn add_writer(&mut self) {
        self.writers_count += 1;
    }

    #[allow(dead_code)]
    pub fn readers_count(&self) -> usize {
        self.readers_count
    }

    #[allow(dead_code)]
    pub fn writers_count(&self) -> usize {
        self.writers_count
    }

    pub fn take_readers_to_wake(&mut self) -> Vec<usize> {
        let list = self.blocked_readers.clone();
        self.blocked_readers.clear();
        list
    }

    pub fn take_writers_to_wake(&mut self) -> Vec<usize> {
        let list = self.blocked_writers.clone();
        self.blocked_writers.clear();
        list
    }
}
