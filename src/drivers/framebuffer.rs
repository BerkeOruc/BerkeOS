// BerkeOS — framebuffer.rs
// Pixel framebuffer writer for 32bpp and 24bpp linear framebuffers.

use crate::graphics::font;
use crate::FbInfo;

#[derive(Copy, Clone)]
pub struct Color(pub u32); // 0x00RRGGBB

impl Color {
    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color(((r as u32) << 16) | ((g as u32) << 8) | (b as u32))
    }
    #[inline]
    pub fn r(self) -> u8 {
        ((self.0 >> 16) & 0xff) as u8
    }
    #[inline]
    pub fn g(self) -> u8 {
        ((self.0 >> 8) & 0xff) as u8
    }
    #[inline]
    pub fn b(self) -> u8 {
        (self.0 & 0xff) as u8
    }
}

pub struct Framebuffer {
    addr: *mut u8,
    pub width: usize,
    pub height: usize,
    pub pitch: usize,
    pub bpp: u8,
}

impl Framebuffer {
    pub unsafe fn new(info: FbInfo) -> Self {
        Framebuffer {
            addr: info.addr as *mut u8,
            width: info.width as usize,
            height: info.height as usize,
            pitch: info.pitch as usize,
            bpp: info.bpp,
        }
    }

    #[inline]
    pub fn put_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x >= self.width || y >= self.height {
            return;
        }
        let offset = y * self.pitch + x * (self.bpp as usize / 8);
        unsafe {
            match self.bpp {
                32 => {
                    let ptr = self.addr.add(offset) as *mut u32;
                    ptr.write_volatile(color.0);
                }
                24 => {
                    let ptr = self.addr.add(offset);
                    ptr.add(0).write_volatile(color.b());
                    ptr.add(1).write_volatile(color.g());
                    ptr.add(2).write_volatile(color.r());
                }
                _ => {}
            }
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: usize, y: usize) -> u32 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        let offset = y * self.pitch + x * (self.bpp as usize / 8);
        unsafe {
            match self.bpp {
                32 => {
                    let ptr = self.addr.add(offset) as *const u32;
                    ptr.read_volatile() & 0x00ff_ffff
                }
                24 => {
                    let ptr = self.addr.add(offset);
                    let b = ptr.add(0).read_volatile() as u32;
                    let g = ptr.add(1).read_volatile() as u32;
                    let r = ptr.add(2).read_volatile() as u32;
                    (r << 16) | (g << 8) | b
                }
                _ => 0,
            }
        }
    }

    pub fn clear(&mut self, color: Color) {
        self.fill_rect(0, 0, self.width, self.height, color);
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: Color) {
        let x2 = (x + w).min(self.width);
        let y2 = (y + h).min(self.height);
        for py in y..y2 {
            for px in x..x2 {
                self.put_pixel(px, py, color);
            }
        }
    }

    /// Draw a single glyph. Returns the x advance (always GLYPH_W).
    pub fn draw_glyph(&mut self, x: usize, y: usize, ch: char, fg: Color, bg: Color) -> usize {
        let (bitmap, gw, gh) = font::get_glyph(ch);
        let stride = (gw + 7) / 8;
        for row in 0..gh {
            for col in 0..gw {
                let byte = bitmap[row * stride + col / 8];
                let bit = 7 - (col % 8);
                let set = (byte >> bit) & 1 != 0;
                self.put_pixel(x + col, y + row, if set { fg } else { bg });
            }
        }
        gw
    }

    /// Draw a UTF-8 string.
    pub fn draw_string(&mut self, x: usize, y: usize, s: &str, fg: Color, bg: Color) {
        let mut cx = x;
        for ch in s.chars() {
            if cx + font::GLYPH_W > self.width {
                break;
            }
            let adv = self.draw_glyph(cx, y, ch, fg, bg);
            cx += adv;
        }
    }
}
