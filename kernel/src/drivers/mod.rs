pub mod keyboard;
pub mod pci;
pub mod timer;

use spin::Mutex;
use keyboard::KeyboardDriver;

pub static KEYBOARD_DRIVER: Mutex<KeyboardDriver> = Mutex::new(KeyboardDriver::new());
