# BerkeOS Hardware Drivers

## Overview

BerkeOS includes a collection of hardware drivers for essential peripherals, focusing on x86_64 PC hardware. Drivers are written in Rust with direct hardware access using I/O ports and memory-mapped registers.

## Storage Drivers

### ATA PIO Driver (`ata.rs`)

The ATA driver implements PIO (Programmed I/O) mode for IDE hard disks:

#### Hardware Interface

- **Primary Channel**: Ports 0x1F0-0x1F7, 0x3F6
- **Data Port**: 0x1F0 (16-bit word transfers)
- **Command/Status**: 0x1F7 (shared register)
- **LBA Addressing**: 28-bit (up to 128 GiB per drive)

#### Key Functions

```rust
// Sector read/write operations
pub unsafe fn read_sector(drive_id: u8, lba: u32, buf: &mut [u8; 512]) -> bool
pub unsafe fn write_sector(drive_id: u8, lba: u32, buf: &[u8; 512]) -> bool

// Drive detection
pub unsafe fn ata_detect() -> bool
```

#### Drive Support

- **Alpha Drive**: Primary master (IDE0, register 0xA0)
- **Beta Drive**: Secondary master (IDE1, register 0xB0)
- **ATA Commands**: IDENTIFY, READ_SECTORS, WRITE_SECTORS, FLUSH_CACHE

#### Status Polling

```rust
const ATA_STATUS_BSY: u8 = 0x80;  // Busy - wait for clear
const ATA_STATUS_DRQ: u8 = 0x08;  // Data Request - ready for transfer
const ATA_STATUS_ERR: u8 = 0x01;  // Error occurred
const ATA_STATUS_RDY: u8 = 0x40;  // Drive ready
```

### AHCI SATA Driver (`ahci.rs`)

Experimental SATA controller driver using AHCI (Advanced Host Controller Interface):

- **AHCI Detection**: Scans PCI for SATA controllers
- **Port Initialization**: Sets up command lists and FIS buffers
- **Status**: Early experimental stage

## Input/Output Drivers

### VGA Text Mode (`vga.rs`)

80×25 character text display driver:

#### Memory Layout

- **Base Address**: 0xB8000 (physical)
- **Cell Format**: [character byte, attribute byte]
- **Attribute**: (background << 4) | foreground

#### Color Palette

```rust
pub enum Color {
    Black = 0, Blue = 1, Green = 2, Cyan = 3,
    Red = 4, Magenta = 5, Brown = 6, LightGray = 7,
    DarkGray = 8, LightBlue = 9, LightGreen = 10,
    LightCyan = 11, LightRed = 12, Pink = 13,
    Yellow = 14, White = 15,
}
```

#### Core Functions

```rust
impl Vga {
    pub fn print_at(&self, col: usize, row: usize, s: &str, fg: Color, bg: Color)
    pub fn clear(&self, bg: Color)
    pub fn fill_row(&self, row: usize, bg: Color)
}
```

### Framebuffer Graphics (`framebuffer.rs`)

Graphical framebuffer driver for high-resolution display:

- **Multiboot2 Info**: Parses framebuffer details from bootloader
- **Font Rendering**: Built-in bitmap font
- **Color Support**: 32-bit RGBA
- **Resolution**: Configurable (default 1920×1080)

### PS/2 Keyboard (`keyboard.rs`)

Scancode-to-character conversion with modifier support:

#### Key Mapping

- **Modifiers**: Shift, Caps Lock, Ctrl
- **Special Keys**: Arrow keys, function keys, escape
- **Control Sequences**: Ctrl+C, Ctrl+A, etc.

#### Scancode Processing

```rust
pub enum Key {
    Char(u8), Up, Down, Left, Right, Delete,
    F1, F2, Escape, CtrlC, CtrlA, /* ... */
}
```

## Peripheral Drivers

### Real-Time Clock (`rtc.rs`)

CMOS RTC driver for date/time services:

#### CMOS Interface

- **Address Port**: 0x70
- **Data Port**: 0x71
- **BCD Conversion**: Hardware stores BCD, converted to binary

#### Date/Time Structure

```rust
pub struct DateTime {
    pub second: u8, minute: u8, hour: u8,
    pub day: u8, month: u8, year: u16,
}
```

#### Update Protection

```rust
// Wait for RTC update completion
while update_in_progress() {
    // Prevent reading during update
}
```

### PC Speaker (`pcspeaker.rs`, `audio.rs`)

Audio output via programmable PC speaker:

- **PIT Channel 2**: Frequency generation
- **Waveforms**: Square wave output
- **Melodies**: Support for musical note sequences

#### Audio Interface

```rust
pub fn play_note(frequency: u32, duration_ms: u32)
pub fn beep()  // Simple beep
pub fn play_melody(notes: &[(u32, u32)])  // Note sequence
```

## Serial Port (`serial.rs`)

COM1 serial communication for debugging:

- **Port**: 0x3F8 (COM1)
- **Baud Rate**: 115,200
- **Format**: 8N1 (8 data, no parity, 1 stop)
- **Kernel Logging**: `kinfo!`, `kwarn!`, `kerr!` macros

## Experimental Drivers

### USB Stack (`usb/`)

Early-stage USB host controller drivers:

- **OHCI**: Open Host Controller Interface
- **Mass Storage**: USB storage device support
- **Status**: Experimental, not fully functional

### Network (`rtl8139.rs`, `net/`)

RTL8139 network card driver:

- **PCI Detection**: Scans for RTL8139 devices
- **Buffer Management**: Transmit/receive buffers
- **IPv4/ARP**: Basic network protocol support
- **Status**: Experimental, early development

## Driver Architecture

### Common Patterns

- **I/O Helpers**: `inb()`, `outb()`, `inw()`, `outw()` for port access
- **Status Polling**: Busy-wait loops with timeouts
- **Error Handling**: Status bit checking and timeout detection
- **Memory Safety**: Unsafe blocks for hardware access, safe interfaces

### Initialization Order

1. **Serial**: Early logging setup
2. **VGA/Framebuffer**: Display initialization
3. **IDT/PIC/PIT**: Interrupt system
4. **ATA/AHCI**: Storage detection
5. **RTC**: Time services
6. **Keyboard**: Input handling

### Hardware Detection

- **ATA**: IDENTIFY command for drive presence
- **AHCI**: PCI scanning for SATA controllers
- **VGA**: Memory probe at 0xB8000
- **RTC**: CMOS register reading

## Limitations

- **No DMA**: PIO-only transfers (slower)
- **No Interrupts**: Polling-based I/O
- **Limited Hardware**: x86_64 PC-specific
- **No Hotplug**: Static hardware detection
- **Experimental**: USB and network drivers incomplete

---
Note: This documentation may not always be up-to-date or may contain inaccuracies.