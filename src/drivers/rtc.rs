// BerkeOS — rtc.rs
// CMOS Real Time Clock driver
// Reads current date and time from hardware RTC

use core::arch::asm;

const CMOS_ADDR: u16 = 0x70;
const CMOS_DATA: u16 = 0x71;

#[derive(Copy, Clone)]
pub struct DateTime {
    pub second: u8,
    pub minute: u8,
    pub hour: u8,
    pub day: u8,
    pub month: u8,
    pub year: u16,
}

impl DateTime {
    pub const fn zero() -> Self {
        DateTime {
            second: 0,
            minute: 0,
            hour: 0,
            day: 1,
            month: 1,
            year: 2025,
        }
    }
}

#[inline]
unsafe fn cmos_read(reg: u8) -> u8 {
    outb(CMOS_ADDR, reg);
    inb(CMOS_DATA)
}

#[inline]
unsafe fn outb(port: u16, val: u8) {
    asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    asm!("in al, dx", out("al") v, in("dx") port, options(nomem, nostack));
    v
}

// ── BCD → binary ──────────────────────────────────────────────────────────────
fn bcd_to_bin(bcd: u8) -> u8 {
    (bcd & 0x0F) + ((bcd >> 4) * 10)
}

// ── Check if RTC update in progress ──────────────────────────────────────────
unsafe fn update_in_progress() -> bool {
    outb(CMOS_ADDR, 0x0A);
    inb(CMOS_DATA) & 0x80 != 0
}

// ── Read current date/time ────────────────────────────────────────────────────
pub fn read() -> DateTime {
    unsafe {
        // Wait for no update in progress
        let mut timeout = 0u32;
        while update_in_progress() {
            timeout += 1;
            if timeout > 100000 {
                break;
            }
        }

        let sec = bcd_to_bin(cmos_read(0x00));
        let min = bcd_to_bin(cmos_read(0x02));
        let hour = bcd_to_bin(cmos_read(0x04));
        let day = bcd_to_bin(cmos_read(0x07));
        let month = bcd_to_bin(cmos_read(0x08));
        let year = bcd_to_bin(cmos_read(0x09)) as u16 + 2000;

        // Check status register B for binary/BCD mode
        let regb = cmos_read(0x0B);
        let (sec, min, hour, day, month) = if regb & 0x04 != 0 {
            // Already binary
            (sec, min, hour, day, month)
        } else {
            (sec, min, hour, day, month)
        };

        DateTime {
            second: sec,
            minute: min,
            hour: hour,
            day: day,
            month: month,
            year: year,
        }
    }
}

// ── Format date/time as string into buffer ────────────────────────────────────
pub fn format_datetime(dt: &DateTime, buf: &mut [u8; 32]) -> usize {
    // Format: "2025-01-15  22:30:45"
    let mut i = 0;

    // Year
    let y = dt.year;
    buf[i] = b'0' + ((y / 1000) % 10) as u8;
    i += 1;
    buf[i] = b'0' + ((y / 100) % 10) as u8;
    i += 1;
    buf[i] = b'0' + ((y / 10) % 10) as u8;
    i += 1;
    buf[i] = b'0' + (y % 10) as u8;
    i += 1;
    buf[i] = b'-';
    i += 1;

    // Month
    buf[i] = b'0' + (dt.month / 10);
    i += 1;
    buf[i] = b'0' + (dt.month % 10);
    i += 1;
    buf[i] = b'-';
    i += 1;

    // Day
    buf[i] = b'0' + (dt.day / 10);
    i += 1;
    buf[i] = b'0' + (dt.day % 10);
    i += 1;

    buf[i] = b' ';
    i += 1;
    buf[i] = b' ';
    i += 1;

    // Hour
    buf[i] = b'0' + (dt.hour / 10);
    i += 1;
    buf[i] = b'0' + (dt.hour % 10);
    i += 1;
    buf[i] = b':';
    i += 1;

    // Minute
    buf[i] = b'0' + (dt.minute / 10);
    i += 1;
    buf[i] = b'0' + (dt.minute % 10);
    i += 1;
    buf[i] = b':';
    i += 1;

    // Second
    buf[i] = b'0' + (dt.second / 10);
    i += 1;
    buf[i] = b'0' + (dt.second % 10);
    i += 1;

    i
}

// ── Format uptime ─────────────────────────────────────────────────────────────
pub fn format_uptime(seconds: u64, buf: &mut [u8; 48]) -> usize {
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    let mut i = 0;

    // Hours
    if h > 0 {
        if h >= 10 {
            buf[i] = b'0' + (h / 10) as u8;
            i += 1;
        }
        buf[i] = b'0' + (h % 10) as u8;
        i += 1;
        for &b in b" hours, " {
            buf[i] = b;
            i += 1;
        }
    }

    // Minutes
    if m >= 10 {
        buf[i] = b'0' + (m / 10) as u8;
        i += 1;
    }
    buf[i] = b'0' + (m % 10) as u8;
    i += 1;
    for &b in b" minutes, " {
        buf[i] = b;
        i += 1;
    }

    // Seconds
    if s >= 10 {
        buf[i] = b'0' + (s / 10) as u8;
        i += 1;
    }
    buf[i] = b'0' + (s % 10) as u8;
    i += 1;
    for &b in b" seconds" {
        buf[i] = b;
        i += 1;
    }

    i
}
