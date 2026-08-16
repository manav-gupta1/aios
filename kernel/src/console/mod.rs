use spin::Mutex;

const BUFFER_SIZE: usize = 256;

struct CharBuffer {
    buffer: [char; BUFFER_SIZE],
    read: usize,
    write: usize,
}

impl CharBuffer {
    const fn new() -> Self {
        Self {
            buffer: ['\0'; BUFFER_SIZE],
            read: 0,
            write: 0,
        }
    }

    fn push(&mut self, character: char) {
        let next = (self.write + 1) % BUFFER_SIZE;

        // Drop the character if the buffer is full.
        if next == self.read {
            return;
        }

        self.buffer[self.write] = character;
        self.write = next;
    }

    fn pop(&mut self) -> Option<char> {
        if self.read == self.write {
            return None;
        }

        let character = self.buffer[self.read];
        self.read = (self.read + 1) % BUFFER_SIZE;

        Some(character)
    }
}

static CHAR_BUFFER: Mutex<CharBuffer> = Mutex::new(CharBuffer::new());

pub fn write_char(character: char) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        CHAR_BUFFER.lock().push(character);
    });
}

pub fn read_char() -> Option<char> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        CHAR_BUFFER.lock().pop()
    })
}

