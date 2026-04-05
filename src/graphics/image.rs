use crate::drivers::framebuffer::{Color, Framebuffer};

pub struct BmpImage {
    pub width: usize,
    pub height: usize,
    pub data: [u8; 800 * 600 * 3],
}

impl BmpImage {
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 54 {
            return None;
        }

        if data[0] != b'B' || data[1] != b'M' {
            return None;
        }

        let size = u32::from_le_bytes([data[2], data[3], data[4], data[5]]) as usize;
        let offset = u32::from_le_bytes([data[10], data[11], data[12], data[13]]) as usize;

        let bmp_info_size = u32::from_le_bytes([data[14], data[15], data[16], data[17]]) as usize;
        if bmp_info_size < 40 {
            return None;
        }

        let width = i32::from_le_bytes([data[18], data[19], data[20], data[21]]).abs() as usize;
        let height = i32::from_le_bytes([data[22], data[23], data[24], data[25]]).abs() as usize;
        let planes = u16::from_le_bytes([data[26], data[27]]);
        let bit_count = u16::from_le_bytes([data[28], data[29]]);

        if planes != 1 || (bit_count != 24 && bit_count != 32) {
            return None;
        }

        if width > 800 || height > 600 {
            return None;
        }

        let mut img = BmpImage {
            width,
            height,
            data: [0u8; 800 * 600 * 3],
        };

        let row_size = ((width * (bit_count as usize) + 31) / 32) * 4;
        let data_offset = offset;

        for y in 0..height {
            let row = if height > 0 { height - 1 - y } else { y };
            for x in 0..width {
                let src_idx = data_offset + row * row_size + x * (bit_count as usize / 8);
                if src_idx + 2 < data.len() {
                    let dest_idx = (y * width + x) * 3;
                    img.data[dest_idx] = data[src_idx + 2];
                    img.data[dest_idx + 1] = data[src_idx + 1];
                    img.data[dest_idx + 2] = data[src_idx];
                }
            }
        }

        Some(img)
    }

    pub fn draw(&self, fb: &mut Framebuffer, start_x: usize, start_y: usize) {
        for y in 0..self.height.min(600) {
            for x in 0..self.width.min(800) {
                let idx = (y * self.width + x) * 3;
                let r = self.data[idx + 2];
                let g = self.data[idx + 1];
                let b = self.data[idx];
                fb.fill_rect(start_x + x, start_y + y, 1, 1, Color::rgb(r, g, b));
            }
        }
    }
}

pub fn draw_image(data: &[u8], fb: &mut Framebuffer) {
    if let Some(img) = BmpImage::decode(data) {
        let x = (fb.width.saturating_sub(img.width)) / 2;
        let y = (fb.height.saturating_sub(img.height)) / 2;
        img.draw(fb, x, y);
    }
}
