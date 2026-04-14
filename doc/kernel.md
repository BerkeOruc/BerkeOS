# BerkeOS Kernel Architecture

## Overview

BerkeOS implements a monolithic kernel architecture written entirely in Rust with `#![no_std]`. The kernel provides core OS services including interrupt handling, memory management, process scheduling, and system calls, all running in kernel space for simplicity and performance.

## Core Components

### Interrupt Handling

The kernel uses the x86_64 Interrupt Descriptor Table (IDT) with 256 entries to handle CPU exceptions and hardware interrupts.

#### IDT Structure

```rust
#[repr(C, packed)]
pub struct IdtEntry {
    offset_lo: u16,    // Lower 16 bits of handler address
    selector: u16,     // Code segment selector (0x08)
    ist: u8,           // Interrupt Stack Table (0)
    type_attr: u8,     // Type and attributes (0x8E for interrupts)
    offset_mid: u16,   // Middle 16 bits of handler address
    offset_hi: u32,    // Upper 32 bits of handler address
    zero: u32,         // Reserved (0)
}
```

#### Exception Handlers

The kernel implements handlers for all x86_64 CPU exceptions (0-31):

- **#DE (0)**: Divide by Zero
- **#DB (1)**: Debug
- **#BP (3)**: Breakpoint
- **#UD (6)**: Invalid Opcode
- **#DF (8)**: Double Fault
- **#GP (13)**: General Protection Fault
- **#PF (14)**: Page Fault

Exceptions display error messages on VGA and halt the system.

#### PIC and IRQs

Hardware interrupts (IRQs) are managed through dual Intel 8259 PICs:

- **PIC Remapping**: IRQs 0-15 remapped to interrupts 32-47
- **IRQ0**: Timer (100 Hz) - triggers scheduler
- **IRQ1**: Keyboard - buffered scancode handling
- **EOI**: End-of-Interrupt signaling to PICs

```rust
// PIC initialization sequence
unsafe fn init() {
    // ICW1: Initialize command
    outb(PIC1_CMD, ICW1_INIT);
    outb(PIC2_CMD, ICW1_INIT);
    
    // ICW2: Remap interrupts
    outb(PIC1_DATA, PIC1_OFFSET);  // 32
    outb(PIC2_DATA, PIC2_OFFSET);  // 40
    
    // ICW3: Cascade configuration
    outb(PIC1_DATA, 0x04);  // Master
    outb(PIC2_DATA, 0x02);  // Slave
    
    // ICW4: 8086 mode
    outb(PIC1_DATA, ICW4_8086);
    outb(PIC2_DATA, ICW4_8086);
}
```

### Timer System (PIT)

The Programmable Interval Timer provides system timing:

- **Base Frequency**: 1,193,182 Hz
- **Configured Rate**: 100 Hz (10ms ticks)
- **Channel 0**: IRQ0 generation
- **Divisor Calculation**: `1193182 / hz`

```rust
pub unsafe fn init(hz: u32) {
    let divisor = 1193182u32 / hz;
    outb(PIT_CMD, 0x36);        // Channel 0, lo/hi, mode 3
    outb(PIT_CHANNEL0, (divisor & 0xFF) as u8);
    outb(PIT_CHANNEL0, ((divisor >> 8) & 0xFF) as u8);
}
```

### Process Management

#### Scheduler

BerkeOS implements a basic round-robin preemptive scheduler:

- **Tick Rate**: 100 Hz (every 10ms)
- **Process Table**: Fixed array of 16 processes
- **Context Switching**: Saves/restores CPU registers
- **Kernel Stacks**: 8 KiB per process in BSS

```rust
pub static PTABLE: Mutex<ProcessTable> = Mutex::new(ProcessTable::new());
pub static SCHEDULER_ENABLED: AtomicBool = AtomicBool::new(false);
```

#### Process States

```rust
pub enum ProcessState {
    Unused,
    Runnable,
    Running,
    Sleeping,
    Zombie,
}
```

### Memory Management

#### Virtual Memory

- **Page Size**: 4 KiB pages with 2 MiB huge page support
- **Kernel Space**: `0xFFFF800000000000` and above
- **User Space**: `0x0000000000400000` and below
- **Identity Mapping**: First 4 GiB physical memory

#### Heap Allocator

Custom kernel allocator using linked list of free blocks:

```rust
#[global_allocator]
static KERNEL_ALLOCATOR: KernelAllocator = KernelAllocator::new();
```

### System Calls

The syscall interface allows user programs (BexVM) to request kernel services:

```rust
pub enum Syscall {
    Read = 0,
    Write = 1,
    Open = 2,
    Close = 3,
    // ... more syscalls
}
```

Syscalls are invoked via `syscall` instruction with parameters in registers.

### Kernel Initialization Sequence

1. **Serial Init**: COM1 for logging
2. **VGA Probe**: Hardware detection
3. **IDT Setup**: Exception and IRQ handlers
4. **PIC Init**: Interrupt controller configuration
5. **PIT Init**: 100 Hz timer
6. **Scheduler Init**: Process management
7. **PIC Enable**: Interrupts activated
8. **Storage Detection**: ATA/AHCI drives
9. **Filesystem Mount**: BerkeFS initialization
10. **Shell Launch**: berkesh interactive shell

### Synchronization

The kernel uses spin-based mutexes for thread safety:

```rust
use spin::Mutex;

// Global state protection
static VGA_BUFFER: Mutex<VgaBuffer> = Mutex::new(VgaBuffer::new());
```

### Error Handling

- **Panic Handler**: Serial output with stack dump
- **Exception Handlers**: VGA error display and halt
- **Logging**: `kinfo!`, `kwarn!`, `kerr!` macros

### Kernel Stack

- **Size**: 64 KiB in assembly BSS
- **Location**: Fixed address below kernel code
- **Usage**: Interrupt handlers and kernel functions

## Design Principles

- **Minimalism**: Only essential features for OS education
- **Safety**: Rust's ownership system prevents memory corruption
- **Performance**: Monolithic design reduces context switches
- **Hardware Control**: Direct hardware access without abstractions
- **Debugging**: Serial logging and VGA error display

---
Note: This documentation may not always be up-to-date or may contain inaccuracies.