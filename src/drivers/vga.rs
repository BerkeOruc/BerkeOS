// BerkeOS — VGA Text Mode Driver
// Physical address 0xB8000 — 80×25 cells — 2 bytes each: [char, attr]
// attr = (bg << 4) | fg

const VGA_BASE: usize = 0xb8000;
const COLS: usize = 80;
const ROWS: usize = 25;

#[allow(dead_code)]
#[derive(Copy, Clone)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

pub struct Vga;

impl Vga {
    pub fn new() -> Self {
        Vga
    }

    #[inline]
    fn put(&self, col: usize, row: usize, ch: u8, fg: Color, bg: Color) {
        if col >= COLS || row >= ROWS {
            return;
        }
        let attr = ((bg as u8) << 4) | (fg as u8);
        let off = (row * COLS + col) * 2;
        unsafe {
            let ptr = (VGA_BASE + off) as *mut u8;
            ptr.write_volatile(ch);
            ptr.add(1).write_volatile(attr);
        }
    }

    pub fn clear(&self, bg: Color) {
        for row in 0..ROWS {
            for col in 0..COLS {
                self.put(col, row, b' ', Color::LightGray, bg);
            }
        }
    }

    pub fn fill_row(&self, row: usize, bg: Color) {
        for col in 0..COLS {
            self.put(col, row, b' ', Color::Black, bg);
        }
    }

    pub fn print_at(&self, col: usize, row: usize, s: &str, fg: Color, bg: Color) {
        let mut c = col;
        for byte in s.bytes() {
            if c >= COLS {
                break;
            }
            let ch = if byte >= 0x20 && byte < 0x7f {
                byte
            } else {
                b'?'
            };
            self.put(c, row, ch, fg, bg);
            c += 1;
        }
    }
}
