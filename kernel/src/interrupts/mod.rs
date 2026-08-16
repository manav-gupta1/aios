
use pic8259::ChainedPics;
use x86_64::structures::idt::{
    InterruptDescriptorTable,
    InterruptStackFrame,
    PageFaultErrorCode,
};
use x86_64::{PrivilegeLevel, VirtAddr};

const PIC_1_OFFSET: u8 = 32;
const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;
const SYSCALL_VECTOR: u8 = 128; // 0x80

pub static mut KERNEL_CS: u64 = 0x08;
pub static mut KERNEL_SS: u64 = 0x10;

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
            .invalid_opcode
            .set_handler_fn(invalid_opcode_handler);

        (*idt)
            .double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(crate::gdt::DOUBLE_FAULT_IST_INDEX);

        (*idt)
            .general_protection_fault
            .set_handler_fn(gpf_handler);

        (*idt)
            .page_fault
            .set_handler_fn(page_fault_handler);

        // Timer IRQ0 -> interrupt vector 32 with preemptive context switching entry.
        (&mut *idt)[InterruptIndex::Timer.as_u8()]
            .set_handler_addr(VirtAddr::new(timer_interrupt_entry as *const () as usize as u64));

        // Keyboard IRQ1 -> interrupt vector 33.
        (&mut *idt)[InterruptIndex::Keyboard.as_u8()]
            .set_handler_fn(keyboard_handler);
            
        (&mut *idt)[32 + 9].set_handler_fn(irq9_handler);
        (&mut *idt)[32 + 10].set_handler_fn(irq10_handler);
        (&mut *idt)[32 + 11].set_handler_fn(irq11_handler);

        // Syscall vector 0x80 (128) with Ring 3 privilege level.
        (&mut *idt)[SYSCALL_VECTOR]
            .set_handler_addr(VirtAddr::new(syscall_interrupt_entry as *const () as usize as u64))
            .set_privilege_level(PrivilegeLevel::Ring3);

        (*idt).load();

        let pics = core::ptr::addr_of_mut!(PICS);
        (*pics).initialize();
        // Unmask IRQ0 (Timer) and IRQ1 (Keyboard), mask all unused IRQs.
        (*pics).write_masks(0xFC, 0xFF);
    }

    // Initialize 8254 Programmable Interval Timer at 100 Hz using TimerDriver
    crate::drivers::timer::TimerDriver::init(crate::drivers::timer::TIMER_FREQUENCY_HZ);

    x86_64::instructions::interrupts::enable();
}

extern "x86-interrupt" fn breakpoint_handler(
    _stack_frame: InterruptStackFrame,
) {
    // Breakpoint interrupt reached NOVA.
}

extern "x86-interrupt" fn invalid_opcode_handler(
    stack_frame: InterruptStackFrame,
) {
    if (stack_frame.code_segment.0 & 3) == 3 {
        // User space fault - terminate user process safely
        crate::console::write_str("\n[Fault] User process error: Invalid Opcode Exception (terminating task)\n");
        crate::process::PROCESS_TABLE.lock().exit_process(crate::process::current_pid(), 1);
        crate::task::scheduler::exit_current_task();
    } else {
        loop {
            core::hint::spin_loop();
        }
    }
}

extern "x86-interrupt" fn double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

extern "x86-interrupt" fn gpf_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) {
    if (stack_frame.code_segment.0 & 3) == 3 {
        // User space fault - terminate user process safely
        crate::drivers::storage::serial_print("\n[Fault] User process error: General Protection Fault (terminating task)\n");
        crate::console::write_str("\n[Fault] User process error: General Protection Fault (terminating task)\n");
        crate::process::PROCESS_TABLE.lock().exit_process(crate::process::current_pid(), 1);
        crate::task::scheduler::exit_current_task();
    } else {
        crate::drivers::storage::serial_print("\n[Fault] Kernel GPF\n");
        loop {
            core::hint::spin_loop();
        }
    }
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let fault_addr = x86_64::registers::control::Cr2::read().unwrap_or(x86_64::VirtAddr::new(0));

    // Check if it is a valid Copy-on-Write write fault (from user mode or syscall accessing user buffer)
    if error_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE) {
        let pid = crate::process::current_pid();
        if crate::process::handle_cow_fault(pid, fault_addr).is_ok() {
            // COW resolved successfully! Resume instruction execution.
            return;
        }
    }

    if (stack_frame.code_segment.0 & 3) == 3 {
        // User space fault - terminate user process safely
        crate::console::write_str("\n[Fault] User process error: Page Fault (terminating task)\n");
        crate::process::PROCESS_TABLE.lock().exit_process(crate::process::current_pid(), 1);
        crate::task::scheduler::exit_current_task();
    } else {
        crate::drivers::storage::serial_print("\n[Fault] Kernel Page Fault at: ");
        let mut num_str = alloc::string::String::new();
        use core::fmt::Write;
        let _ = write!(num_str, "{:#x}\n", fault_addr.as_u64());
        crate::drivers::storage::serial_print(&num_str);
        
        loop {
            core::hint::spin_loop();
        }
    }
}

#[unsafe(naked)]
pub unsafe extern "C" fn timer_interrupt_entry() {
    core::arch::naked_asm!(
        "push rax",
        "push rbx",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rbp",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        "mov rdi, rsp",
        "call {schedule_tick}",

        "mov rsp, rax",

        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rbp",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rbx",
        "pop rax",

        "iretq",
        schedule_tick = sym schedule_tick_handler,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn schedule_tick_handler(current_rsp: usize) -> usize {
    unsafe {
        let cs_ptr = (current_rsp + 16 * 8) as *const u64;
        let ss_ptr = (current_rsp + 19 * 8) as *const u64;
        KERNEL_CS = *cs_ptr;
        KERNEL_SS = *ss_ptr;

        let pics = core::ptr::addr_of_mut!(PICS);
        (*pics).notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }

    crate::drivers::timer::TimerDriver::tick(current_rsp)
}

#[unsafe(naked)]
pub unsafe extern "C" fn syscall_interrupt_entry() {
    core::arch::naked_asm!(
        "push rax",
        "push rbx",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rbp",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // SysV ABI calling convention for syscall_dispatch:
        // RDI = num (RAX)
        // RSI = arg1 (RDI)
        // RDX = arg2 (RSI)
        // RCX = arg3 (RDX)
        // R8  = saved_cs ([rsp + 16*8])
        // R9  = frame_ptr (RSP)
        "mov r9, rsp",
        "mov r8, [rsp + 16 * 8]",
        "mov rcx, rdx",
        "mov rdx, rsi",
        "mov rsi, rdi",
        "mov rdi, rax",
        "call {syscall_handler}",

        // Put return value into saved RAX on stack (offset 14*8 = 112)
        "mov [rsp + 14 * 8], rax",

        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rbp",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rbx",
        "pop rax",

        "iretq",
        syscall_handler = sym syscall_handler_wrapper,
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn syscall_handler_wrapper(
    num: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    saved_cs: u64,
    frame_ptr: *mut u64,
) -> u64 {
    crate::syscall::syscall_dispatch(num, arg1, arg2, arg3, saved_cs, frame_ptr)
}

extern "x86-interrupt" fn keyboard_handler(
    _stack_frame: InterruptStackFrame,
) {
    if let Some(character) = crate::drivers::KEYBOARD_DRIVER.lock().handle_interrupt() {
        let event = crate::tty::TTY.lock().tty_input(character);
        match event {
            crate::tty::TtyEvent::WakeReaders(pids) => {
                for pid in pids {
                    crate::process::PROCESS_TABLE.lock().wake_process_by_pid(pid);
                }
            }
            crate::tty::TtyEvent::SignalForeground(signum) => {
                let fg_pgid = crate::tty::TTY.lock().foreground_pgid();
                let _ = crate::process::PROCESS_TABLE.lock().sys_killpg(0, fg_pgid, signum);
            }
            crate::tty::TtyEvent::None => {}
        }
    }

    unsafe {
        let pics = core::ptr::addr_of_mut!(PICS);

        (*pics).notify_end_of_interrupt(
            InterruptIndex::Keyboard.as_u8(),
        );
    }
}

extern "x86-interrupt" fn irq9_handler(_stack_frame: InterruptStackFrame) {
    crate::drivers::virtio::block::handle_irq();
    unsafe {
        let pics = core::ptr::addr_of_mut!(PICS);
        (*pics).notify_end_of_interrupt(32 + 9);
    }
}

extern "x86-interrupt" fn irq10_handler(_stack_frame: InterruptStackFrame) {
    crate::drivers::virtio::block::handle_irq();
    unsafe {
        let pics = core::ptr::addr_of_mut!(PICS);
        (*pics).notify_end_of_interrupt(32 + 10);
    }
}

extern "x86-interrupt" fn irq11_handler(_stack_frame: InterruptStackFrame) {
    crate::drivers::virtio::block::handle_irq();
    unsafe {
        let pics = core::ptr::addr_of_mut!(PICS);
        (*pics).notify_end_of_interrupt(32 + 11);
    }
}

pub fn unmask_irq(irq: u8) {
    unsafe {
        let pics = core::ptr::addr_of_mut!(PICS);
        let mut mask1 = 0xFF;
        let mut mask2 = 0xFF;
        // Assume default mask (Timer + Keyboard unmasked)
        mask1 &= !0x03; 
        
        if irq < 8 {
            mask1 &= !(1 << irq);
        } else {
            mask1 &= !(1 << 2); // Cascade
            mask2 &= !(1 << (irq - 8));
        }
        (*pics).write_masks(mask1, mask2);
    }
}
