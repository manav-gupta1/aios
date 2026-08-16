use spin::Mutex;

const BUFFER_SIZE: usize = 512;

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
static OUT_BUFFER: Mutex<CharBuffer> = Mutex::new(CharBuffer::new());

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

pub fn write_str(s: &str) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut buf = OUT_BUFFER.lock();
        for c in s.chars() {
            buf.push(c);
        }
    });
}

pub fn read_out_char() -> Option<char> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        OUT_BUFFER.lock().pop()
    })
}
