use x86_64::structures::idt::{
    InterruptDescriptorTable,
    InterruptStackFrame,
};

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init() {
    unsafe {
        let idt = core::ptr::addr_of_mut!(IDT);

        (*idt)
            .breakpoint
            .set_handler_fn(breakpoint_handler);

        (*idt).load();
    }
}

extern "x86-interrupt" fn breakpoint_handler(
    _stack_frame: InterruptStackFrame,
) {
    // Breakpoint interrupt reached NOVA.
}
