use alloc::vec::Vec;
use super::task::{Task, TaskId, TaskState};

#[derive(Debug, Clone, Copy)]
pub struct TaskSnapshot {
    pub id: TaskId,
    pub name: &'static str,
    pub state: TaskState,
    pub step_count: usize,
}

pub struct Executor {
    tasks: Vec<Task>,
}

impl Executor {
    pub const fn new() -> Self {
        Executor { tasks: Vec::new() }
    }

    pub fn spawn(&mut self, task: Task) {
        self.tasks.push(task);
    }

    pub fn run_ready_tasks(&mut self) -> usize {
        let mut executed_count = 0;

        for i in 0..self.tasks.len() {
            if self.tasks[i].state() == TaskState::Ready {
                executed_count += 1;
                let _ = self.tasks[i].poll();
            }
        }

        executed_count
    }

    pub fn run_all_to_completion(&mut self) {
        while self.has_ready_tasks() {
            self.run_ready_tasks();
        }
    }

    pub fn has_ready_tasks(&self) -> bool {
        self.tasks.iter().any(|t| t.state() == TaskState::Ready)
    }

    pub fn snapshots(&self) -> Vec<TaskSnapshot> {
        let mut snaps = Vec::with_capacity(self.tasks.len());
        for task in &self.tasks {
            snaps.push(TaskSnapshot {
                id: task.id(),
                name: task.name(),
                state: task.state(),
                step_count: task.step_count(),
            });
        }
        snaps
    }
}
