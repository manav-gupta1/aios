use pic8259::ChainedPics;
use x86_64::structures::idt::{
    InterruptDescriptorTable,
    InterruptStackFrame,
};

const PIC_1_OFFSET: u8 = 32;
const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }
}

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

static mut PICS: ChainedPics =
    unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) };

pub fn init() {
    unsafe {
        let idt = core::ptr::addr_of_mut!(IDT);

        (*idt)
            .breakpoint
            .set_handler_fn(breakpoint_handler);

        (*idt)
            .double_fault
            .set_handler_fn(double_fault_handler);

        // Timer IRQ0 -> interrupt vector 32.
        (&mut *idt)[InterruptIndex::Timer.as_u8()]
            .set_handler_fn(timer_handler);

        // Keyboard IRQ1 -> interrupt vector 33.
        (&mut *idt)[InterruptIndex::Keyboard.as_u8()]
            .set_handler_fn(keyboard_handler);

        (*idt).load();

        let pics = core::ptr::addr_of_mut!(PICS);
        (*pics).initialize();
        // Unmask IRQ0 (Timer) and IRQ1 (Keyboard), mask all unused IRQs.
        (*pics).write_masks(0xFC, 0xFF);
    }

    x86_64::instructions::interrupts::enable();
}

extern "x86-interrupt" fn breakpoint_handler(
    _stack_frame: InterruptStackFrame,
) {
    // Breakpoint interrupt reached NOVA.
}

extern "x86-interrupt" fn double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

extern "x86-interrupt" fn timer_handler(
    _stack_frame: InterruptStackFrame,
) {
    unsafe {
        let pics = core::ptr::addr_of_mut!(PICS);

        (*pics).notify_end_of_interrupt(
            InterruptIndex::Timer.as_u8(),
        );
    }
}

extern "x86-interrupt" fn keyboard_handler(
    _stack_frame: InterruptStackFrame,
) {
    crate::keyboard::handle_interrupt();

    unsafe {
        let pics = core::ptr::addr_of_mut!(PICS);

        (*pics).notify_end_of_interrupt(
            InterruptIndex::Keyboard.as_u8(),
        );
    }
}
