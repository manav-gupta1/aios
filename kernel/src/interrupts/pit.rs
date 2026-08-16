use x86_64::instructions::port::Port;

const PIT_CHANNEL_0_DATA: u16 = 0x40;
const PIT_COMMAND_PORT: u16 = 0x43;
const PIT_BASE_FREQUENCY: u32 = 1_193_182;

pub const TIMER_FREQUENCY_HZ: u32 = 100;

pub fn init(frequency_hz: u32) {
    let divisor = (PIT_BASE_FREQUENCY / frequency_hz) as u16;

    let mut cmd_port = Port::<u8>::new(PIT_COMMAND_PORT);
    let mut data_port = Port::<u8>::new(PIT_CHANNEL_0_DATA);

    unsafe {
        // Channel 0, Access Mode: low/high byte, Mode 3 (Square Wave Generator), 16-bit binary
        cmd_port.write(0x36);
        data_port.write((divisor & 0xFF) as u8);
        data_port.write(((divisor >> 8) & 0xFF) as u8);
    }
}
