use alloc::vec::Vec;
use spin::Mutex;
use super::task::{Task, TaskId, TaskKind, TaskState, TASK_STACK_SIZE};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct TaskSnapshot {
    pub id: TaskId,
    pub process_id: usize,
    pub name: &'static str,
    pub state: TaskState,
    pub kind: TaskKind,
    pub ticks: usize,
}

pub struct Scheduler {
    tasks: Vec<Task>,
    current_index: usize,
    ticks_in_current_slice: usize,
    slice_length: usize,
    initialized: bool,
}

impl Scheduler {
    pub const fn new() -> Self {
        Scheduler {
            tasks: Vec::new(),
            current_index: 0,
            ticks_in_current_slice: 0,
            slice_length: 1, // 1 timer tick per time slice
            initialized: false,
        }
    }

    pub fn init(&mut self) {
        if self.initialized {
            return;
        }
        // Task 1 is the kernel main/shell thread
        let kernel_task = Task::kernel_main_task();
        self.tasks.push(kernel_task);
        self.current_index = 0;
        self.initialized = true;
    }

    pub fn spawn(&mut self, name: &'static str, entry_fn: extern "C" fn()) -> usize {
        let task = Task::new(name, entry_fn);
        let id = task.id.0;
        self.tasks.push(task);
        id
    }

    pub fn spawn_user(&mut self, process_id: usize, name: &'static str, user_rip: u64, user_rsp: u64) -> usize {
        let task = Task::new_user_task(process_id, name, user_rip, user_rsp);
        let id = task.id.0;
        self.tasks.push(task);
        id
    }

    pub fn add_task(&mut self, task: Task) -> usize {
        let id = task.id.0;
        self.tasks.push(task);
        id
    }

    pub fn current_process_id(&self) -> usize {
        if self.current_index < self.tasks.len() {
            self.tasks[self.current_index].process_id
        } else {
            1
        }
    }

    #[allow(dead_code)]
    pub fn current_task_id(&self) -> usize {
        if self.current_index < self.tasks.len() {
            self.tasks[self.current_index].id.0
        } else {
            1
        }
    }

    pub fn current_task_mut(&mut self) -> Option<&mut Task> {
        if self.current_index < self.tasks.len() {
            Some(&mut self.tasks[self.current_index])
        } else {
            None
        }
    }

    pub fn set_task_state(&mut self, task_id: usize, state: TaskState) {
        for task in &mut self.tasks {
            if task.id.0 == task_id {
                task.state = state;
                break;
            }
        }
    }

    pub fn set_task_stopped(&mut self, task_id: usize, stopped: bool) {
        for task in &mut self.tasks {
            if task.id.0 == task_id {
                task.is_stopped = stopped;
                break;
            }
        }
    }

    pub fn remove_task(&mut self, task_id: usize) {
        if let Some(idx) = self.tasks.iter().position(|t| t.id.0 == task_id) {
            self.tasks.remove(idx);
            if idx < self.current_index {
                self.current_index -= 1;
            } else if idx == self.current_index {
                if self.current_index >= self.tasks.len() {
                    self.current_index = 0;
                }
            }
        }
    }

    pub fn reset_task_user_context(
        &mut self,
        task_id: usize,
        name: &'static str,
        user_rip: u64,
        user_rsp: u64,
    ) {
        for task in &mut self.tasks {
            if task.id.0 == task_id {
                task.reset_user_context(name, user_rip, user_rsp);
                break;
            }
        }
    }

    pub fn schedule_next(&mut self, current_rsp: usize) -> usize {
        if !self.initialized || self.tasks.is_empty() {
            return current_rsp;
        }

        // Save current RSP and increment tick counter for current task
        self.tasks[self.current_index].rsp = current_rsp;
        self.tasks[self.current_index].ticks += 1;
        self.ticks_in_current_slice += 1;

        let current_state = self.tasks[self.current_index].state;
        let is_stopped = self.tasks[self.current_index].is_stopped;
        let time_slice_expired = self.ticks_in_current_slice >= self.slice_length;
        let must_switch = current_state == TaskState::Finished
            || current_state == TaskState::Blocked
            || is_stopped
            || time_slice_expired;

        if !must_switch {
            return current_rsp;
        }

        self.ticks_in_current_slice = 0;

        if self.tasks[self.current_index].state == TaskState::Running {
            self.tasks[self.current_index].state = TaskState::Ready;
        }

        // Round-robin selection of the next Ready task
        let mut next_index = self.current_index;
        let mut found_next = false;

        for _ in 0..self.tasks.len() {
            next_index = (next_index + 1) % self.tasks.len();
            if self.tasks[next_index].state == TaskState::Ready && !self.tasks[next_index].is_stopped {
                found_next = true;
                break;
            }
        }

        if !found_next {
            if self.tasks[self.current_index].state == TaskState::Ready && !self.tasks[self.current_index].is_stopped {
                self.tasks[self.current_index].state = TaskState::Running;
                return self.tasks[self.current_index].rsp;
            }
            // Fallback to kernel main thread (Task 0)
            self.current_index = 0;
            self.tasks[0].state = TaskState::Running;
            return self.tasks[0].rsp;
        }

        self.current_index = next_index;
        self.tasks[self.current_index].state = TaskState::Running;

        // If target task has a kernel stack, update TSS privilege stack table
        if let Some(ref stack) = self.tasks[self.current_index].stack {
            let stack_top = (stack.as_ptr() as u64 + TASK_STACK_SIZE as u64) & !0xF;
            crate::gdt::set_privilege_stack(x86_64::VirtAddr::new(stack_top));
        }

        self.tasks[self.current_index].rsp
    }

    #[allow(dead_code)]
    pub fn snapshots(&self) -> Vec<TaskSnapshot> {
        let mut snaps = Vec::with_capacity(self.tasks.len());
        for task in &self.tasks {
            snaps.push(TaskSnapshot {
                id: task.id,
                process_id: task.process_id,
                name: task.name,
                state: task.state,
                kind: task.kind,
                ticks: task.ticks,
            });
        }
        snaps
    }
}

pub static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

pub fn tick(current_rsp: usize) -> usize {
    let mut sched = SCHEDULER.lock();
    let next_rsp = sched.schedule_next(current_rsp);
    drop(sched);
    next_rsp
}

pub fn exit_current_task() -> ! {
    x86_64::instructions::interrupts::disable();
    {
        let mut sched = SCHEDULER.lock();
        if let Some(curr) = sched.current_task_mut() {
            curr.state = TaskState::Finished;
        }
    }
    x86_64::instructions::interrupts::enable();

    loop {
        x86_64::instructions::hlt();
    }
}

pub fn block_current_task() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut sched = SCHEDULER.lock();
        if let Some(curr) = sched.current_task_mut() {
            curr.state = TaskState::Blocked;
        }
    });
    // Yield CPU immediately
    // Wait, we don't have yield_now exported? We can just spin until preempted.
    // Actually, setting Blocked and hlt-ing or waiting for timer interrupt is fine.
    loop {
        x86_64::instructions::interrupts::enable_and_hlt();
        x86_64::instructions::interrupts::disable();
        let mut sched = SCHEDULER.lock();
        if sched.current_task_mut().map(|t| t.state) == Some(TaskState::Running) {
            break;
        }
    }
    x86_64::instructions::interrupts::enable();
}

pub fn yield_current_task() {
    x86_64::instructions::interrupts::enable_and_hlt();
    x86_64::instructions::interrupts::disable();
}
