# BerkeOS Boot Process

## Overview

The BerkeOS boot process follows a multi-stage approach starting from BIOS/UEFI firmware through GRUB bootloader, assembly bootstrap code, and finally the Rust kernel. The system supports both UEFI and legacy BIOS boot with automatic detection.

## Boot Stages

### Stage 1: Firmware (BIOS/UEFI)

- **UEFI/BIOS**: Firmware initializes hardware and loads the bootloader
- **GRUB**: Multiboot2-compliant bootloader loads the kernel image
- **Multiboot2 Header**: Located in `boot.asm`, specifies kernel requirements including preferred framebuffer resolution (1920x1080x32)

### Stage 2: Assembly Bootstrap (`boot.asm`)

The NASM assembly code handles the transition from 32-bit protected mode to 64-bit Long Mode.

#### Multiboot2 Header

```nasm
header_start:
    dd 0xe85250d6                   ; Multiboot2 magic number
    dd 0                            ; Architecture: i386 protected mode
    dd header_end - header_start    ; Header length
    dd 0x100000000 - (0xe85250d6 + 0 + (header_end - header_start))

    ; Framebuffer request tag (type 5)
    align 8
    dw 5            ; Tag type = framebuffer
    dw 0            ; Flags = 0
    dd 20           ; Size = 20 bytes
    dd 1920         ; Preferred width
    dd 1080         ; Preferred height
    dd 32           ; Preferred bits per pixel
```

#### Memory Layout Setup

The bootstrap sets up 4-level page tables for identity mapping the first 4 GiB of physical memory using 2 MiB huge pages:

- **PML4 (P4)**: Top-level page table
- **PDPT (P3)**: Page Directory Pointer Table with 4 entries
- **PD (P2)**: Four Page Directory tables, each covering 1 GiB
- **Stack**: 64 KiB stack in BSS section

#### Page Table Initialization

```nasm
setup_page_tables:
    ; P4[0] → p3_table
    mov  eax, p3_table
    or   eax, 0x03          ; Present + Writable
    mov  [p4_table], eax

    ; Fill P2 tables with 2 MiB huge page entries
    ; Each entry: physical_address | 0x83 (Present + Writable + Huge Page)
```

#### Long Mode Transition

1. **Enable PAE**: Set CR4.PAE bit for Physical Address Extension
2. **Enable Long Mode**: Set EFER.LME bit in MSR 0xC0000080
3. **Enable Paging**: Set CR0.PG bit to activate Long Mode
4. **Load GDT**: 64-bit Global Descriptor Table with code segment
5. **Far Jump**: `jmp 0x08:long_mode_start` to 64-bit code

### Stage 3: Rust Kernel Entry (`kernel_main`)

The `kernel_main` function in `src/lib.rs` takes over from assembly:

```rust
pub extern "C" fn kernel_main(mb2_info_ptr: u32) -> ! {
    // Initialize serial port for logging
    serial::init();
    
    // Parse Multiboot2 framebuffer information
    let fb_info = unsafe { parse_mb2_framebuffer(mb2_info_ptr) };
    
    // Initialize core subsystems...
}
```

#### Initialization Sequence

1. **Serial Port**: COM1 (115200 8N1) for kernel logging
2. **VGA Probe**: Detect VGA hardware availability
3. **Multiboot2 Parsing**: Extract framebuffer information
4. **Interrupt Setup**: IDT, PIC 8259, PIT timer (100 Hz)
5. **Scheduler**: Initialize process scheduler
6. **Storage Detection**: ATA PIO and AHCI SATA drives
7. **Filesystem**: Mount BerkeFS on detected drives
8. **Shell**: Launch berkesh interactive shell

## Boot Information

The `BootInfo` struct tracks boot parameters:

```rust
pub struct BootInfo {
    pub boot_disk: u8,        // BIOS boot disk number
    pub boot_device: u32,     // Boot device identifier
    pub kernel_loaded: bool,  // Kernel load status
    pub multiboot_magic: u32, // Multiboot2 magic number
}
```

## Error Handling

- **Magic Number Check**: Validates Multiboot2 compliance
- **VGA Detection**: Graceful fallback when VGA hardware unavailable
- **UEFI Compatibility**: Avoids VGA writes that cause faults on UEFI systems

## Build Process

The boot process is compiled as part of the kernel build pipeline:

```
1. NASM: boot.asm → boot.o (32-bit ELF)
2. Cargo: Rust code → libkernelos.a (static library)
3. LD: boot.o + libkernelos.a → kernel.bin (linked binary)
4. GRUB: kernel.bin → bootable ISO via grub-mkrescue
```

## Memory Map

After boot, the system has identity-mapped the first 4 GiB of physical memory with 2 MiB huge pages, providing a flat memory model for the kernel while maintaining compatibility with x86_64 paging requirements.

---
Note: This documentation may not always be up-to-date or may contain inaccuracies.