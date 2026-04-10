// BerkeOS — pit.rs
// Intel 8253/8254 Programmable Interval Timer
// Sets IRQ0 frequency to 100 Hz (10ms per tick)

use core::arch::asm;

const PIT_CHANNEL0: u16 = 0x40;
const PIT_CMD: u16 = 0x43;

// ── Set PIT frequency ─────────────────────────────────────────────────────────
// PIT base frequency = 1,193,182 Hz
// divisor = 1193182 / desired_hz
// At 100 Hz: divisor = 11931
pub unsafe fn init(hz: u32) {
    let divisor = 1193182u32 / hz;

    // Channel 0, lo/hi byte, mode 3 (square wave), binary
    outb(PIT_CMD, 0x36);
    outb(PIT_CHANNEL0, (divisor & 0xFF) as u8);
    outb(PIT_CHANNEL0, ((divisor >> 8) & 0xFF) as u8);
}

#[inline]
unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let mut val: u8;
    asm!("in al, dx", in("dx") port, out("al") val);
    val
}

pub fn sleep_ms(ms: u32) {
    unsafe {
        init(1000);
        let start = inb(PIT_CHANNEL0) as u32 | ((inb(PIT_CHANNEL0) as u32) << 8);
        let mut elapsed = 0u32;
        while elapsed < ms {
            let current = inb(PIT_CHANNEL0) as u32 | ((inb(PIT_CHANNEL0) as u32) << 8);
            if current != start {
                elapsed += 1;
            }
        }
    }
}
