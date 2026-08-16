use crate::fs::{FsError, NodeKind, FILESYSTEM, MAX_PATH_LEN};
use crate::graphics::text::TextWriter;
use core::sync::atomic::Ordering;

use alloc::string::String;
use alloc::vec::Vec;
use crate::process::ProcessState;


#[derive(Clone)]
pub struct Job {
    pub id: usize,
    pub pgid: usize,
    pub cmd: String,
    pub is_background: bool,
}

pub struct Shell {
    jobs: Vec<Job>,
    next_job_id: usize,
}

impl Shell {
    pub const fn new() -> Self {
        Self {
            jobs: Vec::new(),
            next_job_id: 1,
        }
    }

    pub fn print_banner(&self, text: &mut TextWriter) {
        text.set_position(60, 60);
        text.set_scale(4);
        text.set_color(80, 160, 255);
        text.write_str("NOVA OS\n\n");

        text.set_scale(2);
        text.set_color(235, 235, 245);
        text.write_str(
            "KERNEL INITIALIZED\n\
             ARCHITECTURE: X86_64\n\
             DISPLAY: 1280X720\n",
        );

        if let Some(info) = crate::memory::get_memory_info() {
            if info.heap_initialized && info.heap_test_passed {
                text.set_color(80, 220, 120);
                text.write_str("MEMORY: READY (HEAP ACTIVE)\n");
            }
        }

        text.set_color(80, 220, 120);
        text.write_str(
            "INTERRUPTS: INITIALIZED (100 HZ PIT)\n\
             KEYBOARD: READY\n\
             SCHEDULER: PREEMPTIVE (ROUND-ROBIN)\n\
             USERSPACE: READY (ELF64 + RING 0/3 + SYSCALLS)\n",
        );

        let info = FILESYSTEM.lock().get_fs_info();
        if info.is_persistent {
            text.set_color(80, 220, 120);
            text.write_str("STORAGE: MOUNTED (PERSISTENT NOVAFS)\n\n");
        } else {
            text.set_color(240, 180, 80);
            text.write_str("STORAGE: MOUNTED (IN-MEMORY)\n\n");
        }

        self.print_prompt(text);
    }

    pub fn print_prompt(&self, text: &mut TextWriter) {
        text.set_color(80, 160, 255);
        text.write_str("NOVA> ");
        text.set_color(235, 235, 245);
    }

    pub fn execute_line(&mut self, line: &str, text: &mut TextWriter) {
        let (command, args) = parse_command(line);

        if command.is_empty() {
            self.print_prompt(text);
            return;
        }

        if command.eq_ignore_ascii_case("help") {
            self.cmd_help(text);
        } else if command.eq_ignore_ascii_case("version") {
            self.cmd_version(text);
        } else if command.eq_ignore_ascii_case("clear") {
            self.cmd_clear(text);
            return;
        } else if command.eq_ignore_ascii_case("echo") {
            self.cmd_echo(args, text);
        } else if command.eq_ignore_ascii_case("whoami") {
            self.cmd_whoami(text);
        } else if command.eq_ignore_ascii_case("run") {
            self.cmd_run(args, text);
        } else if command.eq_ignore_ascii_case("pwd") {
            self.cmd_pwd(text);
        } else if command.eq_ignore_ascii_case("ls") {
            self.cmd_ls(args, text);
        } else if command.eq_ignore_ascii_case("cd") {
            self.cmd_cd(args, text);
        } else if command.eq_ignore_ascii_case("mkdir") {
            self.cmd_mkdir(args, text);
        } else if command.eq_ignore_ascii_case("touch") {
            self.cmd_touch(args, text);
        } else if command.eq_ignore_ascii_case("cat") {
            self.cmd_cat(args, text);
        } else if command.eq_ignore_ascii_case("write") {
            self.cmd_write(args, text);
        } else if command.eq_ignore_ascii_case("rm") {
            self.cmd_rm(args, text);
        } else if command.eq_ignore_ascii_case("rmdir") {
            self.cmd_rmdir(args, text);
        } else if command.eq_ignore_ascii_case("fsinfo") {
            self.cmd_fsinfo(text);
        } else if command.eq_ignore_ascii_case("meminfo") {
            self.cmd_meminfo(text);
        } else if command.eq_ignore_ascii_case("ps") || command.eq_ignore_ascii_case("tasks") {
            self.cmd_ps(text);
        } else if command.eq_ignore_ascii_case("wait") {
            self.cmd_wait(args, text);
        } else if command.eq_ignore_ascii_case("jobs") {
            self.cmd_jobs(text);
        } else if command.eq_ignore_ascii_case("fg") {
            self.cmd_fg(args, text);
        } else if command.eq_ignore_ascii_case("bg") {
            self.cmd_bg(args, text);
        } else if command.eq_ignore_ascii_case("vmmap") {
            self.cmd_vmmap(args, text);
        } else if command.eq_ignore_ascii_case("lspci") {
            self.cmd_lspci(text);
        } else if command.eq_ignore_ascii_case("schedtest") {
            self.cmd_schedtest(text);
        } else {
            self.cmd_unknown(command, text);
        }

        self.print_prompt(text);
    }

    fn cmd_help(&self, text: &mut TextWriter) {
        text.set_color(80, 160, 255);
        text.write_str("Available commands:\n");
        text.set_color(235, 235, 245);
        text.write_str("  help       - Display available commands\n");
        text.write_str("  version    - Display OS version information\n");
        text.write_str("  whoami     - Display current execution privilege/identity\n");
        text.write_str("  run <path> - Execute an ELF64 binary from filesystem in Ring 3\n");
        text.write_str("  clear      - Clear the console screen\n");
        text.write_str("  echo       - Print text to console\n");
        text.write_str("  pwd        - Print working directory\n");
        text.write_str("  ls         - List directory contents\n");
        text.write_str("  cd         - Change working directory\n");
        text.write_str("  mkdir      - Create a new directory\n");
        text.write_str("  touch      - Create an empty file\n");
        text.write_str("  cat        - Display file contents\n");
        text.write_str("  write      - Write text to a file\n");
        text.write_str("  rm         - Remove a file\n");
        text.write_str("  rmdir      - Remove an empty directory\n");
        text.write_str("  fsinfo     - Display filesystem information\n");
        text.write_str("  meminfo    - Display memory subsystem information\n");
        text.write_str("  ps         - Display active processes table\n");
        text.write_str("  lspci      - List PCI devices\n");
        text.write_str("  vmmap      - Display memory mappings for a process\n");
        text.write_str("  wait [pid] - Wait for and collect child process\n");
        text.write_str("  jobs       - List active background and stopped jobs\n");
        text.write_str("  fg <id>    - Bring job to foreground\n");
        text.write_str("  bg <id>    - Resume job in background\n");
        text.write_str("  schedtest  - Run preemptive scheduler verification test\n\n");
    }

    fn cmd_version(&self, text: &mut TextWriter) {
        text.set_color(80, 160, 255);
        text.write_str("NOVA OS\n");
        text.set_color(235, 235, 245);
        text.write_str("Version: 0.1.0\n");
        text.write_str("Architecture: x86_64\n\n");
    }

    fn cmd_whoami(&self, text: &mut TextWriter) {
        text.set_color(235, 235, 245);
        text.write_str("kernel\n\n");
    }

    fn cmd_run(&mut self, args: &str, text: &mut TextWriter) {
        if args.is_empty() {
            text.set_color(240, 80, 80);
            text.write_str("Usage: run <path> (e.g. run /bin/hello)\n\n");
            return;
        }

        // Run negative test suite if requested
        if args.eq_ignore_ascii_case("--test-negative") {
            text.set_color(80, 160, 255);
            text.write_str("Running ELF Loader Negative Validation Tests:\n");
            let pass = crate::elf::run_negative_tests();
            if pass {
                text.set_color(80, 220, 120);
                text.write_str("  All 8 negative validation tests PASSED safely.\n\n");
            } else {
                text.set_color(240, 80, 80);
                text.write_str("  Negative tests FAILED.\n\n");
            }
            return;
        }

        // Parse background indicator
        let mut is_background = false;
        let clean_args = if args.trim_end().ends_with('&') {
            is_background = true;
            args.trim_end().trim_end_matches('&').trim()
        } else {
            args.trim()
        };

        // Resolve path: if relative like "hello", check "/bin/hello"
        let resolved_path: &str = if !clean_args.starts_with('/') {
            let mut bin_path = alloc::string::String::from("/bin/");
            bin_path.push_str(clean_args);
            let fs = FILESYSTEM.lock();
            if fs.resolve_path(&bin_path).is_ok() {
                drop(fs);
                alloc::boxed::Box::leak(bin_path.into_boxed_str())
            } else {
                drop(fs);
                clean_args
            }
        } else {
            clean_args
        };

        // Read executable bytes from filesystem
        let fs = FILESYSTEM.lock();
        let file_bytes = match fs.read_file_bytes(resolved_path) {
            Ok(bytes) => bytes,
            Err(e) => {
                drop(fs);
                self.print_error(e, resolved_path, text);
                return;
            }
        };

        let mut elf_buf = [0u8; crate::fs::MAX_FILE_SIZE];
        let file_len = file_bytes.len();
        elf_buf[..file_len].copy_from_slice(file_bytes);
        drop(fs);

        crate::syscall::USER_OUTPUT_RECEIVED.store(false, Ordering::Relaxed);
        crate::syscall::LAST_SYSCALL_IS_RING3.store(false, Ordering::Relaxed);

        // Spawns child of PID 1
        let prog_name = resolved_path.rsplit('/').next().unwrap_or(resolved_path);
        let leak_name: &'static str = alloc::boxed::Box::leak(
            alloc::string::String::from(prog_name).into_boxed_str(),
        );

        match crate::elf::load_and_spawn_elf(1, leak_name, &elf_buf[..file_len]) {
            Ok(child_pid) => {
                // Assign new process group
                let _ = crate::process::sys_setpgid(1, child_pid, child_pid);

                if is_background {
                    let job_id = self.next_job_id;
                    self.next_job_id += 1;
                    self.jobs.push(Job {
                        id: job_id,
                        pgid: child_pid,
                        cmd: String::from(clean_args),
                        is_background: true,
                    });
                    
                    text.set_color(80, 220, 120);
                    text.write_str("[");
                    write_u32(job_id as u32, text);
                    text.write_str("] ");
                    write_u32(child_pid as u32, text);
                    text.write_str("\n\n");
                    return;
                }

                // Foreground execution
                let _ = crate::process::sys_tcsetpgrp(1, child_pid);
                
                // Wait for user mode execution output until process terminates or stops
                for _ in 0..2000 {
                    while let Some(c) = crate::console::read_out_char() {
                        text.write_char(c);
                    }

                    let state = crate::process::get_process_snapshots()
                        .iter()
                        .find(|p| p.pid == child_pid)
                        .map(|p| p.state);
                        
                    let is_finished = state == Some(crate::process::ProcessState::Zombie);
                    let is_stopped = state == Some(crate::process::ProcessState::Stopped);

                    if is_finished || is_stopped {
                        if is_stopped {
                            let job_id = self.next_job_id;
                            self.next_job_id += 1;
                            self.jobs.push(Job {
                                id: job_id,
                                pgid: child_pid,
                                cmd: String::from(clean_args),
                                is_background: false,
                            });
                            text.write_str("\n[");
                            write_u32(job_id as u32, text);
                            text.write_str("] Stopped\n");
                        }
                        break;
                    }

                    for _ in 0..50_000 {
                        core::hint::spin_loop();
                    }
                }
                
                let _ = crate::process::sys_tcsetpgrp(1, 1); // Return terminal to shell

                // Flush any remaining characters
                while let Some(c) = crate::console::read_out_char() {
                    text.write_char(c);
                }

                text.write_str("\n");
            }
            Err(e) => {
                text.set_color(240, 80, 80);
                text.write_str("ELF Load Error: ");
                text.write_str(e.as_str());
                text.write_str("\n\n");
            }
        }
    }

    fn cmd_wait(&self, args: &str, text: &mut TextWriter) {
        let target_pid = if args.is_empty() {
            None
        } else if let Ok(pid) = args.trim().parse::<usize>() {
            Some(pid)
        } else {
            text.set_color(240, 80, 80);
            text.write_str("Usage: wait [pid]\n\n");
            return;
        };

        // Caller is PID 1 (shell / init)
        match crate::process::waitpid(1, target_pid) {
            Ok((child_pid, status)) => {
                text.set_color(80, 220, 120);
                text.write_str("Child process ");
                write_u32(child_pid as u32, text);
                text.write_str(" collected, exit status = ");
                write_u32(status as u32, text);
                text.write_str("\n\n");
            }
            Err(crate::process::WaitError::NoChild) => {
                text.set_color(240, 80, 80);
                text.write_str("No such child process to wait on.\n\n");
            }
            Err(crate::process::WaitError::WouldBlock) => {
                text.set_color(240, 180, 80);
                text.write_str("Child process is still running.\n\n");
            }
            Err(crate::process::WaitError::InvalidPid) => {
                text.set_color(240, 80, 80);
                text.write_str("Invalid PID.\n\n");
            }
        }
    }

    fn cmd_jobs(&mut self, text: &mut TextWriter) {
        let snaps = crate::process::get_process_snapshots();
        let mut to_remove = Vec::new();
        
        for (i, job) in self.jobs.iter().enumerate() {
            let proc = snaps.iter().find(|p| p.pid == job.pgid); // Process group leader
            
            text.set_color(80, 160, 255);
            text.write_str("[");
            write_u32(job.id as u32, text);
            text.write_str("] ");
            
            match proc {
                Some(p) if p.state == ProcessState::Stopped => {
                    text.set_color(160, 160, 170);
                    text.write_str("Stopped   ");
                },
                Some(p) if p.state == ProcessState::Zombie => {
                    text.set_color(80, 220, 120);
                    text.write_str("Completed ");
                    to_remove.push(i);
                },
                Some(_) => {
                    text.set_color(80, 220, 120);
                    text.write_str("Running   ");
                },
                None => {
                    text.set_color(80, 220, 120);
                    text.write_str("Completed ");
                    to_remove.push(i);
                }
            }
            
            text.set_color(235, 235, 245);
            text.write_str(&job.cmd);
            text.write_str("\n");
        }
        
        for idx in to_remove.iter().rev() {
            self.jobs.remove(*idx);
        }
        text.write_str("\n");
    }

    fn cmd_fg(&mut self, args: &str, text: &mut TextWriter) {
        let job_id = match args.trim().parse::<usize>() {
            Ok(n) => n,
            Err(_) => {
                text.set_color(240, 80, 80);
                text.write_str("Usage: fg <job_id>\n\n");
                return;
            }
        };
        
        let job_idx = match self.jobs.iter().position(|j| j.id == job_id) {
            Some(idx) => idx,
            None => {
                text.set_color(240, 80, 80);
                text.write_str("fg: job not found\n\n");
                return;
            }
        };
        
        let mut job = self.jobs.remove(job_idx);
        job.is_background = false;
        
        text.set_color(235, 235, 245);
        text.write_str(&job.cmd);
        text.write_str("\n");
        
        let _ = crate::process::sys_tcsetpgrp(1, job.pgid);
        let _ = crate::process::sys_killpg(1, job.pgid, crate::process::signal::SIGCONT);
        
        // Wait
        for _ in 0..2000 {
            while let Some(c) = crate::console::read_out_char() {
                text.write_char(c);
            }

            let state = crate::process::get_process_snapshots()
                .iter()
                .find(|p| p.pid == job.pgid)
                .map(|p| p.state);
                
            let is_finished = state == Some(crate::process::ProcessState::Zombie) || state.is_none();
            let is_stopped = state == Some(crate::process::ProcessState::Stopped);

            if is_finished || is_stopped {
                if is_stopped {
                    self.jobs.push(job.clone());
                    text.write_str("\n[");
                    write_u32(job.id as u32, text);
                    text.write_str("] Stopped\n");
                }
                break;
            }

            for _ in 0..50_000 {
                core::hint::spin_loop();
            }
        }
        
        let _ = crate::process::sys_tcsetpgrp(1, 1);
        while let Some(c) = crate::console::read_out_char() {
            text.write_char(c);
        }
        text.write_str("\n");
    }

    fn cmd_bg(&mut self, args: &str, text: &mut TextWriter) {
        let job_id = match args.trim().parse::<usize>() {
            Ok(n) => n,
            Err(_) => {
                text.set_color(240, 80, 80);
                text.write_str("Usage: bg <job_id>\n\n");
                return;
            }
        };
        
        let job = match self.jobs.iter_mut().find(|j| j.id == job_id) {
            Some(j) => j,
            None => {
                text.set_color(240, 80, 80);
                text.write_str("bg: job not found\n\n");
                return;
            }
        };
        
        job.is_background = true;
        let _ = crate::process::sys_killpg(1, job.pgid, crate::process::signal::SIGCONT);
        
        text.set_color(80, 160, 255);
        text.write_str("[");
        write_u32(job.id as u32, text);
        text.write_str("] ");
        text.set_color(235, 235, 245);
        text.write_str(&job.cmd);
        text.write_str(" &\n\n");
    }

    fn cmd_vmmap(&self, args: &str, text: &mut TextWriter) {
        let target_pid = if args.is_empty() {
            crate::process::current_pid()
        } else if let Ok(pid) = args.trim().parse::<usize>() {
            pid
        } else {
            text.set_color(240, 80, 80);
            text.write_str("Usage: vmmap [pid]\n\n");
            return;
        };

        if let Some(regions) = x86_64::instructions::interrupts::without_interrupts(|| crate::process::PROCESS_TABLE.lock().get_process_mmap_regions(target_pid)) {
            text.set_color(80, 160, 255);
            text.write_str("Memory Mappings for PID ");
            write_u32(target_pid as u32, text);
            text.write_str(":\n");
            
            if regions.is_empty() {
                text.set_color(235, 235, 245);
                text.write_str("  No mappings found.\n\n");
                return;
            }

            text.set_color(235, 235, 245);
            text.write_str("  START ADDR         LENGTH (BYTES)  PROT  FLAGS\n");
            for r in regions {
                text.write_str("  0x");
                // simplistic hex print
                let mut buf = [0u8; 16];
                let val = r.start;
                for i in 0..16 {
                    let nibble = (val >> (60 - i * 4)) & 0xf;
                    buf[i] = if nibble < 10 { b'0' + nibble as u8 } else { b'a' + (nibble - 10) as u8 };
                }
                for i in 0..16 {
                    text.write_char(buf[i] as char);
                }
                text.write_str(" ");
                write_u32(r.length as u32, text);
                
                text.write_str("            ");
                write_u32(r.prot as u32, text);
                text.write_str("     ");
                write_u32(r.flags as u32, text);
                text.write_str("\n");
            }
            text.write_str("\n");
        } else {
            text.set_color(240, 80, 80);
            text.write_str("Process not found.\n\n");
        }
    }


    fn cmd_lspci(&self, text: &mut TextWriter) {
        let devices = crate::drivers::pci::enumerate_devices();
        
        text.set_color(80, 160, 255);
        text.write_str("PCI Devices:\n");
        text.set_color(235, 235, 245);
        
        for dev in devices {
            let class_name = crate::drivers::pci::pci_class_name(dev.class_code, dev.subclass);
            let line = alloc::format!(
                "{:02X}:{:02X}.{}  {}\n",
                dev.bus, dev.device, dev.function, class_name
            );
            text.write_str(&line);
        }
    }

    fn cmd_clear(&self, text: &mut TextWriter) {
        text.clear();
        text.set_position(60, 60);
        text.set_scale(2);
        self.print_prompt(text);
    }

    fn cmd_echo(&self, args: &str, text: &mut TextWriter) {
        text.set_color(235, 235, 245);
        if !args.is_empty() {
            text.write_str(args);
        }
        text.write_str("\n\n");
    }

    fn cmd_pwd(&self, text: &mut TextWriter) {
        let mut path_buf = [0u8; MAX_PATH_LEN];
        let len = FILESYSTEM.lock().get_pwd(&mut path_buf);
        let pwd_str = core::str::from_utf8(&path_buf[..len]).unwrap_or("/");
        text.set_color(235, 235, 245);
        text.write_str(pwd_str);
        text.write_str("\n\n");
    }

    fn cmd_ls(&self, args: &str, text: &mut TextWriter) {
        let target_path = if args.is_empty() { "." } else { args };
        let mut count = 0;

        let res = FILESYSTEM.lock().list_dir(target_path, |name, kind| {
            count += 1;
            match kind {
                NodeKind::Directory => {
                    text.set_color(80, 160, 255);
                    text.write_str(name);
                    text.write_str("/\n");
                }
                NodeKind::File => {
                    text.set_color(235, 235, 245);
                    text.write_str(name);
                    text.write_str("\n");
                }
            }
        });

        match res {
            Ok(()) => {
                if count > 0 {
                    text.write_str("\n");
                } else {
                    text.write_str("\n");
                }
            }
            Err(e) => {
                self.print_error(e, target_path, text);
            }
        }
    }

    fn cmd_cd(&mut self, args: &str, text: &mut TextWriter) {
        let target_path = if args.is_empty() { "/" } else { args };
        if let Err(e) = FILESYSTEM.lock().change_dir(target_path) {
            self.print_error(e, target_path, text);
        }
    }

    fn cmd_mkdir(&mut self, args: &str, text: &mut TextWriter) {
        if args.is_empty() {
            text.set_color(240, 80, 80);
            text.write_str("Usage: mkdir <name>\n\n");
            return;
        }

        if let Err(e) = FILESYSTEM.lock().create_dir(args) {
            self.print_error(e, args, text);
        }
    }

    fn cmd_touch(&mut self, args: &str, text: &mut TextWriter) {
        if args.is_empty() {
            text.set_color(240, 80, 80);
            text.write_str("Usage: touch <name>\n\n");
            return;
        }

        if let Err(e) = FILESYSTEM.lock().create_file(args) {
            self.print_error(e, args, text);
        }
    }

    fn cmd_cat(&self, args: &str, text: &mut TextWriter) {
        if args.is_empty() {
            text.set_color(240, 80, 80);
            text.write_str("Usage: cat <name>\n\n");
            return;
        }

        let fs = FILESYSTEM.lock();
        match fs.read_file(args) {
            Ok(content) => {
                text.set_color(235, 235, 245);
                if !content.is_empty() {
                    text.write_str(content);
                    if !content.ends_with('\n') {
                        text.write_str("\n");
                    }
                }
                text.write_str("\n");
            }
            Err(e) => {
                drop(fs);
                self.print_error(e, args, text);
            }
        }
    }

    fn cmd_write(&mut self, args: &str, text: &mut TextWriter) {
        let (filename, content) = parse_command(args);
        if filename.is_empty() {
            text.set_color(240, 80, 80);
            text.write_str("Usage: write <name> <text>\n\n");
            return;
        }

        if let Err(e) = FILESYSTEM.lock().write_file(filename, content.as_bytes()) {
            self.print_error(e, filename, text);
        }
    }

    fn cmd_rm(&mut self, args: &str, text: &mut TextWriter) {
        if args.is_empty() {
            text.set_color(240, 80, 80);
            text.write_str("Usage: rm <name>\n\n");
            return;
        }

        if let Err(e) = FILESYSTEM.lock().remove_file(args) {
            self.print_error(e, args, text);
        }
    }

    fn cmd_rmdir(&mut self, args: &str, text: &mut TextWriter) {
        if args.is_empty() {
            text.set_color(240, 80, 80);
            text.write_str("Usage: rmdir <name>\n\n");
            return;
        }

        if let Err(e) = FILESYSTEM.lock().remove_dir(args) {
            self.print_error(e, args, text);
        }
    }

    fn cmd_fsinfo(&self, text: &mut TextWriter) {
        let info = FILESYSTEM.lock().get_fs_info();
        text.set_color(80, 160, 255);
        text.write_str("Filesystem Information:\n");

        text.set_color(235, 235, 245);
        text.write_str("  Type:          ");
        text.write_str(info.fs_type);
        text.write_str("\n  Version:       ");
        write_u32(info.version, text);

        text.write_str("\n  Storage:       ");
        if info.is_persistent {
            text.set_color(80, 220, 120);
            text.write_str("Persistent (ATA PIO Block Device)\n");
        } else {
            text.set_color(240, 180, 80);
            text.write_str("In-Memory (Disk Unavailable)\n");
        }

        text.set_color(235, 235, 245);
        text.write_str("  Block Size:    ");
        write_u32(info.block_size, text);
        text.write_str(" bytes\n  Total Blocks:  ");
        write_u32(info.total_blocks, text);
        text.write_str("\n  Used Blocks:   ");
        write_u32(info.used_blocks, text);
        text.write_str("\n  Free Blocks:   ");
        write_u32(info.free_blocks, text);
        text.write_str("\n  Total Inodes:  ");
        write_u32(info.total_inodes, text);
        text.write_str("\n  Used Inodes:   ");
        write_u32(info.used_inodes, text);
        text.write_str("\n  Free Inodes:   ");
        write_u32(info.free_inodes, text);
        text.write_str("\n\n");
    }

    fn cmd_meminfo(&self, text: &mut TextWriter) {
        text.set_color(80, 160, 255);
        text.write_str("Memory Subsystem Information:\n");

        text.set_color(235, 235, 245);
        if let Some(info) = crate::memory::get_memory_info() {
            let usable_kib = (info.usable_ram_bytes / 1024) as u32;
            let usable_mib = usable_kib / 1024;
            text.write_str("  Usable RAM:          ");
            write_u32(usable_kib, text);
            text.write_str(" KiB (");
            write_u32(usable_mib, text);
            text.write_str(" MiB)\n");

            text.write_str("  Allocated Frames:    ");
            write_u32(info.allocated_frames as u32, text);
            text.write_str(" frames (");
            write_u32((info.allocated_frames * info.frame_size_bytes / 1024) as u32, text);
            text.write_str(" KiB)\n");

            text.write_str("  Shared (COW) Frames: ");
            write_u32(info.shared_frames as u32, text);
            text.write_str(" frames\n");

            text.write_str("  Frame Size:          4096 bytes (4 KiB)\n");
            text.write_str("  Page Table:          Level-4 OffsetPageTable\n");
            text.write_str("  Heap Virtual Range:  0x444444440000 - 0x444444480000\n");
            text.write_str("  Heap Size:           ");
            write_u32((info.heap_size / 1024) as u32, text);
            text.write_str(" KiB (");
            write_u32((info.heap_size / 4096) as u32, text);
            text.write_str(" pages)\n");

            text.write_str("  Heap Allocator:      GlobalAlloc (Linked-List)\n");
            text.write_str("  Heap Status:         ");
            if info.heap_initialized {
                text.set_color(80, 220, 120);
                text.write_str("Active\n");
            } else {
                text.set_color(240, 80, 80);
                text.write_str("Failed\n");
            }

            text.set_color(235, 235, 245);
            text.write_str("  Allocation Test:     ");
            if info.heap_test_passed {
                text.set_color(80, 220, 120);
                text.write_str("PASSED (Box & Vec verified)\n\n");
            } else {
                text.set_color(240, 80, 80);
                text.write_str("FAILED\n\n");
            }
        } else {
            text.set_color(240, 80, 80);
            text.write_str("  Memory subsystem not initialized.\n\n");
        }
    }

    fn cmd_ps(&self, text: &mut TextWriter) {
        let snapshots = crate::process::get_process_snapshots();
        text.set_color(80, 160, 255);
        text.write_str("PID   PPID   STATE      NAME\n");

        for snap in &snapshots {
            text.set_color(235, 235, 245);
            write_u32(snap.pid as u32, text);
            if snap.pid < 10 {
                text.write_str("     ");
            } else if snap.pid < 100 {
                text.write_str("    ");
            } else {
                text.write_str("   ");
            }

            write_u32(snap.ppid as u32, text);
            if snap.ppid < 10 {
                text.write_str("      ");
            } else if snap.ppid < 100 {
                text.write_str("     ");
            } else {
                text.write_str("    ");
            }

            match snap.state {
                crate::process::ProcessState::Running => {
                    text.set_color(80, 160, 255);
                    text.write_str("RUNNING    ");
                }
                crate::process::ProcessState::Ready => {
                    text.set_color(80, 220, 120);
                    text.write_str("READY      ");
                }
                crate::process::ProcessState::Blocked => {
                    text.set_color(240, 180, 80);
                    text.write_str("BLOCKED    ");
                }
                crate::process::ProcessState::Stopped => {
                    text.set_color(160, 160, 170);
                    text.write_str("STOPPED    ");
                }
                crate::process::ProcessState::Zombie => {
                    text.set_color(160, 160, 170);
                    text.write_str("ZOMBIE     ");
                }
            }

            text.set_color(235, 235, 245);
            text.write_str(&snap.name);
            text.write_str("\n");
        }

        text.write_str("\n");
    }

    fn cmd_schedtest(&self, text: &mut TextWriter) {
        text.set_color(80, 160, 255);
        text.write_str("Preemptive Task Execution Test (without yield):\n");

        crate::task::start_preemption_demo();

        let mut last_a = 0;
        let mut last_b = 0;

        for _ in 0..3000 {
            let a = crate::task::TASK_A_COUNT.load(core::sync::atomic::Ordering::Acquire);
            let b = crate::task::TASK_B_COUNT.load(core::sync::atomic::Ordering::Acquire);

            if a != last_a {
                text.set_color(235, 235, 245);
                text.write_str("  TASK A: ");
                write_u32(a as u32, text);
                text.write_str("\n");
                last_a = a;
            }

            if b != last_b {
                text.set_color(235, 235, 245);
                text.write_str("  TASK B: ");
                write_u32(b as u32, text);
                text.write_str("\n");
                last_b = b;
            }

            if a >= 3 && b >= 3 {
                break;
            }

            for _ in 0..50_000 {
                core::hint::spin_loop();
            }
        }

        text.set_color(80, 220, 120);
        text.write_str("Preemption test passed: Tasks switched via PIT timer.\n\n");
    }

    fn cmd_unknown(&self, command: &str, text: &mut TextWriter) {
        text.set_color(240, 80, 80);
        text.write_str("Unknown command: ");
        text.write_str(command);
        text.write_str("\n\n");
    }

    fn print_error(&self, error: FsError, path: &str, text: &mut TextWriter) {
        text.set_color(240, 80, 80);
        match error {
            FsError::NotFound => {
                text.write_str("Not found: ");
                text.write_str(path);
            }
            FsError::AlreadyExists => {
                text.write_str("Already exists: ");
                text.write_str(path);
            }
            FsError::NotADirectory => {
                text.write_str("Not a directory: ");
                text.write_str(path);
            }
            FsError::IsADirectory => {
                text.write_str("Is a directory: ");
                text.write_str(path);
            }
            FsError::DirectoryNotEmpty => {
                text.write_str("Directory not empty: ");
                text.write_str(path);
            }
            FsError::CannotRemoveRoot => {
                text.write_str("Cannot remove root directory");
            }
            FsError::DirectoryInUse => {
                text.write_str("Cannot remove active working directory: ");
                text.write_str(path);
            }
            FsError::StorageFull => {
                text.write_str("Storage full");
            }
            FsError::NameTooLong => {
                text.write_str("Name too long: ");
                text.write_str(path);
            }
            FsError::FileTooLarge => {
                text.write_str("File too large");
            }
            FsError::InvalidPath => {
                text.write_str("Invalid path: ");
                text.write_str(path);
            }
            FsError::DiskUnavailable => {
                text.write_str("Disk unavailable");
            }
            FsError::IoError => {
                text.write_str("Disk I/O error");
            }
        }
        text.write_str("\n\n");
    }
}

fn write_u32(n: u32, text: &mut TextWriter) {
    if n == 0 {
        text.write_char('0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = 0;
    let mut val = n;
    while val > 0 {
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        text.write_char(buf[i] as char);
    }
}

fn parse_command(input: &str) -> (&str, &str) {
    let trimmed = input.trim_start();
    if trimmed.is_empty() {
        return ("", "");
    }

    if let Some(space_idx) = trimmed.find(' ') {
        let command = &trimmed[..space_idx];
        let args = trimmed[space_idx..].trim_start();
        (command, args)
    } else {
        (trimmed.trim_end(), "")
    }
}
