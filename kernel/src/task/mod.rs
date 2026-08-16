pub mod scheduler;
pub mod task;

use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
#[allow(unused_imports)]
pub use scheduler::{TaskSnapshot, SCHEDULER};
#[allow(unused_imports)]
pub use task::{TaskKind, TaskState};

pub static TASK_A_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static TASK_B_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn init() {
    SCHEDULER.lock().init();
}

pub fn spawn(name: &'static str, entry_fn: extern "C" fn()) -> usize {
    SCHEDULER.lock().spawn(name, entry_fn)
}

pub fn spawn_user(process_id: usize, name: &'static str, user_rip: u64, user_rsp: u64) -> usize {
    SCHEDULER.lock().spawn_user(process_id, name, user_rip, user_rsp)
}

pub fn add_task(task: task::Task) -> usize {
    SCHEDULER.lock().add_task(task)
}

pub fn set_task_state(task_id: usize, state: TaskState) {
    SCHEDULER.lock().set_task_state(task_id, state);
}

pub fn reset_task_user_context(
    task_id: usize,
    name: &'static str,
    user_rip: u64,
    user_rsp: u64,
) {
    SCHEDULER.lock().reset_task_user_context(task_id, name, user_rip, user_rsp);
}

pub fn current_process_id() -> usize {
    SCHEDULER.lock().current_process_id()
}

#[allow(dead_code)]
pub fn current_task_id() -> usize {
    SCHEDULER.lock().current_task_id()
}

#[allow(dead_code)]
pub fn get_task_snapshots() -> Vec<TaskSnapshot> {
    SCHEDULER.lock().snapshots()
}

extern "C" fn worker_a_entry() {
    for i in 1..=3 {
        TASK_A_COUNT.store(i, Ordering::Release);
        // Compute loop without yielding - relies completely on timer preemption
        for _ in 0..1_000_000 {
            core::hint::spin_loop();
        }
    }
}

extern "C" fn worker_b_entry() {
    for i in 1..=3 {
        TASK_B_COUNT.store(i, Ordering::Release);
        // Compute loop without yielding - relies completely on timer preemption
        for _ in 0..1_000_000 {
            core::hint::spin_loop();
        }
    }
}

pub fn start_preemption_demo() {
    TASK_A_COUNT.store(0, Ordering::Relaxed);
    TASK_B_COUNT.store(0, Ordering::Relaxed);

    spawn("worker_a", worker_a_entry);
    spawn("worker_b", worker_b_entry);
}
