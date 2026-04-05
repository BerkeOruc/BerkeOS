// BerkeOS Kernel Logging

use crate::drivers::serial;

static LOG_COUNTER: spin::Mutex<u32> = spin::Mutex::new(0);

pub fn next_counter() -> u32 {
    let mut counter = LOG_COUNTER.lock();
    *counter = counter.wrapping_add(1);
    *counter
}

pub fn serial_write(s: &str) {
    unsafe {
        crate::drivers::serial::write_str(s);
    }
}

pub fn format_for_vga<'a>(_level: &'a str, msg: &'a str) -> &'a str {
    msg
}

pub fn init() {
    serial_write("[LOG] Logging initialized\n");
}

/// kinfo! - Kernel info log
/// Writes to serial, returns message for VGA
#[macro_export]
macro_rules! kinfo {
    ($($arg:tt)*) => {{
        let msg = core::concat!(core::stringify!($($arg)*));
        $crate::log::serial_write("[INFO] ");
        $crate::log::serial_write(core::concat!(msg, "\n"));
        $crate::log::format_for_vga("[INFO]", msg)
    }};
}

/// kwarn! - Kernel warning log
/// Writes to serial, returns message for VGA
#[macro_export]
macro_rules! kwarn {
    ($($arg:tt)*) => {{
        let msg = core::concat!(core::stringify!($($arg)*));
        $crate::log::serial_write("[WARN] ");
        $crate::log::serial_write(core::concat!(msg, "\n"));
        $crate::log::format_for_vga("[WARN]", msg)
    }};
}

/// kerr! - Kernel error log
/// Writes to serial, returns message for VGA
#[macro_export]
macro_rules! kerr {
    ($($arg:tt)*) => {{
        let msg = core::concat!(core::stringify!($($arg)*));
        $crate::log::serial_write("[ERR] ");
        $crate::log::serial_write(core::concat!(msg, "\n"));
        $crate::log::format_for_vga("[ERR]", msg)
    }};
}

/// kdebug! - Kernel debug log (requires debug_log feature)
#[cfg(feature = "debug_log")]
#[macro_export]
macro_rules! kdebug {
    ($($arg:tt)*) => {{
        let msg = core::concat!(core::stringify!($($arg)*));
        $crate::log::serial_write("[DEBUG] ");
        $crate::log::serial_write(core::concat!(msg, "\n"));
        $crate::log::format_for_vga("[DEBUG]", msg)
    }};
}
