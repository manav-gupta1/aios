use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use x86_64::structures::paging::{PageTableFlags};

use crate::process::process::{FileDescriptor, Process, ProcessAddressSpace, ProcessState};
use crate::process::table::ProcessTable;
use crate::task::task::{ContextFrame, Task, TaskId, TaskKind, TaskState, TASK_STACK_SIZE};

pub fn do_fork(
    table: &mut ProcessTable,
    parent_pid: usize,
    frame_ptr: *mut u64,
) -> Result<usize, &'static str> {
    // 1. Locate parent process
    let parent = table
        .processes
        .iter_mut()
        .find(|p| p.pid == parent_pid)
        .ok_or("Parent process not found")?;

    // 2. Allocate new PID for child
    let child_pid = table.pid_allocator.allocate();

    // 3. Duplicate parent's address space using Copy-on-Write
    let mut child_address_space = ProcessAddressSpace::new();

    let parent_pages = parent.address_space.pages.clone();

    for page in parent_pages {
        let frame = match crate::memory::translate_page(page) {
            Some(f) => f,
            None => continue,
        };

        let page_addr = page.start_address().as_u64() as usize;
        let mut is_shared = false;
        let mut shared_prot = 0;
        
        for r in &parent.address_space.mmap_regions {
            if r.start <= page_addr && page_addr < r.start + r.length {
                if (r.flags & crate::memory::mmap::MAP_SHARED) != 0 {
                    is_shared = true;
                    shared_prot = r.prot;
                }
                break;
            }
        }

        if is_shared {
            let mut shared_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
            if (shared_prot & crate::memory::mmap::PROT_WRITE) != 0 {
                shared_flags |= PageTableFlags::WRITABLE;
            }
            if (shared_prot & crate::memory::mmap::PROT_EXEC) == 0 {
                shared_flags |= PageTableFlags::NO_EXECUTE;
            }
            
            crate::memory::inc_frame_ref(frame);
            let _ = crate::memory::map_page_to_frame(page, frame, shared_flags);
            child_address_space.add_page(page);
        } else {
            // For all user pages, ensure both parent and child map it as read-only COW
            let ro_flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;

            // Clear WRITABLE flag in parent's page table mapping
            let _ = crate::memory::update_page_flags(page, ro_flags);
            parent.address_space.mark_cow(page);

            // Increment reference count of shared physical frame
            crate::memory::inc_frame_ref(frame);

            // Map same frame to child with read-only flags
            let _ = crate::memory::map_page_to_frame(page, frame, ro_flags);
            child_address_space.add_page(page);
            child_address_space.mark_cow(page);
        }
    }

    // 4. Duplicate file descriptor table
    let mut child_fd_table = Vec::with_capacity(parent.fd_table.len());
    for fd_opt in &parent.fd_table {
        if let Some(desc) = fd_opt {
            match desc {
                FileDescriptor::PipeRead(pipe) => {
                    pipe.lock().add_reader();
                    child_fd_table.push(Some(FileDescriptor::PipeRead(pipe.clone())));
                }
                FileDescriptor::PipeWrite(pipe) => {
                    pipe.lock().add_writer();
                    child_fd_table.push(Some(FileDescriptor::PipeWrite(pipe.clone())));
                }
                FileDescriptor::Stdin => child_fd_table.push(Some(FileDescriptor::Stdin)),
                FileDescriptor::Stdout => child_fd_table.push(Some(FileDescriptor::Stdout)),
                FileDescriptor::Stderr => child_fd_table.push(Some(FileDescriptor::Stderr)),
                FileDescriptor::File(path, offset) => {
                    child_fd_table.push(Some(FileDescriptor::File(path.clone(), *offset)));
                }
                FileDescriptor::Socket(id) => {
                    child_fd_table.push(Some(FileDescriptor::Socket(*id)));
                }
            }
        } else {
            child_fd_table.push(None);
        }
    }

    // 5. Create child kernel task and copy register frame from parent's frame_ptr
    let child_stack = Box::new([0u8; TASK_STACK_SIZE]);
    let stack_top = (child_stack.as_ptr() as usize + TASK_STACK_SIZE) & !0xF;
    let frame_size = core::mem::size_of::<ContextFrame>();
    let child_frame_ptr = (stack_top - frame_size) as *mut ContextFrame;

    if !frame_ptr.is_null() {
        unsafe {
            // Copy saved context from parent
            let parent_frame = *(frame_ptr as *const ContextFrame);
            let mut child_frame = parent_frame;
            // Child returns 0 from fork()!
            child_frame.rax = 0;

            let parent_user_rsp = parent_frame.rsp;
            if parent_user_rsp >= crate::elf::loader::USER_STACK_BOTTOM + 0x2000 {
                let child_user_rsp = parent_user_rsp - 0x2000;
                child_frame.rsp = child_user_rsp;

                let parent_page = x86_64::structures::paging::Page::<x86_64::structures::paging::Size4KiB>::containing_address(x86_64::VirtAddr::new(parent_user_rsp));
                let child_page = x86_64::structures::paging::Page::<x86_64::structures::paging::Size4KiB>::containing_address(x86_64::VirtAddr::new(child_user_rsp));
                crate::memory::copy_page_physical(parent_page, child_page);
            }

            core::ptr::write(child_frame_ptr, child_frame);
        }
    }

    let child_name: &'static str = alloc::boxed::Box::leak(
        alloc::format!("{}-child", parent.name).into_boxed_str(),
    );

    let child_task = Task {
        id: TaskId::new(),
        process_id: child_pid,
        name: child_name,
        state: TaskState::Ready,
        kind: TaskKind::User,
        rsp: child_frame_ptr as usize,
        ticks: 0,
        stack: Some(child_stack),
        is_stopped: false,
    };

    let child_task_id = child_task.id.0;
    crate::task::add_task(child_task);

    // 6. Create child Process structure
    let child_process = Process {
        pid: child_pid,
        ppid: parent_pid,
        pgid: parent.pgid,
        sid: parent.sid,
        state: ProcessState::Ready,
        name: String::from(parent.name.as_str()),
        main_task_id: child_task_id,
        exit_status: None,
        address_space: child_address_space,
        waiting_target_pid: None,
        fd_table: child_fd_table,
        pending_signals: 0,
        blocked_signals: parent.blocked_signals,
        sig_actions: parent.sig_actions,
        is_stopped: false,
        is_orphan: false,
    };

    table.processes.push(child_process);

    Ok(child_pid)
}
