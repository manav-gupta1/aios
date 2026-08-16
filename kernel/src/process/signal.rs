#![allow(dead_code)]
pub const SIGINT: usize = 2;
pub const SIGKILL: usize = 9;
pub const SIGTERM: usize = 15;
pub const SIGCHLD: usize = 17;
pub const SIGCONT: usize = 18;
pub const SIGSTOP: usize = 19;
pub const SIGTSTP: usize = 20;

pub const SIG_DFL: usize = 0;
pub const SIG_IGN: usize = 1;
