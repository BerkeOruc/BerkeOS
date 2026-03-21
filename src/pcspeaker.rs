// BerkeOS — pcspeaker.rs
// PC Speaker Driver using PIT Channel 2

use core::sync::atomic::{AtomicBool, Ordering};

// PIT Registers
const PIT_CHANNEL_2: u8 = 0x42;
const PIT_COMMAND: u8 = 0x43;
const PIT_MODE: u8 = 0xB6;

// Speaker Control
const SPEAKER_PORT: u16 = 0x61;
const SPEAKER_ON_MASK: u8 = 0x03;

// PIT frequencies
const PIT_FREQ: u32 = 1193180;

pub static SPEAKER_ENABLED: AtomicBool = AtomicBool::new(false);
pub static AUDIO_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub unsafe fn speaker_beep(frequency: u16, _duration_ms: u16) {
    if frequency == 0 {
        return;
    }

    let divisor = PIT_FREQ / (frequency as u32);
    if divisor == 0 || divisor > 0xFFFF {
        return;
    }

    let port61 = SPEAKER_PORT as *mut u8;
    let current = port61.read_volatile();
    port61.write_volatile(current & !SPEAKER_ON_MASK);

    let cmd = PIT_MODE | (3 << 1) | 0x30;
    outb(PIT_COMMAND, cmd);

    let div_low = (divisor & 0xFF) as u8;
    let div_high = ((divisor >> 8) & 0xFF) as u8;
    outb(PIT_CHANNEL_2, div_low);
    outb(PIT_CHANNEL_2, div_high);

    let port61 = SPEAKER_PORT as *mut u8;
    let current = port61.read_volatile();
    port61.write_volatile(current | SPEAKER_ON_MASK);
    SPEAKER_ENABLED.store(true, Ordering::Relaxed);
}

pub unsafe fn speaker_off() {
    let port61 = SPEAKER_PORT as *mut u8;
    let current = port61.read_volatile();
    port61.write_volatile(current & !SPEAKER_ON_MASK);
    SPEAKER_ENABLED.store(false, Ordering::Relaxed);
}

pub unsafe fn stop_beep_timer() {
    let port61 = SPEAKER_PORT as *mut u8;
    let current = port61.read_volatile();
    port61.write_volatile(current & !SPEAKER_ON_MASK);
    SPEAKER_ENABLED.store(false, Ordering::Relaxed);
}

fn spin_wait() {
    unsafe {
        for _ in 0..1000 {
            core::arch::asm!("pause");
        }
    }
}

unsafe fn outb(port: u8, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port as u32, in("al") value, options(nomem, nostack));
}

pub fn beep(frequency: u16, duration_ms: u16) {
    unsafe { speaker_beep(frequency, duration_ms) }
}

pub fn beep_ok() {
    unsafe {
        speaker_beep(880, 150);
        spin_wait();
        spin_wait();
        spin_wait();
        speaker_beep(1100, 150);
    }
}

pub fn beep_error() {
    unsafe {
        speaker_beep(200, 300);
        spin_wait();
        speaker_beep(150, 300);
    }
}

pub fn init_audio() -> bool {
    unsafe {
        speaker_beep(880, 100);
    }
    AUDIO_INITIALIZED.store(true, Ordering::Relaxed);
    true
}

pub fn is_audio_working() -> bool {
    AUDIO_INITIALIZED.load(Ordering::Relaxed)
}
