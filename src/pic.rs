// BerkeOS — pic.rs
// Intel 8259 PIC driver + IRQ handlers
// Remaps IRQ0-15 to interrupts 32-47 (above CPU exceptions 0-31)

use core::arch::{asm, naked_asm};
use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use spin::Mutex;

// ── PIC I/O ports ─────────────────────────────────────────────────────────────
const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const PIC_EOI: u8 = 0x20;
const ICW1_INIT: u8 = 0x11;
const ICW4_8086: u8 = 0x01;

const PIC1_OFFSET: u8 = 32;
const PIC2_OFFSET: u8 = 40;

// ── Global tick counter ───────────────────────────────────────────────────────
pub static TICKS: AtomicU64 = AtomicU64::new(0);

// ── Keyboard scancode ring buffer ─────────────────────────────────────────────
const KB_BUF_SIZE: usize = 64;
// Raw buffer for assembly access (assembly needs direct memory address)
static KB_BUF_RAW: [u8; KB_BUF_SIZE] = [0; KB_BUF_SIZE];
// Mutex-wrapped buffer for safe Rust access
static KB_BUF: Mutex<[u8; KB_BUF_SIZE]> = Mutex::new([0; KB_BUF_SIZE]);
static KB_HEAD: AtomicU8 = AtomicU8::new(0);
static KB_TAIL: AtomicU8 = AtomicU8::new(0);

// ── I/O helpers ───────────────────────────────────────────────────────────────
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack));
    val
}

#[inline]
pub unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
}

#[inline]
unsafe fn io_wait() {
    outb(0x80, 0);
}

// ── Initialize and remap both PICs ────────────────────────────────────────────
pub unsafe fn init() {
    let mask1 = inb(PIC1_DATA);
    let mask2 = inb(PIC2_DATA);

    outb(PIC1_CMD, ICW1_INIT);
    io_wait();
    outb(PIC2_CMD, ICW1_INIT);
    io_wait();
    outb(PIC1_DATA, PIC1_OFFSET);
    io_wait();
    outb(PIC2_DATA, PIC2_OFFSET);
    io_wait();
    outb(PIC1_DATA, 0x04);
    io_wait();
    outb(PIC2_DATA, 0x02);
    io_wait();
    outb(PIC1_DATA, ICW4_8086);
    io_wait();
    outb(PIC2_DATA, ICW4_8086);
    io_wait();

    // Mask everything except IRQ0 (timer)
    // IRQ1 (keyboard) stays MASKED — we poll port 0x60 directly
    // This prevents the race condition between IRQ1 handler and polling
    outb(PIC1_DATA, 0b11111110); // Only IRQ0 unmasked
    outb(PIC2_DATA, 0xFF); // All slave IRQs masked

    let _ = mask1;
    let _ = mask2;
}

#[inline]
pub unsafe fn eoi_master() {
    outb(PIC1_CMD, PIC_EOI);
}

#[inline]
pub unsafe fn eoi_slave() {
    outb(PIC2_CMD, PIC_EOI);
    outb(PIC1_CMD, PIC_EOI);
}

// ── IRQ handlers — MUST use naked + iretq ────────────────────────────────────
// Regular Rust functions use `ret` which crashes when called as interrupt handler
// We need `iretq` to properly return from a 64-bit interrupt

#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "C" fn irq0_handler() {
    naked_asm!(
        // Save scratch registers
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        // Increment tick counter
        "mov rax, qword ptr [rip + {ticks}]",
        "add rax, 1",
        "mov qword ptr [rip + {ticks}], rax",
        // Send EOI to master PIC
        "mov al, 0x20",
        "out 0x20, al",
        // Restore scratch registers
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "iretq",
        ticks = sym TICKS,
    );
}

#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "C" fn irq1_handler() {
    naked_asm!(
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        // Read scancode from port 0x60
        "in al, 0x60",
        // Store in ring buffer
        "movzx rcx, byte ptr [rip + {head}]",
        "mov byte ptr [rip + {buf} + rcx], al",
        "inc rcx",
        "and rcx, 63",
        "mov byte ptr [rip + {head}], cl",
        // Send EOI
        "mov al, 0x20",
        "out 0x20, al",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "iretq",
        buf  = sym KB_BUF_RAW,
        head = sym KB_HEAD,
    );
}

#[unsafe(naked)]
#[no_mangle]
pub unsafe extern "C" fn irq_spurious() {
    naked_asm!("iretq");
}

// ── Read scancode from keyboard ring buffer ───────────────────────────────────
pub fn read_scancode() -> Option<u8> {
    let tail = KB_TAIL.load(Ordering::Relaxed);
    let head = KB_HEAD.load(Ordering::Relaxed);
    if tail == head {
        return None;
    }
    let sc = unsafe { KB_BUF_RAW[tail as usize] };
    KB_TAIL.store((tail + 1) % KB_BUF_SIZE as u8, Ordering::Relaxed);
    Some(sc)
}

pub fn uptime_seconds() -> u64 {
    TICKS.load(Ordering::Relaxed) / 100
}

pub fn uptime_ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

#[inline]
pub fn enable() {
    unsafe {
        asm!("sti", options(nostack));
    }
}

#[inline]
pub fn disable() {
    unsafe {
        asm!("cli", options(nostack));
    }
}
