// BerkeOS — process.rs
// Process Control Block + Process Table
// Supports up to 16 concurrent processes

use core::sync::atomic::{AtomicU32, Ordering};

pub const MAX_PROCESSES: usize = 16;
pub const KERNEL_STACK_SIZE: usize = 8192; // 8 KiB per process

static NEXT_PID: AtomicU32 = AtomicU32::new(1);

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ProcessState {
    Empty,
    Ready,
    Running,
    Blocked,
    Zombie,
}

// Saved CPU context for context switching
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Context {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    pub rsp: u64,
    pub rflags: u64,
}

impl Context {
    pub const fn zero() -> Self {
        Context {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rip: 0,
            rsp: 0,
            rflags: 0x202, // IF set
        }
    }
}

// Process Control Block
#[derive(Copy, Clone)]
pub struct Process {
    pub pid: u32,
    pub state: ProcessState,
    pub context: Context,
    pub name: [u8; 32],
    pub name_len: usize,
    pub exit_code: i32,
    pub ticks: u64,      // CPU time used
    pub stack_base: u64, // kernel stack base
}

impl Process {
    pub const fn empty() -> Self {
        Process {
            pid: 0,
            state: ProcessState::Empty,
            context: Context::zero(),
            name: [0u8; 32],
            name_len: 0,
            exit_code: 0,
            ticks: 0,
            stack_base: 0,
        }
    }

    pub fn get_name(&self) -> &[u8] {
        &self.name[..self.name_len]
    }

    pub fn set_name(&mut self, name: &[u8]) {
        let n = name.len().min(31);
        self.name[..n].copy_from_slice(&name[..n]);
        self.name_len = n;
    }
}

// Global process table — static to avoid heap
pub struct ProcessTable {
    pub procs: [Process; MAX_PROCESSES],
    pub current: usize, // index of running process
    pub count: usize,   // total active processes
}

impl ProcessTable {
    pub const fn new() -> Self {
        ProcessTable {
            procs: [Process::empty(); MAX_PROCESSES],
            current: 0,
            count: 0,
        }
    }

    // Allocate a new PID
    pub fn alloc_pid() -> u32 {
        NEXT_PID.fetch_add(1, Ordering::Relaxed)
    }

    // Find a free slot
    pub fn alloc_slot(&mut self) -> Option<usize> {
        for i in 0..MAX_PROCESSES {
            if self.procs[i].state == ProcessState::Empty {
                return Some(i);
            }
        }
        None
    }

    // Create a kernel thread
    pub fn create_kernel_thread(&mut self, name: &[u8], entry: u64, stack: u64) -> Option<u32> {
        let slot = self.alloc_slot()?;
        let pid = Self::alloc_pid();

        self.procs[slot].pid = pid;
        self.procs[slot].state = ProcessState::Ready;
        self.procs[slot].exit_code = 0;
        self.procs[slot].ticks = 0;
        self.procs[slot].stack_base = stack;
        self.procs[slot].set_name(name);

        self.procs[slot].context = Context::zero();
        self.procs[slot].context.rip = entry;
        self.procs[slot].context.rsp = stack + KERNEL_STACK_SIZE as u64 - 8;
        self.procs[slot].context.rflags = 0x202; // IF=1

        self.count += 1;
        Some(pid)
    }

    // Get current running process
    pub fn current_proc(&self) -> &Process {
        &self.procs[self.current]
    }

    // Get current running process mutably
    pub fn current_proc_mut(&mut self) -> &mut Process {
        &mut self.procs[self.current]
    }

    // Find process by PID
    pub fn find_pid(&self, pid: u32) -> Option<usize> {
        for i in 0..MAX_PROCESSES {
            if self.procs[i].state != ProcessState::Empty && self.procs[i].pid == pid {
                return Some(i);
            }
        }
        None
    }

    // Kill a process
    pub fn kill(&mut self, pid: u32, exit_code: i32) -> bool {
        if let Some(i) = self.find_pid(pid) {
            self.procs[i].state = ProcessState::Zombie;
            self.procs[i].exit_code = exit_code;
            self.count = self.count.saturating_sub(1);
            true
        } else {
            false
        }
    }

    // Reap zombie processes
    pub fn reap(&mut self, pid: u32) -> Option<i32> {
        if let Some(i) = self.find_pid(pid) {
            if self.procs[i].state == ProcessState::Zombie {
                let code = self.procs[i].exit_code;
                self.procs[i] = Process::empty();
                return Some(code);
            }
        }
        None
    }

    // Round-robin: find next ready process
    pub fn next_ready(&self, from: usize) -> Option<usize> {
        let n = MAX_PROCESSES;
        for i in 1..=n {
            let idx = (from + i) % n;
            if self.procs[idx].state == ProcessState::Ready {
                return Some(idx);
            }
        }
        None
    }

    // List all processes
    pub fn list(&self, out: &mut [(u32, ProcessState, [u8; 32], usize, u64); MAX_PROCESSES]) {
        for i in 0..MAX_PROCESSES {
            out[i] = (
                self.procs[i].pid,
                self.procs[i].state,
                self.procs[i].name,
                self.procs[i].name_len,
                self.procs[i].ticks,
            );
        }
    }
}
