pub mod keyboard;
pub mod timer;
pub mod pci;
pub mod storage;
pub mod virtio;

use spin::Mutex;
pub static KEYBOARD_DRIVER: Mutex<keyboard::KeyboardDriver> = Mutex::new(keyboard::KeyboardDriver::new());
