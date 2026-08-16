use alloc::boxed::Box;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const TASK_STACK_SIZE: usize = 32 * 1024; // 32 KiB

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId(pub usize);

impl TaskId {
    pub fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(2);
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Kernel,
    User,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ContextFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

pub struct Task {
    pub id: TaskId,
    pub process_id: usize,
    pub name: &'static str,
    pub state: TaskState,
    #[allow(dead_code)]
    pub kind: TaskKind,
    pub rsp: usize,
    pub ticks: usize,
    #[allow(dead_code)]
    pub stack: Option<Box<[u8; TASK_STACK_SIZE]>>,
}

impl Task {
    pub fn kernel_main_task() -> Self {
        Task {
            id: TaskId(1),
            process_id: 1,
            name: "kernel",
            state: TaskState::Running,
            kind: TaskKind::Kernel,
            rsp: 0,
            ticks: 0,
            stack: None,
        }
    }

    pub fn new(name: &'static str, entry_fn: extern "C" fn()) -> Self {
        let stack = Box::new([0u8; TASK_STACK_SIZE]);
        let stack_top = (stack.as_ptr() as usize + TASK_STACK_SIZE) & !0xF;

        let frame_size = core::mem::size_of::<ContextFrame>();
        let frame_ptr = (stack_top - frame_size) as *mut ContextFrame;

        let selectors = crate::gdt::get_selectors();
        let frame = ContextFrame {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rbp: 0,
            rdi: entry_fn as *const () as usize as u64,
            rsi: 0,
            rdx: 0,
            rcx: 0,
            rbx: 0,
            rax: 0,
            rip: task_trampoline as *const () as usize as u64,
            cs: selectors.kernel_code.0 as u64,
            rflags: 0x202, // IF=1
            rsp: stack_top as u64,
            ss: selectors.kernel_data.0 as u64,
        };

        unsafe {
            core::ptr::write(frame_ptr, frame);
        }

        Task {
            id: TaskId::new(),
            process_id: 1,
            name,
            state: TaskState::Ready,
            kind: TaskKind::Kernel,
            rsp: frame_ptr as usize,
            ticks: 0,
            stack: Some(stack),
        }
    }

    pub fn new_user_task(process_id: usize, name: &'static str, user_rip: u64, user_rsp: u64) -> Self {
        let stack = Box::new([0u8; TASK_STACK_SIZE]);
        let stack_top = (stack.as_ptr() as usize + TASK_STACK_SIZE) & !0xF;

        let frame_size = core::mem::size_of::<ContextFrame>();
        let frame_ptr = (stack_top - frame_size) as *mut ContextFrame;

        let selectors = crate::gdt::get_selectors();
        let frame = ContextFrame {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rbp: 0,
            rdi: 0,
            rsi: 0,
            rdx: 0,
            rcx: 0,
            rbx: 0,
            rax: 0,
            rip: user_rip,
            cs: (selectors.user_code.0 as u64) | 3, // Ring 3
            rflags: 0x202,                        // IF=1
            rsp: user_rsp,                        // User stack
            ss: (selectors.user_data.0 as u64) | 3, // Ring 3
        };

        unsafe {
            core::ptr::write(frame_ptr, frame);
        }

        Task {
            id: TaskId::new(),
            process_id,
            name,
            state: TaskState::Ready,
            kind: TaskKind::User,
            rsp: frame_ptr as usize,
            ticks: 0,
            stack: Some(stack),
        }
    }

    pub fn reset_user_context(&mut self, name: &'static str, user_rip: u64, user_rsp: u64) {
        if let Some(ref stack) = self.stack {
            let stack_top = (stack.as_ptr() as usize + TASK_STACK_SIZE) & !0xF;
            let frame_size = core::mem::size_of::<ContextFrame>();
            let frame_ptr = (stack_top - frame_size) as *mut ContextFrame;

            let selectors = crate::gdt::get_selectors();
            let frame = ContextFrame {
                r15: 0,
                r14: 0,
                r13: 0,
                r12: 0,
                r11: 0,
                r10: 0,
                r9: 0,
                r8: 0,
                rbp: 0,
                rdi: 0,
                rsi: 0,
                rdx: 0,
                rcx: 0,
                rbx: 0,
                rax: 0,
                rip: user_rip,
                cs: (selectors.user_code.0 as u64) | 3, // Ring 3
                rflags: 0x202,                        // IF=1
                rsp: user_rsp,                        // User stack
                ss: (selectors.user_data.0 as u64) | 3, // Ring 3
            };

            unsafe {
                core::ptr::write(frame_ptr, frame);
            }

            self.rsp = frame_ptr as usize;
            self.name = name;
            self.state = TaskState::Ready;
        }
    }
}

#[unsafe(naked)]
extern "C" fn task_trampoline() -> ! {
    core::arch::naked_asm!(
        "call rdi",
        "call {exit_task}",
        "2:",
        "hlt",
        "jmp 2b",
        exit_task = sym crate::task::scheduler::exit_current_task,
    );
}
