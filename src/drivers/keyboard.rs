// BerkeOS — keyboard.rs
// PS/2 Keyboard Driver

#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    core::arch::asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack));
    val
}

#[inline]
pub unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
}

const SC_LSHIFT: u8 = 0x2A;
const SC_RSHIFT: u8 = 0x36;
const SC_CAPS: u8 = 0x3A;
const SC_CTRL: u8 = 0x1D;
const SC_F1: u8 = 0x3B;
const SC_F2: u8 = 0x3C;
const SC_F5: u8 = 0x3F;
const SC_ESC: u8 = 0x01;

pub enum Key {
    Char(u8),
    Up,
    Down,
    Left,
    Right,
    Delete,
    Home,
    End,
    F1,
    F2,
    F5,
    Escape,
    CtrlC,
    CtrlL,
    CtrlA,
    CtrlE,
    CtrlU,
    CtrlK,
    CtrlZ,
    CtrlY,
    CtrlS,
    CtrlQ,
    CtrlW,
    None,
}

pub struct Keyboard {
    shift: bool,
    caps: bool,
    ctrl: bool,
    extended: bool,
}

impl Keyboard {
    pub const fn new() -> Self {
        Keyboard {
            shift: false,
            caps: false,
            ctrl: false,
            extended: false,
        }
    }

    fn get_scancode(&mut self) -> Option<u8> {
        unsafe {
            let status = inb(0x64);
            if status & 1 != 0 {
                let sc = inb(0x60);
                return Some(sc);
            }
            if status & 0x20 != 0 {
                let sc = inb(0x60);
                if sc != 0 {
                    return Some(sc);
                }
            }
        }
        None
    }

    fn get_char(&self, sc: u8) -> Option<u8> {
        if self.shift {
            Some(match sc {
                0x02 => b'!',
                0x03 => b'@',
                0x04 => b'#',
                0x05 => b'$',
                0x06 => b'%',
                0x07 => b'^',
                0x08 => b'&',
                0x09 => b'*',
                0x0A => b'(',
                0x0B => b')',
                0x0C => b'_',
                0x0D => b'+',
                0x0E => b'\x08',
                0x0F => b'\t',
                0x10 => b'Q',
                0x11 => b'W',
                0x12 => b'E',
                0x13 => b'R',
                0x14 => b'T',
                0x15 => b'Y',
                0x16 => b'U',
                0x17 => b'I',
                0x18 => b'O',
                0x19 => b'P',
                0x1A => b'{',
                0x1B => b'}',
                0x1C => b'\n',
                0x1E => b'A',
                0x1F => b'S',
                0x20 => b'D',
                0x21 => b'F',
                0x22 => b'G',
                0x23 => b'H',
                0x24 => b'J',
                0x25 => b'K',
                0x26 => b'L',
                0x27 => b':',
                0x28 => b'"',
                0x29 => b'~',
                0x2B => b'|',
                0x2C => b'Z',
                0x2D => b'X',
                0x2E => b'C',
                0x2F => b'V',
                0x30 => b'B',
                0x31 => b'N',
                0x32 => b'M',
                0x33 => b'<',
                0x34 => b'>',
                0x35 => b'?',
                0x37 => b'*',
                0x39 => b' ',
                _ => return None,
            })
        } else {
            match sc {
                0x02 => Some(b'1'),
                0x03 => Some(b'2'),
                0x04 => Some(b'3'),
                0x05 => Some(b'4'),
                0x06 => Some(b'5'),
                0x07 => Some(b'6'),
                0x08 => Some(b'7'),
                0x09 => Some(b'8'),
                0x0A => Some(b'9'),
                0x0B => Some(b'0'),
                0x0C => Some(b'-'),
                0x0D => Some(b'='),
                0x0E => Some(b'\x08'),
                0x0F => Some(b'\t'),
                0x10 => Some(b'q'),
                0x11 => Some(b'w'),
                0x12 => Some(b'e'),
                0x13 => Some(b'r'),
                0x14 => Some(b't'),
                0x15 => Some(b'y'),
                0x16 => Some(b'u'),
                0x17 => Some(b'i'),
                0x18 => Some(b'o'),
                0x19 => Some(b'p'),
                0x1A => Some(b'['),
                0x1B => Some(b']'),
                0x1C => Some(b'\n'),
                0x1E => Some(b'a'),
                0x1F => Some(b's'),
                0x20 => Some(b'd'),
                0x21 => Some(b'f'),
                0x22 => Some(b'g'),
                0x23 => Some(b'h'),
                0x24 => Some(b'j'),
                0x25 => Some(b'k'),
                0x26 => Some(b'l'),
                0x27 => Some(b';'),
                0x28 => Some(b'\''),
                0x29 => Some(b'`'),
                0x2B => Some(b'\\'),
                0x2C => Some(b'z'),
                0x2D => Some(b'x'),
                0x2E => Some(b'c'),
                0x2F => Some(b'v'),
                0x30 => Some(b'b'),
                0x31 => Some(b'n'),
                0x32 => Some(b'm'),
                0x33 => Some(b','),
                0x34 => Some(b'.'),
                0x35 => Some(b'/'),
                0x37 => Some(b'*'),
                0x39 => Some(b' '),
                _ => None,
            }
        }
    }

    pub fn poll(&mut self) -> Key {
        let sc = match self.get_scancode() {
            Some(s) => s,
            None => return Key::None,
        };

        if sc == 0xE0 {
            self.extended = true;
            return Key::None;
        }

        let extended = self.extended;
        self.extended = false;

        if sc & 0x80 != 0 {
            let rel = sc & 0x7F;
            if rel == SC_LSHIFT || rel == SC_RSHIFT {
                self.shift = false;
            }
            if rel == SC_CTRL || (extended && rel == 0x1D) {
                self.ctrl = false;
            }
            return Key::None;
        }

        match sc {
            SC_LSHIFT | SC_RSHIFT => {
                self.shift = true;
                return Key::None;
            }
            SC_CAPS => {
                self.caps = !self.caps;
                return Key::None;
            }
            SC_CTRL => {
                self.ctrl = true;
                return Key::None;
            }
            _ => {}
        }

        if extended {
            return match sc {
                0x48 => Key::Up,
                0x50 => Key::Down,
                0x4B => Key::Left,
                0x4D => Key::Right,
                0x53 => Key::Delete,
                0x47 => Key::Home,
                0x4F => Key::End,
                _ => Key::None,
            };
        }

        match sc {
            SC_F1 => return Key::F1,
            SC_F2 => return Key::F2,
            SC_F5 => return Key::F5,
            SC_ESC => return Key::Escape,
            _ => {}
        }

        if let Some(mut ch) = self.get_char(sc) {
            if self.caps {
                if ch >= b'a' && ch <= b'z' {
                    ch -= 32;
                } else if ch >= b'A' && ch <= b'Z' {
                    ch += 32;
                }
            }

            if self.ctrl {
                return match ch | 0x20 {
                    b'c' => Key::CtrlC,
                    b'l' => Key::CtrlL,
                    b'a' => Key::CtrlA,
                    b'e' => Key::CtrlE,
                    b'u' => Key::CtrlU,
                    b'k' => Key::CtrlK,
                    b'z' => Key::CtrlZ,
                    b'y' => Key::CtrlY,
                    b's' => Key::CtrlS,
                    b'q' => Key::CtrlQ,
                    b'w' => Key::CtrlW,
                    _ => Key::None,
                };
            }

            return Key::Char(ch);
        }

        Key::None
    }
}
