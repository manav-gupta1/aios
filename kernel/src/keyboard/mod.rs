use pc_keyboard::{
    layouts,
    DecodedKey,
    HandleControl,
    Keyboard,
    ScancodeSet1,
};

use spin::Mutex;
use x86_64::instructions::port::Port;

static KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
    Mutex::new(Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::Ignore,
    ));

pub fn handle_interrupt() {
    let mut keyboard = KEYBOARD.lock();

    let mut port = Port::new(0x60);

    let scancode: u8 = unsafe { port.read() };

    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => {
                    crate::console::write_char(character);
                }

                DecodedKey::RawKey(_) => {}
            }
        }
    }
}
