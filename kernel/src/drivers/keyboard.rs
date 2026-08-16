use pc_keyboard::{
    layouts,
    DecodedKey,
    HandleControl,
    Keyboard,
    ScancodeSet1,
};
use x86_64::instructions::port::Port;

pub struct KeyboardDriver {
    keyboard: Keyboard<layouts::Us104Key, ScancodeSet1>,
    port: Port<u8>,
}

impl KeyboardDriver {
    pub const fn new() -> Self {
        Self {
            keyboard: Keyboard::new(
                ScancodeSet1::new(),
                layouts::Us104Key,
                HandleControl::MapLettersToUnicode,
            ),
            port: Port::new(0x60),
        }
    }

    pub fn handle_interrupt(&mut self) -> Option<char> {
        // SAFETY: We are reading from the standard PS/2 keyboard I/O port 0x60.
        let scancode: u8 = unsafe { self.port.read() };

        if let Ok(Some(key_event)) = self.keyboard.add_byte(scancode) {
            if let Some(key) = self.keyboard.process_keyevent(key_event) {
                if let DecodedKey::Unicode(character) = key {
                    return Some(character);
                }
            }
        }
        None
    }
}
