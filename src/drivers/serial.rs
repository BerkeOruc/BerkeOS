// ── COM1 Serial Port Driver ─────────────────────────────────────────────────
// Standard PC UART at I/O port 0x3F8
// 16550-compatible, polling mode, 115200 8N1

/// COM1 base port address
pub const SERIAL_PORT: u16 = 0x3F8;

// UART register offsets
const UART_RBR: u16 = 0; // Receive Buffer (read), Transmit Holding (write)
const UART_IER: u16 = 1; // Interrupt Enable
const UART_FCR: u16 = 2; // FIFO Control
const UART_LCR: u16 = 3; // Line Control
const UART_MCR: u16 = 4; // Modem Control
const UART_LSR: u16 = 5; // Line Status
const UART_MSR: u16 = 6; // Modem Status
const UART_SCR: u16 = 7; // Scratch

// LSR bits
const LSR_THRE: u8 = 0x20; // Transmitter Holding Register Empty

// ── Low-level port I/O (same pattern as ata.rs) ─────────────────────────────

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    core::arch::asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack));
    val
}

#[inline]
unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
}

// ── Initialize COM1 at 115200 8N1 ───────────────────────────────────────────
// Divisor = 1 for 115200 baud (115200 / 1 = 115200)
/// Initialize the COM1 serial port at 115200 baud, 8 data bits, no parity, 1 stop bit.
pub fn init() {
    unsafe {
        // Step 1: Enable DLAB (Divisor Latch Access Bit) to set baud rate divisor
        outb(SERIAL_PORT + UART_LCR, 0x80);

        // Step 2: Set divisor to 1 (115200 baud)
        outb(SERIAL_PORT + UART_RBR, 1); // Low byte
        outb(SERIAL_PORT + UART_IER, 0); // High byte

        // Step 3: Set 8N1 (8 data bits, no parity, 1 stop bit), clear DLAB
        outb(SERIAL_PORT + UART_LCR, 0x03);

        // Step 4: Enable FIFO, clear TX/RX buffers
        outb(SERIAL_PORT + UART_FCR, 0xC7);

        // Step 5: Set RTS/DSR ready (modem control)
        outb(SERIAL_PORT + UART_MCR, 0x0B);

        // Step 6: Loopback mode test - set for loopback testing
        outb(SERIAL_PORT + UART_MCR, 0x1F);

        // Step 7: Test serial chip (loopback mode)
        outb(SERIAL_PORT + UART_RBR, 0xAE);
        if inb(SERIAL_PORT + UART_RBR) != 0xAE {
            // Test failed, chip not responding
            return;
        }

        // Step 8: Restore normal mode
        outb(SERIAL_PORT + UART_MCR, 0x0F);
    }
}

// ── Wait for THR to be empty ───────────────────────────────────────────────
/// Wait until the transmit holding register is empty and ready for new data.
fn wait_for_thr_empty() {
    unsafe {
        while (inb(SERIAL_PORT + UART_LSR) & LSR_THRE) == 0 {
            // Spin wait - poll LSR until THRE bit is set
            core::hint::spin_loop();
        }
    }
}

// ── Write a single byte ─────────────────────────────────────────────────────
/// Write a single byte to the serial port. Blocks until the byte is sent.
pub fn write_byte(b: u8) {
    wait_for_thr_empty();
    unsafe {
        outb(SERIAL_PORT + UART_RBR, b);
    }
}

// ── Write a string ──────────────────────────────────────────────────────────
/// Write a null-terminated string to the serial port.
pub fn write_str(s: &str) {
    for &byte in s.as_bytes() {
        write_byte(byte);
    }
}
