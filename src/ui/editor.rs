use crate::fs::berkefs::BerkeFS;
use crate::drivers::framebuffer::{Color, Framebuffer};
use crate::drivers::keyboard::Key;

pub struct Editor {
    buffer: [u8; 8192],
    buf_len: usize,
    cursor_x: usize,
    cursor_y: usize,
    scroll_y: usize,
    filename: [u8; 64],
    fn_len: usize,
    mode: u8,
    command_buf: [u8; 64],
    cmd_len: usize,
}

impl Editor {
    pub fn new() -> Self {
        Editor {
            buffer: [0; 8192],
            buf_len: 0,
            cursor_x: 0,
            cursor_y: 0,
            scroll_y: 0,
            filename: [0; 64],
            fn_len: 0,
            mode: 0,
            command_buf: [0; 64],
            cmd_len: 0,
        }
    }

    pub fn load(&mut self, name: &[u8], fs: &mut BerkeFS) {
        self.fn_len = 0;
        for &b in name {
            if self.fn_len < 63 {
                self.filename[self.fn_len] = b;
                self.fn_len += 1;
            }
        }

        let mut content = [0u8; 8192];
        if let Some(n) = fs.read_file(name, &mut content) {
            self.buf_len = n.min(8191);
            for i in 0..self.buf_len {
                self.buffer[i] = content[i];
            }
        }
    }

    pub fn save(&self, fs: &mut BerkeFS) -> bool {
        if self.fn_len == 0 {
            return false;
        }
        fs.create_file(&self.filename[..self.fn_len], &self.buffer[..self.buf_len])
    }

    pub fn draw(&mut self, fb: &mut Framebuffer) {
        fb.clear(Color::rgb(0x1a, 0x1a, 0x2e));

        fb.draw_string(
            1,
            1,
            " BerkeOS Vim-Compatible Editor ",
            Color::rgb(0xFF, 0xFF, 0xFF),
            Color::rgb(0x00, 0x88, 0xFF),
        );

        if self.fn_len > 0 {
            fb.draw_string(
                40,
                1,
                "File: ",
                Color::rgb(0xAA, 0xAA, 0xAA),
                Color::rgb(0x1a, 0x1a, 0x2e),
            );
            let name = core::str::from_utf8(&self.filename[..self.fn_len]).unwrap_or("untitled");
            fb.draw_string(
                46,
                1,
                name,
                Color::rgb(0x00, 0xFF, 0x00),
                Color::rgb(0x1a, 0x1a, 0x2e),
            );
        } else {
            fb.draw_string(
                40,
                1,
                "File: untitled",
                Color::rgb(0x00, 0xFF, 0x00),
                Color::rgb(0x1a, 0x1a, 0x2e),
            );
        }

        let mode_indicator = match self.mode {
            0 => "NORMAL",
            1 => "INSERT",
            2 => "COMMAND",
            _ => "NORMAL",
        };
        let mode_color = match self.mode {
            0 => Color::rgb(0x00, 0xFF, 0x00),
            1 => Color::rgb(0xFF, 0x00, 0x00),
            2 => Color::rgb(0xFF, 0xFF, 0x00),
            _ => Color::rgb(0x00, 0xFF, 0x00),
        };
        fb.draw_string(
            75,
            1,
            mode_indicator,
            Color::rgb(0x00, 0x00, 0x00),
            mode_color,
        );

        fb.fill_rect(0, 2, 100, 1, Color::rgb(0x44, 0x44, 0x66));

        let cols = 80;
        let rows = 18;
        let mut x = 1usize;
        let mut y = 3usize;
        let mut line_num = 0usize;

        let mut i = 0usize;
        while i < self.buf_len && y < rows + 3 {
            if line_num < self.scroll_y {
                while i < self.buf_len && self.buffer[i] != b'\n' {
                    i += 1;
                }
                if i < self.buf_len {
                    i += 1;
                }
                line_num += 1;
                continue;
            }

            if x == 1 {
                let ln = line_num + 1;
                let mut lnbuf = [b' '; 5];
                let mut pos = 4usize;
                let mut n = ln;
                if n == 0 {
                    lnbuf[pos] = b'0';
                    pos -= 1;
                }
                while n > 0 && pos > 0 {
                    lnbuf[pos] = b'0' + (n % 10) as u8;
                    n /= 10;
                    pos -= 1;
                }
                for j in 0..5 {
                    fb.draw_glyph(
                        j + 1,
                        y,
                        lnbuf[j] as char,
                        Color::rgb(0x66, 0x66, 0x88),
                        Color::rgb(0x1a, 0x1a, 0x2e),
                    );
                }
                fb.draw_glyph(
                    6,
                    y,
                    '│',
                    Color::rgb(0x44, 0x44, 0x66),
                    Color::rgb(0x1a, 0x1a, 0x2e),
                );
            }

            let ch = self.buffer[i];
            if ch == b'\n' {
                x = 1;
                y += 1;
                i += 1;
                line_num += 1;
                if y >= rows + 3 {
                    break;
                }
            } else if x < cols - 8 {
                let color = self.get_syntax_color(ch, i);
                fb.draw_glyph(x + 7, y, ch as char, color, Color::rgb(0x1a, 0x1a, 0x2e));
                x += 1;
                i += 1;
            } else {
                i += 1;
            }
        }

        for y in (3..rows + 3).rev() {
            fb.draw_glyph(
                1,
                y,
                '│',
                Color::rgb(0x44, 0x44, 0x66),
                Color::rgb(0x1a, 0x1a, 0x2e),
            );
            fb.draw_glyph(
                79,
                y,
                '│',
                Color::rgb(0x44, 0x44, 0x66),
                Color::rgb(0x1a, 0x1a, 0x2e),
            );
        }

        fb.fill_rect(0, rows + 3, 100, 1, Color::rgb(0x44, 0x44, 0x66));

        let help_keys = match self.mode {
            0 => "h/j/k/l:move  i:insert  x:del  dd:del line  :cmds  w:word  0:$:line  G:end  /:search",
            1 => "ESC:normal  Backspace  Enter  Arrow keys",
            2 => "w:save  q:quit  q!:force quit  wq:save+quit",
            _ => "",
        };
        fb.draw_string(
            1,
            rows + 3,
            help_keys,
            Color::rgb(0xAA, 0xAA, 0xAA),
            Color::rgb(0x44, 0x44, 0x66),
        );

        let display_y = self.cursor_y - self.scroll_y + 3;
        if display_y >= 3 && display_y < rows + 3 {
            fb.draw_glyph(
                self.cursor_x + 8,
                display_y,
                '█',
                Color::rgb(0x00, 0xFF, 0x00),
                Color::rgb(0x1a, 0x1a, 0x2e),
            );
        }

        if self.mode == 2 {
            fb.fill_rect(0, 24, 100, 1, Color::rgb(0xFF, 0xFF, 0x00));
            fb.draw_string(
                1,
                24,
                ":",
                Color::rgb(0xFF, 0xFF, 0x00),
                Color::rgb(0x00, 0x00, 0x00),
            );
            let cmd_str = core::str::from_utf8(&self.command_buf[..self.cmd_len]).unwrap_or("");
            fb.draw_string(
                2,
                24,
                cmd_str,
                Color::rgb(0xFF, 0xFF, 0x00),
                Color::rgb(0x00, 0x00, 0x00),
            );
        }
    }

    fn get_syntax_color(&self, ch: u8, _pos: usize) -> Color {
        if ch == b'"' {
            return Color::rgb(0x00, 0xFF, 0x00);
        }
        if ch.is_ascii_digit() {
            return Color::rgb(0xFF, 0x00, 0xFF);
        }
        if ch == b'i' || ch == b'f' || ch == b'e' || ch == b'l' || ch == b's' || ch == b'e' {
            return Color::rgb(0x00, 0xFF, 0xFF);
        }
        Color::rgb(0xFF, 0xFF, 0xFF)
    }

    pub fn handle_key(&mut self, key: Key) -> bool {
        match self.mode {
            0 => self.handle_normal(key),
            1 => self.handle_insert(key),
            2 => self.handle_command(key),
            _ => self.handle_normal(key),
        }
    }

    fn handle_normal(&mut self, key: Key) -> bool {
        match key {
            Key::Char(b'i') => {
                self.mode = 1;
                true
            }
            Key::Char(b'a') => {
                self.cursor_x += 1;
                self.mode = 1;
                true
            }
            Key::Char(b'A') => {
                self.cursor_x = 70;
                self.mode = 1;
                true
            }
            Key::Char(b'o') => {
                self.insert_newline();
                self.mode = 1;
                true
            }
            Key::Char(b':') => {
                self.mode = 2;
                self.cmd_len = 0;
                true
            }
            Key::Char(b'h') => {
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                }
                true
            }
            Key::Char(b'j') => {
                self.cursor_y += 1;
                self.fix_cursor();
                true
            }
            Key::Char(b'k') => {
                if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                }
                self.fix_cursor();
                true
            }
            Key::Char(b'l') => {
                self.cursor_x += 1;
                self.fix_cursor_x();
                true
            }
            Key::Char(b'w') => {
                self.cursor_x += 5;
                self.fix_cursor_x();
                true
            }
            Key::Char(b'b') => {
                if self.cursor_x > 5 {
                    self.cursor_x -= 5;
                } else {
                    self.cursor_x = 0;
                }
                true
            }
            Key::Char(b'x') => {
                self.delete_char();
                true
            }
            Key::Char(b'd') => {
                self.delete_line();
                true
            }
            Key::Char(b'0') => {
                self.cursor_x = 0;
                true
            }
            Key::Char(b'$') => {
                self.cursor_x = 70;
                true
            }
            Key::Char(b'G') => {
                self.cursor_y = self.buf_len / 72;
                true
            }
            Key::Char(b'u') => {
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                }
                true
            }
            Key::Char(b'/') => {
                self.mode = 2;
                self.command_buf[0] = b'/';
                self.cmd_len = 1;
                true
            }
            Key::Char(b'O') => {
                if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                }
                self.insert_newline();
                self.cursor_x = 0;
                self.mode = 1;
                true
            }
            Key::Escape => false,
            Key::Up => {
                if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                }
                self.fix_cursor();
                true
            }
            Key::Down => {
                self.cursor_y += 1;
                self.fix_cursor();
                true
            }
            Key::Left => {
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                }
                true
            }
            Key::Right => {
                self.cursor_x += 1;
                self.fix_cursor_x();
                true
            }
            _ => true,
        }
    }

    fn handle_insert(&mut self, key: Key) -> bool {
        match key {
            Key::Escape => {
                self.mode = 0;
                true
            }
            Key::Char(b'\x08') => {
                self.backspace();
                true
            }
            Key::Char(b'\n') => {
                self.insert_newline();
                true
            }
            Key::Char(c) => {
                self.insert_char(c);
                true
            }
            Key::Up => {
                if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                }
                self.fix_cursor();
                true
            }
            Key::Down => {
                self.cursor_y += 1;
                self.fix_cursor();
                true
            }
            Key::Left => {
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                }
                true
            }
            Key::Right => {
                self.cursor_x += 1;
                self.fix_cursor_x();
                true
            }
            Key::Home => {
                self.cursor_x = 0;
                true
            }
            Key::End => {
                self.cursor_x = 70;
                true
            }
            _ => true,
        }
    }

    fn handle_command(&mut self, key: Key) -> bool {
        match key {
            Key::Escape => {
                self.mode = 0;
                self.cmd_len = 0;
                true
            }
            Key::Char(b'\n') => {
                let cmd = core::str::from_utf8(&self.command_buf[..self.cmd_len]).unwrap_or("");
                if cmd == "q" {
                    return false;
                }
                if cmd == "q!" {
                    return false;
                }
                if cmd == "w" {
                    self.mode = 0;
                    self.cmd_len = 0;
                    return true;
                }
                if cmd == "wq" || cmd == "w q" {
                    self.mode = 0;
                    self.cmd_len = 0;
                    return true;
                }
                self.cmd_len = 0;
                self.mode = 0;
                true
            }
            Key::Char(b'\x08') => {
                if self.cmd_len > 0 {
                    self.cmd_len -= 1;
                }
                true
            }
            Key::Char(c) => {
                if self.cmd_len < 63 {
                    self.command_buf[self.cmd_len] = c;
                    self.cmd_len += 1;
                }
                true
            }
            _ => true,
        }
    }

    fn fix_cursor(&mut self) {
        let max_y = self.buf_len / 72;
        if self.cursor_y > max_y {
            self.cursor_y = max_y;
        }
    }

    fn fix_cursor_x(&mut self) {
        if self.cursor_x > 70 {
            self.cursor_x = 70;
        }
    }

    fn insert_char(&mut self, c: u8) {
        if self.buf_len < 8191 {
            let pos = self.cursor_x + self.cursor_y * 72;
            for i in (pos..self.buf_len).rev() {
                self.buffer[i + 1] = self.buffer[i];
            }
            self.buffer[pos] = c;
            self.buf_len += 1;
            self.cursor_x += 1;
        }
    }

    fn delete_char(&mut self) {
        let pos = self.cursor_x + self.cursor_y * 72;
        if pos < self.buf_len && self.buf_len > 0 {
            for i in pos..self.buf_len - 1 {
                self.buffer[i] = self.buffer[i + 1];
            }
            self.buf_len -= 1;
        }
    }

    fn backspace(&mut self) {
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.cursor_x = 70;
        }
        self.delete_char();
    }

    fn insert_newline(&mut self) {
        self.cursor_y += 1;
        self.cursor_x = 0;
    }

    fn delete_line(&mut self) {
        let start = self.cursor_y * 72;
        let mut end = start;
        while end < self.buf_len && self.buffer[end] != b'\n' {
            end += 1;
        }
        if end < self.buf_len {
            end += 1;
        }

        for i in start..self.buf_len.saturating_sub(end - start) {
            self.buffer[i] = self.buffer[i + (end - start)];
        }
        self.buf_len = self.buf_len.saturating_sub(end - start);
    }
}
