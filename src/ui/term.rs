// BerkeOS — term.rs
// Terminal control and ANSI escape sequences

pub const CSI: u8 = 0x1B;
pub const ESC: u8 = 0x1B;
pub const OSC: u8 = 0x9D;
pub const ST: u8 = 0x07;

pub struct TermWriter {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub fg_color: u8,
    pub bg_color: u8,
    pub bold: bool,
}

impl TermWriter {
    pub const fn new(width: usize, height: usize) -> Self {
        TermWriter {
            x: 0,
            y: 0,
            width,
            height,
            fg_color: 7,
            bg_color: 0,
            bold: false,
        }
    }

    pub fn reset(&mut self) {
        self.x = 0;
        self.y = 0;
        self.fg_color = 7;
        self.bg_color = 0;
        self.bold = false;
    }

    pub fn clear_screen(&mut self) -> &[u8] {
        b"\x1b[2J\x1b[H"
    }

    pub fn clear_line(&mut self) -> &[u8] {
        b"\x1b[2K"
    }

    pub fn cursor_home(&mut self) -> &[u8] {
        b"\x1b[H"
    }

    pub fn cursor_show(&mut self) -> &[u8] {
        b"\x1b[?25h"
    }

    pub fn cursor_hide(&mut self) -> &[u8] {
        b"\x1b[?25l"
    }

    pub fn set_color(&mut self, fg: u8, bg: u8) -> [u8; 8] {
        let bold = if self.bold { 1 } else { 0 };
        let mut buf = [0u8; 8];
        buf[0] = ESC;
        buf[1] = b'[';
        buf[2] = b'0' + (bold + 30 + fg) / 100;
        buf[3] = b'0' + ((bold + 30 + fg) / 10) % 10;
        buf[4] = b'0' + (bold + 30 + fg) % 10;
        buf[5] = b';';
        buf[6] = b'4';
        buf[7] = b'0' + bg;
        buf
    }

    pub fn cursor_position(&mut self, x: usize, y: usize) {
        self.x = x;
        self.y = y;
    }

    pub fn scroll_up(&mut self) -> &[u8] {
        b"\x1b[S"
    }

    pub fn scroll_down(&mut self) -> &[u8] {
        b"\x1b[T"
    }
}

fn itoa(mut n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let mut val = 0;
    let mut mult = 1;
    while n > 0 {
        val += (n % 10) * mult;
        n /= 10;
        mult *= 10;
    }
    val
}

pub const COLOR_BLACK: u8 = 0;
pub const COLOR_RED: u8 = 1;
pub const COLOR_GREEN: u8 = 2;
pub const COLOR_YELLOW: u8 = 3;
pub const COLOR_BLUE: u8 = 4;
pub const COLOR_MAGENTA: u8 = 5;
pub const COLOR_CYAN: u8 = 6;
pub const COLOR_WHITE: u8 = 7;
pub const COLOR_BRIGHT: u8 = 8;
