# BerkeOS Process Management

## Overview

BerkeOS implements a basic preemptive multitasking system with round-robin scheduling. The system supports up to 16 concurrent processes with kernel threads, context switching, and basic process lifecycle management.

## Process Model

### Process States

```rust
pub enum ProcessState {
    Empty,    // Unallocated slot
    Ready,    // Ready to run
    Running,  // Currently executing
    Blocked,  // Waiting for resource
    Zombie,   // Terminated, awaiting cleanup
}
```

### Process Control Block

Each process is represented by a PCB containing:

```rust
pub struct Process {
    pub pid: u32,              // Process ID
    pub state: ProcessState,   // Current state
    pub context: Context,      // Saved CPU registers
    pub name: [u8; 32],        // Process name
    pub name_len: usize,       // Name length
    pub exit_code: i32,        // Exit status
    pub ticks: u64,            // CPU time used
    pub stack_base: u64,       // Kernel stack address
}
```

### CPU Context

Saved register state for context switching:

```rust
#[repr(C)]
pub struct Context {
    pub rax: u64, rbx: u64, rcx: u64, rdx: u64,
    pub rsi: u64, rdi: u64, rbp: u64,
    pub r8: u64, r9: u64, r10: u64, r11: u64,
    pub r12: u64, r13: u64, r14: u64, r15: u64,
    pub rip: u64, rsp: u64, rflags: u64,
}
```

## Scheduler

### Round-Robin Scheduling

- **Time Slice**: 10ms (100 Hz timer interrupts)
- **Preemptive**: Timer interrupt triggers context switch
- **Fair Sharing**: Equal CPU time allocation
- **No Priorities**: All processes equal priority

### Scheduler Operations

```rust
// Called from PIT timer interrupt (100 Hz)
pub fn tick() {
    // Update current process CPU time
    // Check if reschedule needed
}

// Voluntary yield
pub fn schedule() {
    // Find next ready process
    // Perform context switch
}
```

### Process Table

Global table managing all processes:

```rust
pub struct ProcessTable {
    pub procs: [Process; MAX_PROCESSES],  // 16 processes max
    pub current: usize,                    // Running process index
    pub count: usize,                      // Active process count
}
```

## Process Lifecycle

### Process Creation

```rust
pub fn create_kernel_thread(&mut self, name: &[u8], entry: u64, stack: u64) -> Option<u32>
```

- Allocates free process slot
- Assigns unique PID
- Initializes stack and context
- Sets initial state to Ready

### Process Termination

```rust
pub fn kill(&mut self, pid: u32, exit_code: i32) -> bool
```

- Changes state to Zombie
- Preserves exit code
- Decrements active count

### Process Cleanup

```rust
pub fn reap(&mut self, pid: u32) -> Option<i32>
```

- Removes zombie processes
- Returns exit code to parent
- Frees process slot

## Context Switching

### Interrupt-Driven Switching

1. **Timer Interrupt**: PIT fires every 10ms
2. **Save Context**: IRQ handler saves current registers
3. **Scheduler Call**: `tick()` updates process state
4. **Select Next**: Find next ready process
5. **Restore Context**: Load new process registers
6. **Return**: `iretq` resumes execution

### Kernel Stacks

- **Size**: 8 KiB per process
- **Location**: Static BSS allocation
- **Management**: Separate stack per process
- **Safety**: Stack overflow protection

## System Calls

### Process-Related Syscalls

- **getpid()**: Return current process ID
- **fork()**: Create child process (future)
- **exit()**: Terminate current process
- **wait()**: Wait for child termination (future)

### Syscall Interface

```rust
pub enum Syscall {
    GetPid = 3,
    // ... other syscalls
}
```

## Limitations

- **Kernel Threads Only**: No user-space processes
- **No Fork/Exec**: Process creation limited
- **Fixed Stack Size**: 8 KiB kernel stacks
- **No Virtual Memory**: All processes share address space
- **No IPC**: No inter-process communication
- **Basic Scheduling**: No priority or time slicing control

## Performance

- **Context Switch Time**: Minimal register save/restore
- **Memory Overhead**: ~200 bytes per process
- **Scalability**: Supports up to 16 concurrent processes
- **Deterministic**: Fixed 10ms scheduling quantum

## Future Enhancements

- **User Processes**: Separate address spaces
- **ELF Loading**: Executable file support
- **Process Groups**: Job control
- **Signals**: Asynchronous notifications
- **Priority Scheduling**: Multi-level queues
- **Real-time Support**: Guaranteed latency

---
Note: This documentation may not always be up-to-date or may contain inaccuracies.