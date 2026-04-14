# BerkeOS Overview

## Introduction

BerkeOS is a modern, DOS-inspired operating system developed entirely from scratch using Rust. It implements a monolithic kernel architecture running in `no_std` environment, targeting x86_64 bare metal hardware. The system features a complete boot chain, custom filesystem (BerkeFS), interactive shell (berkesh), device drivers, and basic process scheduling.

## Architecture Overview

### Kernel Design

BerkeOS uses a **monolithic kernel** architecture where all core OS services run in kernel space. The kernel is written in Rust with `#![no_std]` and `#![no_main]`, providing memory safety guarantees and preventing common vulnerabilities.

Key architectural components:

- **Boot Process**: UEFI/BIOS auto-detection with NASM assembly bootstrap
- **Memory Management**: 2 MiB huge pages with custom heap allocator
- **Interrupt Handling**: IDT + PIC 8259 + PIT 100Hz timer
- **Process Management**: Round-robin preemptive scheduler
- **File System**: Custom BerkeFS supporting up to 12 drives
- **Device Drivers**: ATA PIO, AHCI SATA, PS/2 keyboard, RTC, PC speaker
- **System Calls**: Interface for user-space programs (via BexVM)

### Project Structure

```
BerkeOS/
├── src/
│   ├── lib.rs                    # Kernel entry point (kernel_main)
│   ├── boot/                     # Assembly bootstrap code
│   ├── drivers/                  # Hardware device drivers
│   ├── memory/                   # Memory management (paging, allocator)
│   ├── process/                  # Process scheduling and management
│   ├── syscall/                  # System call interface
│   ├── fs/                       # Filesystem implementation
│   ├── ui/                       # User interface (shell, editor)
│   └── vm/                       # BexVM runtime for .bex programs
├── berkebex/                     # Python-to-bytecode compiler
└── doc/                          # Documentation
```

### Memory Layout

The system uses a 64-bit virtual address space with:

- **Kernel Space**: `0xFFFF800000000000` and above
- **User Space**: `0x0000000000400000` and below
- **Page Size**: 4 KiB pages with support for 2 MiB huge pages

### Boot Sequence

1. **BIOS/UEFI**: Loads GRUB bootloader
2. **GRUB**: Loads `kernel.bin` and Multiboot2 information
3. **boot.asm**: Sets up 32-bit protected mode, page tables, enables Long Mode
4. **kernel_main()**: Initializes hardware, mounts filesystem, starts shell

### Key Design Decisions

- **Rust no_std**: Ensures no runtime dependencies, full control over memory
- **Monolithic Kernel**: Simpler development, better performance for small system
- **Custom Filesystem**: Optimized for embedded use cases, supports multiple drives
- **Minimal Hardware Requirements**: Runs on basic x86_64 hardware
- **DOS-inspired UI**: Familiar command-line interface for accessibility

### Supported Hardware

- **CPU**: x86_64 with Long Mode support
- **Storage**: ATA PIO and AHCI SATA drives
- **Input**: PS/2 keyboard
- **Display**: VGA text mode or graphical framebuffer
- **Audio**: PC speaker (beep and melodies)
- **RTC**: Real-time clock for date/time

### Development Environment

- **Language**: Rust nightly with custom target (`x86_64-berkeos.json`)
- **Build System**: Cargo + NASM + LD + GRUB
- **Testing**: QEMU emulator with automated test scripts
- **Cross-compilation**: Host builds kernel for bare metal target

---
Note: This documentation may not always be up-to-date or may contain inaccuracies.