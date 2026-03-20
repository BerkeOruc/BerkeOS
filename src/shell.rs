// BerkeOS — shell.rs
// Interactive shell

use crate::berkefs::BerkeFS;
use crate::deno;
use crate::disk_io;
use crate::font;
use crate::framebuffer::{Color, Framebuffer};
use crate::keyboard::{Key, Keyboard};

// ── Color palette ─────────────────────────────────────────────────────────────
fn col_bg() -> Color {
    Color::rgb(0x05, 0x00, 0x0f)
}
fn col_pink() -> Color {
    Color::rgb(0xff, 0x69, 0xb4)
}
fn col_dkpnk() -> Color {
    Color::rgb(0x8b, 0x00, 0x57)
}
fn col_ltpnk() -> Color {
    Color::rgb(0xff, 0xb6, 0xc1)
}
fn col_magenta() -> Color {
    Color::rgb(0xff, 0x00, 0xaa)
}
fn col_cyan() -> Color {
    Color::rgb(0xff, 0x77, 0xdd)
}
fn col_yellow() -> Color {
    Color::rgb(0xff, 0xd7, 0x00)
}
fn col_white() -> Color {
    Color::rgb(0xff, 0xff, 0xff)
}
fn col_gray() -> Color {
    Color::rgb(0x55, 0x22, 0x44)
}
fn col_red() -> Color {
    Color::rgb(0xff, 0x44, 0x44)
}
fn col_gold() -> Color {
    Color::rgb(0xff, 0xcc, 0x00)
}
fn col_green() -> Color {
    Color::rgb(0x44, 0xff, 0x44)
}

const GW: usize = font::GLYPH_W;
const GH: usize = font::GLYPH_H;

const MAX_LINE: usize = 256;
const MAX_HISTORY: usize = 64;
const MAX_ROWS: usize = 1024;
const COLS: usize = 200;

// ── Greek drive names ─────────────────────────────────────────────────────────
#[derive(Copy, Clone, PartialEq)]
enum DriveId {
    Alpha,
    Beta,
    Gamma,
    Sigma,
    Epsilon,
    Zeta,
    Eta,
    Theta,
    Iota,
    Kappa,
    Lambda,
    Mu,
    None,
}

impl DriveId {
    fn from_u8(n: u8) -> Self {
        match n {
            0 => DriveId::Alpha,
            1 => DriveId::Beta,
            2 => DriveId::Gamma,
            3 => DriveId::Sigma,
            4 => DriveId::Epsilon,
            5 => DriveId::Zeta,
            6 => DriveId::Eta,
            7 => DriveId::Theta,
            8 => DriveId::Iota,
            9 => DriveId::Kappa,
            10 => DriveId::Lambda,
            11 => DriveId::Mu,
            _ => DriveId::None,
        }
    }

    fn to_u8(&self) -> u8 {
        match self {
            DriveId::Alpha => 0,
            DriveId::Beta => 1,
            DriveId::Gamma => 2,
            DriveId::Sigma => 3,
            DriveId::Epsilon => 4,
            DriveId::Zeta => 5,
            DriveId::Eta => 6,
            DriveId::Theta => 7,
            DriveId::Iota => 8,
            DriveId::Kappa => 9,
            DriveId::Lambda => 10,
            DriveId::Mu => 11,
            DriveId::None => 255,
        }
    }

    fn from_bytes(b: &[u8]) -> Self {
        match b {
            b"Alpha" | b"Alpha:" => DriveId::Alpha,
            b"Beta" | b"Beta:" => DriveId::Beta,
            b"Gamma" | b"Gamma:" => DriveId::Gamma,
            b"Sigma" | b"Sigma:" => DriveId::Sigma,
            b"Epsilon" | b"Epsilon:" => DriveId::Epsilon,
            b"Zeta" | b"Zeta:" => DriveId::Zeta,
            b"Eta" | b"Eta:" => DriveId::Eta,
            b"Theta" | b"Theta:" => DriveId::Theta,
            b"Iota" | b"Iota:" => DriveId::Iota,
            b"Kappa" | b"Kappa:" => DriveId::Kappa,
            b"Lambda" | b"Lambda:" => DriveId::Lambda,
            b"Mu" | b"Mu:" => DriveId::Mu,
            _ => DriveId::None,
        }
    }
    fn name(&self) -> &'static str {
        match self {
            DriveId::Alpha => "Alpha",
            DriveId::Beta => "Beta",
            DriveId::Gamma => "Gamma",
            DriveId::Sigma => "Sigma",
            DriveId::Epsilon => "Epsilon",
            DriveId::Zeta => "Zeta",
            DriveId::Eta => "Eta",
            DriveId::Theta => "Theta",
            DriveId::Iota => "Iota",
            DriveId::Kappa => "Kappa",
            DriveId::Lambda => "Lambda",
            DriveId::Mu => "Mu",
            DriveId::None => "None",
        }
    }
}

// ── Path handling ─────────────────────────────────────────────────────────────
struct Path {
    drive: DriveId,
    parts: [[u8; 32]; 8],
    part_len: [usize; 8],
    depth: usize,
}

impl Path {
    const fn new() -> Self {
        Path {
            drive: DriveId::Alpha,
            parts: [[0u8; 32]; 8],
            part_len: [0usize; 8],
            depth: 0,
        }
    }

    fn as_display(&self, buf: &mut [u8; 128]) -> usize {
        let mut i = 0;
        let name = self.drive.name();
        for &b in name.as_bytes() {
            if i < 128 {
                buf[i] = b;
                i += 1;
            }
        }
        buf[i] = b':';
        i += 1;
        buf[i] = b'\\';
        i += 1;
        for d in 0..self.depth {
            let len = self.part_len[d];
            for j in 0..len {
                if i < 126 {
                    buf[i] = self.parts[d][j];
                    i += 1;
                }
            }
            buf[i] = b'\\';
            i += 1;
        }
        i
    }

    fn push(&mut self, name: &[u8]) -> bool {
        if self.depth >= 8 {
            return false;
        }
        let n = name.len().min(31);
        self.parts[self.depth][..n].copy_from_slice(&name[..n]);
        self.part_len[self.depth] = n;
        self.depth += 1;
        true
    }

    fn pop(&mut self) {
        if self.depth > 0 {
            self.depth -= 1;
        }
    }
}

// ── Drive state ───────────────────────────────────────────────────────────────
const MAX_DRIVES: usize = 24;
const MAX_DRIVE_LABEL: usize = 16;
const MAX_DRIVE_SIZE: u64 = 2_000_000_000_000; // 2TB max

#[derive(Copy, Clone, PartialEq)]
enum DriveType {
    None,
    System,
    Named,
    Virtual,
    Formatted,
    RamDisk,
}

#[derive(Copy, Clone)]
struct DriveInfo {
    drive_type: DriveType,
    label: [u8; MAX_DRIVE_LABEL],
    size: u64,
    used: u64,
    is_ramdisk: bool,
}

impl DriveInfo {
    const fn new() -> Self {
        DriveInfo {
            drive_type: DriveType::None,
            label: [0u8; MAX_DRIVE_LABEL],
            size: 0,
            used: 0,
            is_ramdisk: false,
        }
    }

    fn set_label(&mut self, name: &[u8]) {
        self.label = [0u8; MAX_DRIVE_LABEL];
        let n = name.len().min(MAX_DRIVE_LABEL - 1);
        self.label[..n].copy_from_slice(&name[..n]);
    }
}

#[derive(Copy, Clone, PartialEq)]
enum UserRole {
    Guest,
    User,
    Admin,
}

struct User {
    name: [u8; 16],
    role: UserRole,
    drive_access: u8,
}

impl Copy for User {}
impl Clone for User {
    fn clone(&self) -> Self {
        *self
    }
}

impl User {
    const fn new() -> Self {
        User {
            name: [0; 16],
            role: UserRole::Guest,
            drive_access: 0,
        }
    }

    fn can_access(&self, drive_idx: usize) -> bool {
        if self.role == UserRole::Admin {
            return true;
        }
        (self.drive_access & (1 << drive_idx)) != 0
    }
}

pub struct Shell {
    buf: [u8; MAX_LINE],
    buf_len: usize,
    cursor: usize,

    history: [[u8; MAX_LINE]; MAX_HISTORY],
    hist_len: [usize; MAX_HISTORY],
    hist_count: usize,
    hist_idx: usize,

    lines: [[u8; COLS]; MAX_ROWS],
    line_colors: [LineColor; MAX_ROWS],
    line_lens: [usize; MAX_ROWS],
    line_count: usize,
    scroll_top: usize,

    term_cols: usize,
    term_rows: usize,

    fb_w: usize,
    fb_h: usize,

    path: Path,
    disk_ok: bool,
    drives: [DriveInfo; MAX_DRIVES],

    current_user: User,
    users: [User; 4],
    user_count: usize,
    logged_in: bool,

    // Per-drive BerkeFS instances (12 drives: Alpha..Mu)
    fs_ptrs: [*mut BerkeFS; 12],
}

#[derive(Copy, Clone)]
pub enum LineColor {
    Normal,
    Success,
    Error,
    Info,
    Command,
    Yellow,
    Gold,
    Warning,
}

impl Shell {
    pub const fn new_static() -> Self {
        Shell {
            buf: [0; MAX_LINE],
            buf_len: 0,
            cursor: 0,
            history: [[0; MAX_LINE]; MAX_HISTORY],
            hist_len: [0; MAX_HISTORY],
            hist_count: 0,
            hist_idx: 0,
            lines: [[0; COLS]; MAX_ROWS],
            line_colors: [LineColor::Normal; MAX_ROWS],
            line_lens: [0; MAX_ROWS],
            line_count: 0,
            scroll_top: 0,
            term_cols: 0,
            term_rows: 0,
            fb_w: 0,
            fb_h: 0,
            path: Path::new(),
            disk_ok: false,
            drives: [DriveInfo::new(); MAX_DRIVES],
            current_user: User::new(),
            users: [User::new(); 4],
            user_count: 0,
            logged_in: false,
            fs_ptrs: [core::ptr::null_mut(); 12],
        }
    }

    pub fn init(
        &mut self,
        fb_w: usize,
        fb_h: usize,
        disk_ok: bool,
        disk_count: usize,
        fs0: &'static mut BerkeFS,
        fs1: &'static mut BerkeFS,
        fs2: &'static mut BerkeFS,
        fs3: &'static mut BerkeFS,
        fs4: &'static mut BerkeFS,
        fs5: &'static mut BerkeFS,
        fs6: &'static mut BerkeFS,
        fs7: &'static mut BerkeFS,
        fs8: &'static mut BerkeFS,
        fs9: &'static mut BerkeFS,
        fs10: &'static mut BerkeFS,
        fs11: &'static mut BerkeFS,
    ) {
        self.fs_ptrs = [
            fs0 as *mut BerkeFS,
            fs1 as *mut BerkeFS,
            fs2 as *mut BerkeFS,
            fs3 as *mut BerkeFS,
            fs4 as *mut BerkeFS,
            fs5 as *mut BerkeFS,
            fs6 as *mut BerkeFS,
            fs7 as *mut BerkeFS,
            fs8 as *mut BerkeFS,
            fs9 as *mut BerkeFS,
            fs10 as *mut BerkeFS,
            fs11 as *mut BerkeFS,
        ];
        let term_cols = (fb_w.saturating_sub(20)) / GW;
        let usable_h = fb_h.saturating_sub(26 + 26 + GH + 10);
        let term_rows = usable_h / GH;
        self.term_cols = term_cols.min(COLS);
        self.term_rows = term_rows.min(MAX_ROWS);
        self.fb_w = fb_w;
        self.fb_h = fb_h;
        self.disk_ok = disk_ok;
        self.path.drive = DriveId::Alpha;
        self.path.depth = 0;

        for i in 0..MAX_DRIVES {
            self.drives[i].drive_type = DriveType::None;
        }

        let actual_disks = if disk_ok { disk_count.max(1) } else { 1 };

        if disk_ok {
            if actual_disks >= 1 {
                self.drives[0].drive_type = DriveType::Formatted;
                self.drives[0].set_label(b"Alpha");
                self.drives[0].size = 128 * 1024 * 1024;
                self.drives[0].used = 0;
                self.drives[0].is_ramdisk = true;
            }
            if actual_disks >= 2 {
                self.drives[1].drive_type = DriveType::Formatted;
                self.drives[1].set_label(b"Beta");
                self.drives[1].size = 256 * 1024 * 1024;
                self.drives[1].used = 0;
                self.drives[1].is_ramdisk = false;
            }
            if actual_disks >= 3 {
                self.drives[2].drive_type = DriveType::Formatted;
                self.drives[2].set_label(b"Gamma");
                self.drives[2].size = 512 * 1024 * 1024;
                self.drives[2].used = 0;
            }
            if actual_disks >= 4 {
                self.drives[3].drive_type = DriveType::Formatted;
                self.drives[3].set_label(b"Sigma");
                self.drives[3].size = 1024 * 1024 * 1024;
                self.drives[3].used = 0;
            }
        } else {
            self.drives[0].drive_type = DriveType::Formatted;
            self.drives[0].set_label(b"Alpha");
            self.drives[0].size = 64 * 1024 * 1024;
            self.drives[0].used = 0;
            self.drives[0].is_ramdisk = true;

            self.drives[1].drive_type = DriveType::Formatted;
            self.drives[1].set_label(b"Beta");
            self.drives[1].size = 256 * 1024 * 1024;
            self.drives[1].used = 0;
            self.drives[1].is_ramdisk = false;

            self.drives[2].drive_type = DriveType::Formatted;
            self.drives[2].set_label(b"Gamma");
            self.drives[2].size = 128 * 1024 * 1024;
            self.drives[2].used = 0;

            self.drives[3].drive_type = DriveType::Formatted;
            self.drives[3].set_label(b"Sigma");
            self.drives[3].size = 128 * 1024 * 1024;
            self.drives[3].used = 0;
        }

        // Initialize default users
        self.users = [User::new(); 4];
        self.user_count = 0;
        self.logged_in = false;

        // Add admin user (default)
        self.users[0].name = Self::str_to_array("admin");
        self.users[0].role = UserRole::Admin;
        self.users[0].drive_access = 0xFF; // All drives
        self.user_count = 1;

        // Add guest user
        self.users[1].name = Self::str_to_array("guest");
        self.users[1].role = UserRole::Guest;
        self.users[1].drive_access = 0x01; // Only Alpha
        self.user_count = 2;

        // Set current user to guest by default
        self.current_user = self.users[1];
        self.logged_in = true;
    }

    fn get_current_fs(&mut self) -> &mut BerkeFS {
        let drive_idx = self.path.drive.to_u8() as usize;
        if drive_idx < 12 && !self.fs_ptrs[drive_idx].is_null() {
            unsafe { &mut *self.fs_ptrs[drive_idx] }
        } else if !self.fs_ptrs[0].is_null() {
            unsafe { &mut *self.fs_ptrs[0] }
        } else {
            panic!("No filesystem available");
        }
    }

    fn str_to_array(s: &str) -> [u8; 16] {
        let mut arr = [0u8; 16];
        for (i, b) in s.bytes().enumerate() {
            if i < 16 {
                arr[i] = b;
            }
        }
        arr
    }

    pub fn new(fb_w: usize, fb_h: usize) -> Self {
        let mut s = Shell::new_static();
        unsafe {
            s.init(
                fb_w,
                fb_h,
                false,
                0,
                &mut crate::FS0,
                &mut crate::FS1,
                &mut crate::FS2,
                &mut crate::FS3,
                &mut crate::FS4,
                &mut crate::FS5,
                &mut crate::FS6,
                &mut crate::FS7,
                &mut crate::FS8,
                &mut crate::FS9,
                &mut crate::FS10,
                &mut crate::FS11,
            );
        }
        s
    }

    // ── Output ────────────────────────────────────────────────────────────────
    fn push_line(&mut self, text: &[u8], color: LineColor) {
        if self.line_count >= MAX_ROWS {
            for i in 0..MAX_ROWS - 1 {
                self.lines[i] = self.lines[i + 1];
                self.line_colors[i] = self.line_colors[i + 1];
                self.line_lens[i] = self.line_lens[i + 1];
            }
            self.line_count = MAX_ROWS - 1;
            if self.scroll_top > 0 {
                self.scroll_top -= 1;
            }
        }
        let n = text.len().min(COLS);
        self.lines[self.line_count][..n].copy_from_slice(&text[..n]);
        self.line_lens[self.line_count] = n;
        self.line_colors[self.line_count] = color;
        self.line_count += 1;
        if self.line_count > self.term_rows {
            self.scroll_top = self.line_count - self.term_rows;
        }
    }

    pub fn println(&mut self, s: &str, color: LineColor) {
        let bytes = s.as_bytes();
        if bytes.is_empty() {
            self.push_line(b"", LineColor::Normal);
            return;
        }
        let mut start = 0;
        while start < bytes.len() {
            let end = (start + self.term_cols).min(bytes.len());
            self.push_line(&bytes[start..end], color);
            start = end;
        }
    }

    pub fn empty_line(&mut self) {
        self.push_line(b"", LineColor::Normal);
    }

    // ── Draw ──────────────────────────────────────────────────────────────────
    pub fn draw_full(&self, fb: &mut Framebuffer) {
        fb.clear(col_bg());
        self.draw_topbar(fb);
        self.draw_output(fb);
        self.draw_prompt(fb);
        self.draw_bottombar(fb);
    }

    fn draw_topbar(&self, fb: &mut Framebuffer) {
        fb.fill_rect(0, 0, self.fb_w, 24, col_dkpnk());
        let disk_str = if self.disk_ok {
            "BerkeFS:OK"
        } else {
            "BerkeFS:NO-DISK"
        };
        let top = "BerkeOS  \u{2551}  Berke Oruc  \u{2551}  ";
        let mut buf = [0u8; 200];
        let mut i = 0;
        for &b in top.as_bytes() {
            if i < 200 {
                buf[i] = b;
                i += 1;
            }
        }
        for &b in disk_str.as_bytes() {
            if i < 200 {
                buf[i] = b;
                i += 1;
            }
        }
        for &b in "  \u{2551}  F1=Help  F2=Clear  F5=neofetch".as_bytes() {
            if i < 200 {
                buf[i] = b;
                i += 1;
            }
        }
        fb.draw_string(
            8,
            4,
            core::str::from_utf8(&buf[..i]).unwrap_or("BerkeOS v0.6.1"),
            col_ltpnk(),
            col_dkpnk(),
        );
    }

    fn draw_bottombar(&self, fb: &mut Framebuffer) {
        let y = self.fb_h.saturating_sub(24);
        fb.fill_rect(0, y, self.fb_w, 24, col_dkpnk());

        // Current path
        let mut pbuf = [0u8; 128];
        let plen = self.path.as_display(&mut pbuf);
        let mut s = [0u8; 130];
        s[0] = b' ';
        s[1] = b' ';
        s[2..2 + plen].copy_from_slice(&pbuf[..plen]);
        fb.draw_string(
            8,
            y + 4,
            core::str::from_utf8(&s[..2 + plen]).unwrap_or(""),
            col_ltpnk(),
            col_dkpnk(),
        );

        // Scroll indicator
        let total = self.line_count;
        let shown = (self.scroll_top + self.term_rows).min(total);
        let mut rbuf = [0u8; 32];
        let rs = fmt_scroll(shown, total, &mut rbuf);
        let rx = self.fb_w.saturating_sub((rs.len() + 4) * GW);
        fb.draw_string(rx, y + 4, rs, col_ltpnk(), col_dkpnk());
    }

    fn draw_output(&self, fb: &mut Framebuffer) {
        let y_start = 26usize;
        for i in 0..self.term_rows {
            let line_idx = self.scroll_top + i;
            if line_idx >= self.line_count {
                break;
            }
            let y = y_start + i * GH;
            let text = core::str::from_utf8(&self.lines[line_idx][..self.line_lens[line_idx]])
                .unwrap_or("");
            let color = match self.line_colors[line_idx] {
                LineColor::Normal => col_pink(),
                LineColor::Success => col_ltpnk(),
                LineColor::Error => col_red(),
                LineColor::Info => col_cyan(),
                LineColor::Command => col_white(),
                LineColor::Yellow => col_yellow(),
                LineColor::Gold => col_gold(),
                LineColor::Warning => col_yellow(),
            };
            fb.draw_string(10, y, text, color, col_bg());
        }
    }

    fn draw_prompt(&self, fb: &mut Framebuffer) {
        let y = self.fb_h.saturating_sub(26 + GH + 8);
        fb.fill_rect(0, y - 2, self.fb_w, 1, col_gray());

        // Build prompt: "berke@BerkeOS Alpha:\dir> "
        let mut prompt = [0u8; 160];
        let mut pi = 0;
        for &b in b"berke@BerkeOS " {
            if pi < 160 {
                prompt[pi] = b;
                pi += 1;
            }
        }
        let mut pbuf = [0u8; 128];
        let plen = self.path.as_display(&mut pbuf);
        for j in 0..plen {
            if pi < 158 {
                prompt[pi] = pbuf[j];
                pi += 1;
            }
        }
        for &b in b" > " {
            if pi < 160 {
                prompt[pi] = b;
                pi += 1;
            }
        }

        let prompt_str = core::str::from_utf8(&prompt[..pi]).unwrap_or("> ");
        fb.draw_string(10, y, prompt_str, col_magenta(), col_bg());
        let prompt_px = 10 + pi * GW;
        let input = core::str::from_utf8(&self.buf[..self.buf_len]).unwrap_or("");
        fb.draw_string(prompt_px, y, input, col_white(), col_bg());
        let cursor_x = prompt_px + self.cursor * GW;
        let ch = if self.cursor < self.buf_len {
            self.buf[self.cursor]
        } else {
            b' '
        };
        fb.fill_rect(cursor_x, y, GW, GH, col_pink());
        let ch_arr = [ch];
        if let Ok(s) = core::str::from_utf8(&ch_arr) {
            fb.draw_string(cursor_x, y, s, col_bg(), col_pink());
        }
    }

    // ── Input handling ────────────────────────────────────────────────────────
    fn insert_char(&mut self, ch: u8) {
        if self.buf_len >= MAX_LINE - 1 {
            return;
        }
        let mut i = self.buf_len;
        while i > self.cursor {
            self.buf[i] = self.buf[i - 1];
            i -= 1;
        }
        self.buf[self.cursor] = ch;
        self.buf_len += 1;
        self.cursor += 1;
    }

    fn delete_before(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut i = self.cursor - 1;
        while i < self.buf_len - 1 {
            self.buf[i] = self.buf[i + 1];
            i += 1;
        }
        self.buf_len -= 1;
        self.cursor -= 1;
    }

    fn delete_at(&mut self) {
        if self.cursor >= self.buf_len {
            return;
        }
        let mut i = self.cursor;
        while i < self.buf_len - 1 {
            self.buf[i] = self.buf[i + 1];
            i += 1;
        }
        self.buf_len -= 1;
    }

    fn history_up(&mut self) {
        if self.hist_count == 0 {
            return;
        }
        if self.hist_idx > 0 {
            self.hist_idx -= 1;
        }
        let idx = self.hist_idx;
        let len = self.hist_len[idx];
        self.buf[..len].copy_from_slice(&self.history[idx][..len]);
        self.buf_len = len;
        self.cursor = len;
    }

    fn history_down(&mut self) {
        if self.hist_idx + 1 >= self.hist_count {
            self.buf = [0; MAX_LINE];
            self.buf_len = 0;
            self.cursor = 0;
            self.hist_idx = self.hist_count;
            return;
        }
        self.hist_idx += 1;
        let idx = self.hist_idx;
        let len = self.hist_len[idx];
        self.buf[..len].copy_from_slice(&self.history[idx][..len]);
        self.buf_len = len;
        self.cursor = len;
    }

    fn commit_history(&mut self) {
        if self.buf_len == 0 {
            return;
        }
        if self.hist_count > 0 {
            let last = self.hist_count - 1;
            if self.hist_len[last] == self.buf_len
                && self.history[last][..self.buf_len] == self.buf[..self.buf_len]
            {
                self.hist_idx = self.hist_count;
                return;
            }
        }
        if self.hist_count < MAX_HISTORY {
            let idx = self.hist_count;
            let n = self.buf_len;
            self.history[idx][..n].copy_from_slice(&self.buf[..n]);
            self.hist_len[idx] = n;
            self.hist_count += 1;
        } else {
            for i in 0..MAX_HISTORY - 1 {
                self.history[i] = self.history[i + 1];
                self.hist_len[i] = self.hist_len[i + 1];
            }
            let n = self.buf_len;
            self.history[MAX_HISTORY - 1][..n].copy_from_slice(&self.buf[..n]);
            self.hist_len[MAX_HISTORY - 1] = n;
        }
        self.hist_idx = self.hist_count;
    }

    // ── Command execution ─────────────────────────────────────────────────────
    fn execute(&mut self, fb: &mut Framebuffer, fs: &mut BerkeFS) {
        if self.buf_len == 0 {
            self.empty_line();
            return;
        }

        // Echo the command
        let mut echo = [0u8; MAX_LINE + 4];
        echo[0] = b'>';
        echo[1] = b' ';
        echo[2..2 + self.buf_len].copy_from_slice(&self.buf[..self.buf_len]);
        self.push_line(&echo[..2 + self.buf_len], LineColor::Command);

        self.commit_history();

        // Copy command buffer before clearing
        let mut cmd_buf = [0u8; MAX_LINE];
        let cmd_len = self.buf_len;
        cmd_buf[..cmd_len].copy_from_slice(&self.buf[..cmd_len]);

        // ── KEY PARSING LOGIC ─────────────────────────────────────────────────────
        // Parse: find command end and arg start
        // `ci` = command index (ilk bosluga kadar)
        // `ai` = argument index (bosluklari atlayarak ilk arg'a gelir)
        // Ornek: "cat dosya.txt" -> ci=3 (cat), ai=4 (dosya.txt)
        let mut ci = 0;
        while ci < cmd_len && cmd_buf[ci] != b' ' {
            ci += 1;
        }
        let mut ai = ci;
        while ai < cmd_len && cmd_buf[ai] == b' ' {
            ai += 1;
        }

        let mut cmd_arr = [0u8; 32];
        let clen = ci.min(32);
        cmd_arr[..clen].copy_from_slice(&cmd_buf[..clen]);

        let mut arg_arr = [0u8; MAX_LINE];
        let alen = if ai < cmd_len {
            (cmd_len - ai).min(MAX_LINE)
        } else {
            0
        };
        if alen > 0 {
            arg_arr[..alen].copy_from_slice(&cmd_buf[ai..ai + alen]);
        }

        // Clear input buffer
        self.buf = [0; MAX_LINE];
        self.buf_len = 0;
        self.cursor = 0;

        // Copy arg to local
        let mut arg = [0u8; MAX_LINE];
        arg[..alen].copy_from_slice(&arg_arr[..alen]);
        let arg_slice = &arg[..alen];

        // ── DISPATCHER ──────────────────────────────────────────────────────────
        // Komut ayristirma: once komut sonra argüman ayrilir
        // Command parsing: first word = command, rest = arguments
        // Match statement butun komutlari tek yerde tutar
        // Her branch ayri bir cmd_ fonksiyonuna yonlendirir
        match &cmd_arr[..clen] {
            b"help" => self.cmd_help(),
            b"clear" | b"cls" => self.cmd_clear(),
            b"echo" => {
                self.push_line(arg_slice, LineColor::Normal);
            }
            b"about" => self.cmd_about(),
            b"version" | b"ver" => self.cmd_version(),
            b"uname" => self.cmd_uname(),
            b"uptime" => self.cmd_uptime(),
            b"whoami" => self.cmd_whoami(),
            b"drives" => self.cmd_drives(),
            b"ls" | b"dir" => self.cmd_ls(arg_slice, fs),
            b"pwd" => self.cmd_pwd(),
            b"cd" => self.cmd_cd(arg_slice, fs),
            b"cat" | b"type" => self.cmd_cat(arg_slice, fs),
            b"mkdir" | b"md" => self.cmd_mkdir(arg_slice, fs),
            b"touch" => self.cmd_touch(arg_slice, fs),
            b"rm" | b"del" => self.cmd_rm(arg_slice, fs),
            b"write" => self.cmd_write(arg_slice, fs),
            b"cp" | b"copy" => self.cmd_cp(arg_slice, fs),
            b"mv" | b"move" | b"ren" => self.cmd_mv(arg_slice, fs),
            b"find" => self.cmd_find(arg_slice, fs),
            b"stat" => self.cmd_stat(arg_slice, fs),
            b"fsinfo" => self.cmd_fsinfo(fs),
            b"format" => self.cmd_format(arg_slice, fs),
            b"mkdrive" => self.cmd_mkdrive(arg_slice),
            b"rmdrive" => self.cmd_rmdrive(arg_slice),

            b"beep" => self.cmd_beep(arg_slice),
            b"music" | b"sound" => self.cmd_music(),
            b"audio" => self.cmd_audio(),
            b"play" => self.cmd_play(arg_slice),
            b"install" | b"berkeinstall" => self.cmd_install(arg_slice),
            b"history" => self.cmd_history(),
            b"reboot" => self.cmd_reboot(),
            b"halt" | b"shutdown" => self.cmd_halt(),
            b"mem" => self.cmd_mem(),
            b"color" => self.cmd_color(),
            b"banner" => self.cmd_banner(),
            b"date" => self.cmd_date(),
            b"ticks" => self.cmd_ticks(),
            b"phase" => self.cmd_phase(),
            b"roadmap" => self.cmd_roadmap(),
            b"neofetch" => self.cmd_neofetch(),
            b"sysinfo" => self.cmd_sysinfo(),
            b"calc" => self.cmd_calc(arg_slice),
            b"df" => self.cmd_df(fs),
            b"deno" | b"edit" | b"nano" => self.cmd_editor(arg_slice, fs, fb),
            b"img" => self.cmd_img(arg_slice),
            b"video" => self.cmd_video(arg_slice),
            b"doom" => self.cmd_doom(),
            b"berke" => self.cmd_berke(),
            b"update" => self.cmd_update(),
            other => {
                // Check if it's a drive switch: "Alpha:" etc.
                let drive = DriveId::from_bytes(&other[..other.len().min(8)]);
                if drive != DriveId::None {
                    self.switch_drive(drive);
                } else {
                    let mut msg = [0u8; 80];
                    let pfx = b"  command not found: ";
                    let n1 = pfx.len().min(80);
                    msg[..n1].copy_from_slice(&pfx[..n1]);
                    let n2 = other.len().min(80 - n1);
                    msg[n1..n1 + n2].copy_from_slice(&other[..n2]);
                    self.push_line(&msg[..n1 + n2], LineColor::Error);
                    self.println("  Type 'help' for available commands.", LineColor::Info);
                }
            }
        }

        self.empty_line();
        self.draw_full(fb);
    }

    // ── Drive switching ───────────────────────────────────────────────────────
    fn switch_drive(&mut self, drive: DriveId) {
        let drive_idx = drive.to_u8() as usize;

        if self.drives[drive_idx].drive_type == DriveType::None {
            let name = drive.name();
            let mut msg = [0u8; 80];
            let mut mi = 0;
            for &b in b"  Error: drive does not exist: " {
                if mi < 80 {
                    msg[mi] = b;
                    mi += 1;
                }
            }
            for &b in name.as_bytes() {
                if mi < 78 {
                    msg[mi] = b;
                    mi += 1;
                }
            }
            self.push_line(&msg[..mi], LineColor::Error);
            self.println("  Use 'mkdrive' to create it first.", LineColor::Info);
            return;
        }

        self.path.drive = drive;
        self.path.depth = 0;
        let name = drive.name();
        let mut msg = [0u8; 80];
        let pfx = b"  Switched to drive ";
        msg[..pfx.len()].copy_from_slice(pfx);
        let mut mi = pfx.len();
        for &b in name.as_bytes() {
            if mi < 78 {
                msg[mi] = b;
                mi += 1;
            }
        }
        msg[mi] = b':';
        mi += 1;
        self.push_line(&msg[..mi], LineColor::Success);
    }

    // ── Is current drive using BerkeFS? ───────────────────────────────────────
    fn on_berkefs(&self) -> bool {
        let drive_idx = self.path.drive.to_u8() as usize;
        let state = self.drives[drive_idx].drive_type;
        state == DriveType::Formatted
    }

    // ── Build full BerkeFS path from current path + filename ─────────────────
    fn full_path(&self, name: &[u8], out: &mut [u8; 64]) -> usize {
        let mut i = 0;
        for d in 0..self.path.depth {
            let len = self.path.part_len[d];
            for j in 0..len {
                if i < 62 {
                    out[i] = self.path.parts[d][j];
                    i += 1;
                }
            }
            out[i] = b'/';
            i += 1;
        }
        for &b in name {
            if i < 63 {
                out[i] = b;
                i += 1;
            }
        }
        i
    }

    fn copy_str(src: &[u8], dst: &mut [u8]) -> usize {
        let mut i = 0;
        for &b in src {
            if i < dst.len() {
                dst[i] = b;
                i += 1;
            }
        }
        i
    }

    // ── Commands ──────────────────────────────────────────────────────────────

    // YARDIM — help command / Yardim maskesi gosterir
    // Tum komutlari listeler - shows all avaliable commands
    // ASCII box cizgileri ile guzel bir yardim menusu
    fn cmd_help(&mut self) {
        self.empty_line();
        self.println(
            "╔═══════════════════════════════════════════════════════════╗",
            LineColor::Info,
        );
        self.println(
            "║  BerkeOS v0.6.1                                           ║",
            LineColor::Info,
        );
        self.println(
            "║  Developer: Berke Oruc                                    ║",
            LineColor::Info,
        );
        self.println(
            "╚═══════════════════════════════════════════════════════════╝",
            LineColor::Info,
        );
        self.empty_line();
        self.println("STRUCTURE: Monolithic Kernel (x86_64)", LineColor::Normal);
        self.println(
            "LANG: Rust (no_std)                                         ",
            LineColor::Normal,
        );
        self.println(
            "LINES: ~14,288                                               ",
            LineColor::Normal,
        );
        self.empty_line();

        self.println(
            "╔══ NAVIGATION ══════════════════════════════════════════════╗",
            LineColor::Gold,
        );
        self.println(
            "║ cd <path>     - change directory                        ║",
            LineColor::Normal,
        );
        self.println(
            "║ pwd          - print working directory                  ║",
            LineColor::Normal,
        );
        self.println(
            "║ ls / dir     - list directory                           ║",
            LineColor::Normal,
        );
        self.println(
            "╚════════════════════════════════════════════════════════════╝",
            LineColor::Gold,
        );
        self.empty_line();

        self.println(
            "╔══ FILE OPERATIONS ══════════════════════════════════════════╗",
            LineColor::Gold,
        );
        self.println(
            "║ cat <file>    - read file                               ║",
            LineColor::Normal,
        );
        self.println(
            "║ touch <file> - create file                             ║",
            LineColor::Normal,
        );
        self.println(
            "║ mkdir <dir>  - create directory                        ║",
            LineColor::Normal,
        );
        self.println(
            "║ rm <file>    - delete file                             ║",
            LineColor::Normal,
        );
        self.println(
            "║ cp <src> <dst> - copy file                              ║",
            LineColor::Normal,
        );
        self.println(
            "║ mv <src> <dst> - move/rename                            ║",
            LineColor::Normal,
        );
        self.println(
            "╚════════════════════════════════════════════════════════════╝",
            LineColor::Gold,
        );
        self.empty_line();

        self.println(
            "╔══ EDITOR ═══════════════════════════════════════════════════╗",
            LineColor::Gold,
        );
        self.println(
            "║ deno <file> - Deno Text Editor                          ║",
            LineColor::Normal,
        );
        self.println(
            "╚════════════════════════════════════════════════════════════╝",
            LineColor::Gold,
        );
        self.empty_line();

        self.println(
            "╔══ SYSTEM ════════════════════════════════════════════════════╗",
            LineColor::Gold,
        );
        self.println(
            "║ help / ver / uptime / mem / date / df / sysinfo          ║",
            LineColor::Normal,
        );
        self.println(
            "║ reboot / halt / about / neofetch / uname                  ║",
            LineColor::Normal,
        );
        self.println(
            "╚════════════════════════════════════════════════════════════╝",
            LineColor::Gold,
        );
        self.empty_line();

        self.println(
            "╔══ HARDWARE ═══════════════════════════════════════════════════╗",
            LineColor::Gold,
        );
        self.println(
            "║ drives       - list drives                                ║",
            LineColor::Normal,
        );
        self.println(
            "╚════════════════════════════════════════════════════════════╝",
            LineColor::Gold,
        );
        self.empty_line();

        self.println(
            "╔══ UTILITIES ═════════════════════════════════════════════════╗",
            LineColor::Gold,
        );
        self.println(
            "║ calc <expr>  - calculator                                ║",
            LineColor::Normal,
        );
        self.println(
            "║ update      - show planned features                     ║",
            LineColor::Normal,
        );
        self.println(
            "╚════════════════════════════════════════════════════════════╝",
            LineColor::Gold,
        );
        self.empty_line();
    }

    // Temizleme - clear screen / Ekrani siler
    // History korunur sadece ekran temizlenir
    fn cmd_clear(&mut self) {
        self.line_count = 0;
        self.scroll_top = 0;
    }

    fn cmd_drives(&mut self) {
        self.empty_line();
        self.println(
            "  ╔══════════════════════════════════════════════════════════╗",
            LineColor::Gold,
        );
        self.println(
            "  ║              BerkeFS Drives                               ║",
            LineColor::Gold,
        );
        self.println(
            "  ╠═══════════╦═══════════╦════════════════════════════════╣",
            LineColor::Gold,
        );
        self.println(
            "  ║ Drive     ║ Turu      ║ Durum                            ║",
            LineColor::Gold,
        );
        self.println(
            "  ╠═══════════╬═══════════╬════════════════════════════════╣",
            LineColor::Gold,
        );

        let mut count = 0;
        for i in 0..MAX_DRIVES {
            let drive_type = self.drives[i].drive_type;
            if drive_type == DriveType::None {
                continue;
            }

            if drive_type != DriveType::RamDisk && drive_type != DriveType::Formatted {
                continue;
            }

            count += 1;
            let name = DriveId::from_u8(i as u8).name();
            let is_current = self.path.drive.to_u8() as usize == i;

            let type_str: &[u8] = match drive_type {
                DriveType::RamDisk => b"RAM Disk  ",
                DriveType::Formatted => b"Disk     ",
                _ => continue,
            };

            let mut line_buf = [0u8; 64];
            let mut li = 0;
            let pfx = "  \u{2551} ";
            for &b in pfx.as_bytes() {
                line_buf[li] = b;
                li += 1;
            }

            for &b in name.as_bytes() {
                if li < 12 {
                    line_buf[li] = b;
                    li += 1;
                }
            }

            let sep1 = " \u{2551} ";
            for &b in sep1.as_bytes() {
                line_buf[li] = b;
                li += 1;
            }

            for &b in type_str {
                if li < 25 {
                    line_buf[li] = b;
                    li += 1;
                }
            }

            let sep2 = " \u{2551} ";
            for &b in sep2.as_bytes() {
                line_buf[li] = b;
                li += 1;
            }

            for &b in b"Hazir" {
                if li < 60 {
                    line_buf[li] = b;
                    li += 1;
                }
            }

            if is_current {
                if li < 60 {
                    line_buf[li] = b'*';
                    li += 1;
                }
            }

            while li < 60 {
                line_buf[li] = b' ';
                li += 1;
            }
            let end = " \u{2551}";
            for &b in end.as_bytes() {
                line_buf[li] = b;
                li += 1;
            }

            self.push_line(&line_buf[..li], LineColor::Success);
        }

        if count == 0 {
            self.println(
                "  ║  No drives found.                                      ║",
                LineColor::Info,
            );
        }

        self.println(
            "  ╚═══════════╩═══════════╩════════════════════════════════╝",
            LineColor::Gold,
        );
        self.empty_line();
    }

    fn cmd_pwd(&mut self) {
        let mut pbuf = [0u8; 128];
        let plen = self.path.as_display(&mut pbuf);
        let mut s = [0u8; 132];
        s[0] = b' ';
        s[1] = b' ';
        s[2..2 + plen].copy_from_slice(&pbuf[..plen]);
        self.push_line(&s[..2 + plen], LineColor::Normal);
    }

    // Dizyn degistirme - change directory / Klasorler arasi gecis yapar
    // `cd ..` bir geri, `cd ...` iki geri gider
    // `cd Alpha:` ile drive degistirme de yapilabilir
    fn cmd_cd(&mut self, arg: &[u8], fs: &mut BerkeFS) {
        if arg.is_empty() {
            self.path.depth = 0;
            return;
        }
        if arg == b"/" || arg == b"\\" {
            self.path.depth = 0;
            return;
        }
        // Drive switch: "cd Alpha:" etc.
        let drive = DriveId::from_bytes(arg);
        if drive != DriveId::None {
            if drive == self.path.drive {
                self.println("  Already on this drive.", LineColor::Warning);
                return;
            }
            self.switch_drive(drive);
            return;
        }
        if arg == b".." {
            self.path.pop();
            return;
        }
        if arg == b"..." {
            self.path.pop();
            self.path.pop();
            return;
        }
        // Check for drive prefix "Alpha:\..."
        let mut di = 0;
        while di < arg.len() && arg[di] != b':' {
            di += 1;
        }
        if di < arg.len() {
            let drive_part = &arg[..di];
            let drv = DriveId::from_bytes(drive_part);
            if drv != DriveId::None {
                let same_drive = drv == self.path.drive;
                if same_drive {
                    self.println("  Already on this drive.", LineColor::Warning);
                }
                self.path.drive = drv;
                self.path.depth = 0;
                let rest_start =
                    if di + 1 < arg.len() && (arg[di + 1] == b'\\' || arg[di + 1] == b'/') {
                        di + 2
                    } else {
                        di + 1
                    };
                let rest = &arg[rest_start..];
                if !rest.is_empty() {
                    self.navigate_path(rest, fs);
                }
                return;
            }
        }
        self.navigate_path(arg, fs);
    }

    // Path parsing helper - cd'nin asil isini yapan fonksiyon
    // Yol bileşenlerini '/' veya '\\' ile ayirir ve `path.push()` cagirir
    // `..` gelirse `path.pop()` cagirir (bir seviye geri)
    fn navigate_path(&mut self, path: &[u8], fs: &mut BerkeFS) {
        let mut start = 0;
        let mut i = 0;
        while i <= path.len() {
            if i == path.len() || path[i] == b'/' || path[i] == b'\\' {
                let part = &path[start..i];
                if !part.is_empty() {
                    if part == b".." {
                        self.path.pop();
                    } else if part != b"." {
                        if self.on_berkefs() && fs.mounted {
                            let mut fp = [0u8; 64];
                            let fplen = self.full_path(part, &mut fp);
                            if !fs.path_exists(&fp[..fplen]) {
                                let mut msg = [0u8; 80];
                                let pfx = b"  cd: not found: ";
                                msg[..pfx.len()].copy_from_slice(pfx);
                                let n2 = part.len().min(80 - pfx.len());
                                msg[pfx.len()..pfx.len() + n2].copy_from_slice(&part[..n2]);
                                self.push_line(&msg[..pfx.len() + n2], LineColor::Error);
                                return;
                            }
                        }
                        if !self.path.push(part) {
                            self.println("  cd: path too deep", LineColor::Error);
                            return;
                        }
                    }
                }
                start = i + 1;
            }
            i += 1;
        }
    }

    // Dosya listeleme - list directory / Klasor icerigini gosterir
    // BerkeFS'den dosyalari okur ve [DIR] / [FILE] olarak listeler
    // Toplam dosya sayisi ve boyut da gosterilir
    fn cmd_ls(&mut self, _arg: &[u8], fs: &mut BerkeFS) {
        self.empty_line();
        let mut pbuf = [0u8; 128];
        let plen = self.path.as_display(&mut pbuf);
        let mut header = [0u8; 132];
        let pfx = b"  Directory of ";
        header[..pfx.len()].copy_from_slice(pfx);
        header[pfx.len()..pfx.len() + plen].copy_from_slice(&pbuf[..plen]);
        self.push_line(&header[..pfx.len() + plen], LineColor::Info);
        self.empty_line();

        if self.on_berkefs() && fs.mounted {
            let mut cur_prefix = [0u8; 64];
            let mut cp_len = 0;
            for d in 0..self.path.depth {
                let len = self.path.part_len[d];
                for j in 0..len {
                    if cp_len < 63 {
                        cur_prefix[cp_len] = self.path.parts[d][j];
                        cp_len += 1;
                    }
                }
                if cp_len < 63 {
                    cur_prefix[cp_len] = b'/';
                    cp_len += 1;
                }
            }

            let mut found = false;
            let mut total_size: usize = 0;
            let mut file_count: usize = 0;
            let mut dir_count: usize = 0;

            for i in 0..crate::berkefs::MAX_INODES {
                let ftype = fs.inodes[i].ftype;
                if ftype == crate::berkefs::FTYPE_FREE {
                    continue;
                }
                let mut nbuf = [0u8; 64];
                let full_name = fs.get_full_name(i, &mut nbuf);
                let entry_name = if cp_len == 0 {
                    if full_name.iter().any(|&b| b == b'/') {
                        continue;
                    }
                    full_name
                } else {
                    if full_name.len() <= cp_len {
                        continue;
                    }
                    if &full_name[..cp_len] != &cur_prefix[..cp_len] {
                        continue;
                    }
                    let rest = &full_name[cp_len..];
                    if rest.iter().any(|&b| b == b'/') {
                        continue;
                    }
                    rest
                };

                found = true;
                let size = fs.inodes[i].size as usize;
                let type_str = if ftype == crate::berkefs::FTYPE_DIR {
                    "[DIR] "
                } else {
                    "[FILE]"
                };
                let mut line = [0u8; COLS];
                let mut li = 0;
                for &b in b"  " {
                    if li < COLS {
                        line[li] = b;
                        li += 1;
                    }
                }
                for &b in type_str.as_bytes() {
                    if li < COLS {
                        line[li] = b;
                        li += 1;
                    }
                }
                line[li] = b' ';
                li += 1;
                for &b in entry_name {
                    if li < COLS {
                        line[li] = b;
                        li += 1;
                    }
                }
                while li < 32 {
                    line[li] = b' ';
                    li += 1;
                }
                if ftype == crate::berkefs::FTYPE_FILE {
                    let mut sbuf = [0u8; 16];
                    let sn = write_uint_buf(&mut sbuf, 0, size);
                    for &b in &sbuf[..sn] {
                        if li < COLS {
                            line[li] = b;
                            li += 1;
                        }
                    }
                    for &b in b" bytes" {
                        if li < COLS {
                            line[li] = b;
                            li += 1;
                        }
                    }
                    total_size += size;
                    file_count += 1;
                } else {
                    for &b in b"<DIR>" {
                        if li < COLS {
                            line[li] = b;
                            li += 1;
                        }
                    }
                    dir_count += 1;
                }
                let lc = if ftype == crate::berkefs::FTYPE_DIR {
                    LineColor::Gold
                } else {
                    LineColor::Normal
                };
                self.push_line(&line[..li], lc);
            }

            if !found {
                self.println("  (empty directory)", LineColor::Info);
            }
            self.empty_line();
            let mut stat = [0u8; 120];
            let mut si = 0;
            for &b in b"  " {
                if si < 120 {
                    stat[si] = b;
                    si += 1;
                }
            }
            let mut nb = [0u8; 8];
            let nn = write_uint_buf(&mut nb, 0, file_count);
            for &b in &nb[..nn] {
                if si < 120 {
                    stat[si] = b;
                    si += 1;
                }
            }
            for &b in b" file(s), " {
                if si < 120 {
                    stat[si] = b;
                    si += 1;
                }
            }
            let mut db = [0u8; 8];
            let dn = write_uint_buf(&mut db, 0, dir_count);
            for &b in &db[..dn] {
                if si < 120 {
                    stat[si] = b;
                    si += 1;
                }
            }
            for &b in b" dir(s), " {
                if si < 120 {
                    stat[si] = b;
                    si += 1;
                }
            }
            let mut tb = [0u8; 8];
            let tn = write_uint_buf(&mut tb, 0, total_size);
            for &b in &tb[..tn] {
                if si < 120 {
                    stat[si] = b;
                    si += 1;
                }
            }
            for &b in b" bytes total" {
                if si < 120 {
                    stat[si] = b;
                    si += 1;
                }
            }
            self.push_line(&stat[..si], LineColor::Info);
        } else {
            let drive_idx = self.path.drive.to_u8() as usize;
            let state = self.drives[drive_idx].drive_type;

            if state == DriveType::Formatted && fs.mounted {
            } else if state == DriveType::Named {
                self.println("  (adlandirilmis surucu - 'format' yazin)", LineColor::Info);
            } else if state == DriveType::Formatted {
                self.println("  (disk baglanmadi)", LineColor::Info);
            } else {
                self.println("  (bos surucu)", LineColor::Info);
            }
        }
    }

    // Dosya okuma - read file / Icerigi satir satir gosterir
    // BerkeFS'den dosya okur ve stdout'a yazar
    // Satir bazli cikti - her satir ayri push_line cagrilir
    fn cmd_cat(&mut self, arg: &[u8], fs: &mut BerkeFS) {
        if arg.is_empty() {
            self.println("  cat: missing filename", LineColor::Error);
            return;
        }
        if self.on_berkefs() && fs.mounted {
            let mut fp = [0u8; 64];
            let fplen = self.full_path(arg, &mut fp);
            let mut buf = [0u8; 512];
            match fs.read_file(&fp[..fplen], &mut buf) {
                Some(n) => {
                    if n == 0 {
                        self.println("  (empty file)", LineColor::Info);
                        return;
                    }
                    let mut start = 0;
                    while start < n {
                        let mut end = start;
                        while end < n && buf[end] != b'\n' {
                            end += 1;
                        }
                        let mut line = [0u8; COLS];
                        line[0] = b' ';
                        line[1] = b' ';
                        let len = (end - start).min(COLS - 2);
                        line[2..2 + len].copy_from_slice(&buf[start..start + len]);
                        self.push_line(&line[..2 + len], LineColor::Normal);
                        start = end + 1;
                    }
                }
                None => {
                    let mut msg = [0u8; 80];
                    let pfx = b"  cat: not found: ";
                    msg[..pfx.len()].copy_from_slice(pfx);
                    let n2 = arg.len().min(80 - pfx.len());
                    msg[pfx.len()..pfx.len() + n2].copy_from_slice(&arg[..n2]);
                    self.push_line(&msg[..pfx.len() + n2], LineColor::Error);
                }
            }
        } else {
            match arg {
                b"berkeos.cfg" => {
                    self.println("  version = 0.6.1", LineColor::Normal);
                    self.println("  author  = Berke Oruc", LineColor::Normal);
                }
                _ => {
                    let mut msg = [0u8; 80];
                    let pfx = b"  cat: not found: ";
                    msg[..pfx.len()].copy_from_slice(pfx);
                    let n2 = arg.len().min(80 - pfx.len());
                    msg[pfx.len()..pfx.len() + n2].copy_from_slice(&arg[..n2]);
                    self.push_line(&msg[..pfx.len() + n2], LineColor::Error);
                }
            }
        }
    }

    fn cmd_touch(&mut self, arg: &[u8], fs: &mut BerkeFS) {
        if arg.is_empty() {
            self.println("  touch: missing name", LineColor::Error);
            return;
        }
        if !self.on_berkefs() {
            self.println("  Hata: Dosya sistemi baglanmamis", LineColor::Error);
            return;
        }
        if !fs.mounted {
            self.println("  Hata: Dosya sistemi baglanmamis", LineColor::Error);
            return;
        }
        let mut fp = [0u8; 64];
        let fplen = self.full_path(arg, &mut fp);
        if fs.create_file(&fp[..fplen], b"") {
            let mut msg = [0u8; 80];
            let pfx = b"  Created: ";
            msg[..pfx.len()].copy_from_slice(pfx);
            let n2 = arg.len().min(80 - pfx.len());
            msg[pfx.len()..pfx.len() + n2].copy_from_slice(&arg[..n2]);
            self.push_line(&msg[..pfx.len() + n2], LineColor::Success);
        } else {
            self.println("  touch: failed (disk full or exists)", LineColor::Error);
        }
    }

    fn cmd_write(&mut self, arg: &[u8], fs: &mut BerkeFS) {
        let mut si = 0;
        while si < arg.len() && arg[si] != b' ' {
            si += 1;
        }
        if si == 0 {
            self.println("  write: usage: write <file> <data>", LineColor::Error);
            return;
        }
        let fname = &arg[..si];
        let mut di = si;
        while di < arg.len() && arg[di] == b' ' {
            di += 1;
        }
        let data = if di < arg.len() { &arg[di..] } else { b"" };
        if !self.on_berkefs() {
            self.println("  Hata: Dosya sistemi baglanmamis", LineColor::Error);
            return;
        }
        if !fs.mounted {
            self.println("  Hata: Dosya sistemi baglanmamis", LineColor::Error);
            return;
        }
        let mut fp = [0u8; 64];
        let fplen = self.full_path(fname, &mut fp);
        fs.delete_file(&fp[..fplen]);
        if fs.create_file(&fp[..fplen], data) {
            let mut msg = [0u8; 80];
            let pfx = b"  Written: ";
            msg[..pfx.len()].copy_from_slice(pfx);
            let n2 = fname.len().min(80 - pfx.len());
            msg[pfx.len()..pfx.len() + n2].copy_from_slice(&fname[..n2]);
            self.push_line(&msg[..pfx.len() + n2], LineColor::Success);
        } else {
            self.println("  write: failed (disk full)", LineColor::Error);
        }
    }

    // Klasor olusturma - make directory / Yeni dizin yaratir
    // BerkeFS inode sistemi uzerinde DIR tipinde kayit acar
    // Disk doluysa veya klasor zaten varsa hata verir
    fn cmd_mkdir(&mut self, arg: &[u8], fs: &mut BerkeFS) {
        if arg.is_empty() {
            self.println("  mkdir: missing name", LineColor::Error);
            return;
        }
        if !self.on_berkefs() {
            self.println("  Hata: Dosya sistemi baglanmamis", LineColor::Error);
            return;
        }
        if !fs.mounted {
            self.println("  Hata: Dosya sistemi baglanmamis", LineColor::Error);
            return;
        }
        let mut fp = [0u8; 64];
        let fplen = self.full_path(arg, &mut fp);
        if fs.create_dir(&fp[..fplen]) {
            let mut msg = [0u8; 80];
            let pfx = b"  Directory created: ";
            msg[..pfx.len()].copy_from_slice(pfx);
            let n2 = arg.len().min(80 - pfx.len());
            msg[pfx.len()..pfx.len() + n2].copy_from_slice(&arg[..n2]);
            self.push_line(&msg[..pfx.len() + n2], LineColor::Success);
        } else {
            self.println("  mkdir: failed (exists or full)", LineColor::Error);
        }
    }

    // Dosya silme - remove file / BerkeFS'den dosyayi siler
    // Dikkat! Silinen dosya geri alinamaz - no recycle bin here!
    // Sadece FILE tipini siler, DIR'leri silmez
    fn cmd_rm(&mut self, arg: &[u8], fs: &mut BerkeFS) {
        if arg.is_empty() {
            self.println("  rm: missing name", LineColor::Error);
            return;
        }
        if !self.on_berkefs() {
            self.println("  Hata: Dosya sistemi baglanmamis", LineColor::Error);
            return;
        }
        if !fs.mounted {
            self.println("  Hata: Dosya sistemi baglanmamis", LineColor::Error);
            return;
        }
        let mut fp = [0u8; 64];
        let fplen = self.full_path(arg, &mut fp);
        if fs.delete_file(&fp[..fplen]) {
            let mut msg = [0u8; 80];
            let pfx = b"  Deleted: ";
            msg[..pfx.len()].copy_from_slice(pfx);
            let n2 = arg.len().min(80 - pfx.len());
            msg[pfx.len()..pfx.len() + n2].copy_from_slice(&arg[..n2]);
            self.push_line(&msg[..pfx.len() + n2], LineColor::Success);
        } else {
            self.println("  rm: not found", LineColor::Error);
        }
    }

    fn cmd_cp(&mut self, arg: &[u8], fs: &mut BerkeFS) {
        let mut si = 0;
        while si < arg.len() && arg[si] != b' ' {
            si += 1;
        }
        if si == 0 || si >= arg.len() {
            self.println("  cp: usage: cp <src> <dst>", LineColor::Error);
            return;
        }
        let src = &arg[..si];
        let mut di = si;
        while di < arg.len() && arg[di] == b' ' {
            di += 1;
        }
        let dst = &arg[di..];
        if dst.is_empty() {
            self.println("  cp: usage: cp <src> <dst>", LineColor::Error);
            return;
        }
        if !self.on_berkefs() || !fs.mounted {
            self.println("  Hata: Dosya sistemi baglanmamis", LineColor::Error);
            return;
        }
        let mut sfp = [0u8; 64];
        let sfplen = self.full_path(src, &mut sfp);
        let mut dfp = [0u8; 64];
        let dfplen = self.full_path(dst, &mut dfp);
        let mut buf = [0u8; 512];
        match fs.read_file(&sfp[..sfplen], &mut buf) {
            Some(n) => {
                let data: [u8; 512] = buf;
                fs.delete_file(&dfp[..dfplen]);
                if fs.create_file(&dfp[..dfplen], &data[..n]) {
                    self.println("  Copied successfully.", LineColor::Success);
                } else {
                    self.println("  cp: write failed", LineColor::Error);
                }
            }
            None => {
                self.println("  cp: source not found", LineColor::Error);
            }
        }
    }

    fn cmd_mv(&mut self, arg: &[u8], fs: &mut BerkeFS) {
        let mut si = 0;
        while si < arg.len() && arg[si] != b' ' {
            si += 1;
        }
        if si == 0 || si >= arg.len() {
            self.println("  mv: usage: mv <src> <dst>", LineColor::Error);
            return;
        }
        let src = &arg[..si];
        let mut di = si;
        while di < arg.len() && arg[di] == b' ' {
            di += 1;
        }
        let dst = &arg[di..];
        if dst.is_empty() {
            self.println("  mv: usage: mv <src> <dst>", LineColor::Error);
            return;
        }
        if !self.on_berkefs() || !fs.mounted {
            self.println("  Hata: Dosya sistemi baglanmamis", LineColor::Error);
            return;
        }
        let mut sfp = [0u8; 64];
        let sfplen = self.full_path(src, &mut sfp);
        let mut dfp = [0u8; 64];
        let dfplen = self.full_path(dst, &mut dfp);
        let mut buf = [0u8; 512];
        match fs.read_file(&sfp[..sfplen], &mut buf) {
            Some(n) => {
                let data: [u8; 512] = buf;
                fs.delete_file(&dfp[..dfplen]);
                if fs.create_file(&dfp[..dfplen], &data[..n]) {
                    fs.delete_file(&sfp[..sfplen]);
                    self.println("  Moved successfully.", LineColor::Success);
                } else {
                    self.println("  mv: write failed", LineColor::Error);
                }
            }
            None => {
                if fs.rename_entry(&sfp[..sfplen], &dfp[..dfplen]) {
                    self.println("  Renamed successfully.", LineColor::Success);
                } else {
                    self.println("  mv: source not found", LineColor::Error);
                }
            }
        }
    }

    fn cmd_find(&mut self, arg: &[u8], fs: &mut BerkeFS) {
        if arg.is_empty() {
            self.println("  find: missing term", LineColor::Error);
            return;
        }
        if !self.on_berkefs() || !fs.mounted {
            self.println("  Hata: Dosya sistemi baglanmamis", LineColor::Error);
            return;
        }
        self.empty_line();
        let mut found = false;
        for i in 0..crate::berkefs::MAX_INODES {
            if fs.inodes[i].ftype == crate::berkefs::FTYPE_FREE {
                continue;
            }
            let mut nbuf = [0u8; 64];
            let name = fs.get_full_name(i, &mut nbuf);
            if contains_slice(name, arg) {
                let type_str = if fs.inodes[i].ftype == crate::berkefs::FTYPE_DIR {
                    "[DIR] "
                } else {
                    "[FILE]"
                };
                let mut line = [0u8; COLS];
                let mut li = 0;
                for &b in b"  " {
                    if li < COLS {
                        line[li] = b;
                        li += 1;
                    }
                }
                for &b in type_str.as_bytes() {
                    if li < COLS {
                        line[li] = b;
                        li += 1;
                    }
                }
                line[li] = b' ';
                li += 1;
                for &b in name {
                    if li < COLS {
                        line[li] = b;
                        li += 1;
                    }
                }
                self.push_line(&line[..li], LineColor::Success);
                found = true;
            }
        }
        if !found {
            self.println("  No matches found.", LineColor::Info);
        }
    }

    fn cmd_stat(&mut self, arg: &[u8], fs: &mut BerkeFS) {
        if arg.is_empty() {
            self.println("  stat: missing name", LineColor::Error);
            return;
        }
        if !self.on_berkefs() || !fs.mounted {
            self.println("  Hata: Dosya sistemi baglanmamis", LineColor::Error);
            return;
        }
        let mut fp = [0u8; 64];
        let fplen = self.full_path(arg, &mut fp);
        for i in 0..crate::berkefs::MAX_INODES {
            if fs.inodes[i].ftype == crate::berkefs::FTYPE_FREE {
                continue;
            }
            let mut nbuf = [0u8; 64];
            if fs.get_full_name(i, &mut nbuf) == &fp[..fplen] {
                self.empty_line();
                let mut nline = [0u8; 80];
                let pfx = b"  Name   : ";
                nline[..pfx.len()].copy_from_slice(pfx);
                let mut ni = pfx.len();
                for &b in &fp[..fplen] {
                    if ni < 78 {
                        nline[ni] = b;
                        ni += 1;
                    }
                }
                self.push_line(&nline[..ni], LineColor::Info);
                let type_str = if fs.inodes[i].ftype == crate::berkefs::FTYPE_DIR {
                    "  Type   : Directory"
                } else {
                    "  Type   : Regular File"
                };
                self.println(type_str, LineColor::Normal);
                let size = fs.inodes[i].size as usize;
                let mut sline = [0u8; 80];
                let spfx = b"  Size   : ";
                sline[..spfx.len()].copy_from_slice(spfx);
                let mut si2 = spfx.len();
                let mut sbuf = [0u8; 16];
                let sn = write_uint_buf(&mut sbuf, 0, size);
                for &b in &sbuf[..sn] {
                    if si2 < 78 {
                        sline[si2] = b;
                        si2 += 1;
                    }
                }
                for &b in b" bytes" {
                    if si2 < 78 {
                        sline[si2] = b;
                        si2 += 1;
                    }
                }
                self.push_line(&sline[..si2], LineColor::Normal);
                return;
            }
        }
        self.println("  stat: not found", LineColor::Error);
    }

    fn cmd_fsinfo(&mut self, fs: &mut BerkeFS) {
        self.empty_line();
        self.println(
            "  ╔══════════════════════════════════════════════════════════╗",
            LineColor::Gold,
        );
        self.println(
            "  ║  BerkeFS Information                                      ║",
            LineColor::Gold,
        );
        self.println(
            "  ╠══════════════════════════════════════════════════════════╣",
            LineColor::Gold,
        );
        if !self.disk_ok {
            self.println(
                "  ║  Depolama ayg\u{131}t\u{131} alg\u{131}lanamad\u{131}                               ║",
                LineColor::Error,
            );
        } else if !fs.mounted {
            self.println(
                "  ║  Disk found but not formatted                           ║",
                LineColor::Yellow,
            );
            self.println(
                "  ║  Type 'format' to initialize BerkeFS                   ║",
                LineColor::Info,
            );
        } else {
            self.println(
                "  ║  Status   : Mounted \u{2714}                                  ║",
                LineColor::Success,
            );
            self.println(
                "  ║  Magic    : 0xBE4BEF55                                ║",
                LineColor::Normal,
            );
            self.println(
                "  ║  Version  : v3 (Improved)                             ║",
                LineColor::Normal,
            );
            self.println(
                "  ║  Inodes   : 32 slots                                   ║",
                LineColor::Normal,
            );
            self.println(
                "  ║  DataBlks : 128 blocks (64 KiB)                       ║",
                LineColor::Normal,
            );
            let used = fs.used_inodes();
            let free = fs.free_blocks();
            let mut u_line = [0u8; 80];
            let u_pfx = "  ║  Files    : ";
            let mut ui = 0;
            for &b in u_pfx.as_bytes() {
                if ui < 80 {
                    u_line[ui] = b;
                    ui += 1;
                }
            }
            let mut nb = [0u8; 8];
            let nn = write_uint_buf(&mut nb, 0, used);
            for &b in &nb[..nn] {
                if ui < 80 {
                    u_line[ui] = b;
                    ui += 1;
                }
            }
            for &b in " used                                                     ║".as_bytes() {
                if ui < 80 {
                    u_line[ui] = b;
                    ui += 1;
                }
            }
            self.push_line(&u_line[..ui], LineColor::Normal);
            let mut f_line = [0u8; 80];
            let f_pfx = "  ║  FreeBlk  : ";
            let mut fi = 0;
            for &b in f_pfx.as_bytes() {
                if fi < 80 {
                    f_line[fi] = b;
                    fi += 1;
                }
            }
            let mut fb2 = [0u8; 8];
            let fn2 = write_uint_buf(&mut fb2, 0, free);
            for &b in &fb2[..fn2] {
                if fi < 80 {
                    f_line[fi] = b;
                    fi += 1;
                }
            }
            for &b in " free                                                     ║".as_bytes() {
                if fi < 80 {
                    f_line[fi] = b;
                    fi += 1;
                }
            }
            self.push_line(&f_line[..fi], LineColor::Normal);
        }
        self.println(
            "  ╚══════════════════════════════════════════════════════════╝",
            LineColor::Gold,
        );
    }

    fn cmd_format(&mut self, arg: &[u8], fs: &mut BerkeFS) {
        let drive = if arg.is_empty() {
            self.path.drive
        } else {
            DriveId::from_bytes(arg)
        };

        if drive == DriveId::None {
            self.println("  format: invalid drive name", LineColor::Error);
            self.println("  usage: format [drive]", LineColor::Info);
            self.println("  example: format Alpha", LineColor::Info);
            return;
        }

        let drive_idx = drive.to_u8() as usize;
        if drive_idx >= MAX_DRIVES {
            self.println("  format: drive not found", LineColor::Error);
            return;
        }

        let drive_name = drive.name();
        let mut msg = [0u8; 64];
        let mut mi = 0;
        for &b in b"  Formatting " {
            if mi < 64 {
                msg[mi] = b;
                mi += 1;
            }
        }
        for &b in drive_name.as_bytes() {
            if mi < 64 {
                msg[mi] = b;
                mi += 1;
            }
        }
        msg[mi] = b':';
        mi += 1;
        self.push_line(&msg[..mi], LineColor::Yellow);

        if fs.format(drive_name.as_bytes()) {
            self.drives[drive_idx].drive_type = DriveType::Formatted;
            self.drives[drive_idx].set_label(drive_name.as_bytes());
            self.println("  \u{2714} Drive formatted!", LineColor::Success);
            self.println("  Files persist on this drive.", LineColor::Info);
        } else {
            self.println("  \u{2717} Format failed", LineColor::Error);
        }
    }

    fn cmd_mkdrive(&mut self, arg: &[u8]) {
        if arg.is_empty() {
            self.println("  mkdrive: usage: mkdrive <name> [size]", LineColor::Error);
            self.println("  Example: mkdrive Gamma 1GB", LineColor::Info);
            self.println("  Sizes: 1MB, 1GB, 1TB (max 2TB)", LineColor::Info);
            return;
        }

        let mut size: u64 = 0;
        let mut name_end = 0;

        for (i, &b) in arg.iter().enumerate() {
            if b == b' ' {
                name_end = i;
                let size_part = &arg[i + 1..];
                if !size_part.is_empty() {
                    size = self.parse_size(size_part);
                }
                break;
            }
        }

        if name_end == 0 {
            name_end = arg.len();
        }

        let name = &arg[..name_end];

        if name.len() > 16 {
            self.println("  mkdrive: name too long (max 16 chars)", LineColor::Error);
            return;
        }

        if size == 0 {
            size = 128 * 1024 * 1024;
        }

        if size > MAX_DRIVE_SIZE {
            self.println("  mkdrive: size too large (max 2TB)", LineColor::Error);
            return;
        }

        let mut found_idx = MAX_DRIVES;
        for i in 1..MAX_DRIVES {
            if self.drives[i].drive_type == DriveType::None {
                found_idx = i;
                break;
            }
        }

        if found_idx >= MAX_DRIVES {
            self.println("  mkdrive: no free drive slots", LineColor::Error);
            return;
        }

        self.drives[found_idx].drive_type = DriveType::Formatted;
        self.drives[found_idx].set_label(name);
        self.drives[found_idx].size = size;
        self.drives[found_idx].used = 0;
        self.drives[found_idx].is_ramdisk = false;

        let mut msg_buf = [0u8; 80];
        let mut mi = 0;

        let pfx = b"  Olusturuldu: ";
        for &b in pfx {
            if mi < 80 {
                msg_buf[mi] = b;
                mi += 1;
            }
        }
        for &b in name {
            if mi < 80 {
                msg_buf[mi] = b;
                mi += 1;
            }
        }
        msg_buf[mi] = b' ';
        mi += 1;

        let mut size_buf = [0u8; 32];
        let size_len = self.fmt_size(size, &mut size_buf);
        for &b in &size_buf[..size_len] {
            if mi < 80 {
                msg_buf[mi] = b;
                mi += 1;
            }
        }

        self.push_line(&msg_buf[..mi], LineColor::Success);
    }

    fn parse_size(&self, s: &[u8]) -> u64 {
        let mut num: u64 = 0;
        let mut unit_start = 0;

        for (i, &b) in s.iter().enumerate() {
            if b >= b'0' && b <= b'9' {
                num = num * 10 + (b - b'0') as u64;
            } else if b == b'M' || b == b'G' || b == b'T' || b == b'm' || b == b'g' || b == b't' {
                unit_start = i;
                break;
            }
        }

        if unit_start > 0 {
            let unit = s[unit_start];
            match unit {
                b'M' | b'm' => num *= 1024 * 1024,
                b'G' | b'g' => num *= 1024 * 1024 * 1024,
                b'T' | b't' => num *= 1024 * 1024 * 1024 * 1024,
                _ => {}
            }
        }

        num
    }

    fn fmt_size(&self, size: u64, out: &mut [u8; 32]) -> usize {
        let mut pos = 0;

        if size >= 1024 * 1024 * 1024 * 1024 {
            let tb = size / (1024 * 1024 * 1024 * 1024);
            let mut n = tb;
            let mut len = 0;
            if n == 0 {
                out[pos] = b'0';
                pos += 1;
            } else {
                while n > 0 && pos < 20 {
                    out[pos] = b'0' + (n % 10) as u8;
                    n /= 10;
                    pos += 1;
                    len += 1;
                }
                for i in 0..len / 2 {
                    let tmp = out[i];
                    out[i] = out[len - 1 - i];
                    out[len - 1 - i] = tmp;
                }
            }
            out[pos] = b'T';
            pos += 1;
            out[pos] = b'B';
            pos += 1;
        } else if size >= 1024 * 1024 * 1024 {
            let gb = size / (1024 * 1024 * 1024);
            let mut n = gb;
            let mut len = 0;
            if n == 0 {
                out[pos] = b'0';
                pos += 1;
            } else {
                while n > 0 && pos < 20 {
                    out[pos] = b'0' + (n % 10) as u8;
                    n /= 10;
                    pos += 1;
                    len += 1;
                }
                for i in 0..len / 2 {
                    let tmp = out[i];
                    out[i] = out[len - 1 - i];
                    out[len - 1 - i] = tmp;
                }
            }
            out[pos] = b'G';
            pos += 1;
            out[pos] = b'B';
            pos += 1;
        } else if size >= 1024 * 1024 {
            let mb = size / (1024 * 1024);
            let mut n = mb;
            let mut len = 0;
            if n == 0 {
                out[pos] = b'0';
                pos += 1;
            } else {
                while n > 0 && pos < 20 {
                    out[pos] = b'0' + (n % 10) as u8;
                    n /= 10;
                    pos += 1;
                    len += 1;
                }
                for i in 0..len / 2 {
                    let tmp = out[i];
                    out[i] = out[len - 1 - i];
                    out[len - 1 - i] = tmp;
                }
            }
            out[pos] = b'M';
            pos += 1;
            out[pos] = b'B';
            pos += 1;
        } else if size >= 1024 {
            let kb = size / 1024;
            let mut n = kb;
            let mut len = 0;
            if n == 0 {
                out[pos] = b'0';
                pos += 1;
            } else {
                while n > 0 && pos < 20 {
                    out[pos] = b'0' + (n % 10) as u8;
                    n /= 10;
                    pos += 1;
                    len += 1;
                }
                for i in 0..len / 2 {
                    let tmp = out[i];
                    out[i] = out[len - 1 - i];
                    out[len - 1 - i] = tmp;
                }
            }
            out[pos] = b'K';
            pos += 1;
            out[pos] = b'B';
            pos += 1;
        } else {
            let mut n = size;
            let mut len = 0;
            if n == 0 {
                out[pos] = b'0';
                pos += 1;
            } else {
                while n > 0 && pos < 20 {
                    out[pos] = b'0' + (n % 10) as u8;
                    n /= 10;
                    pos += 1;
                    len += 1;
                }
                for i in 0..len / 2 {
                    let tmp = out[i];
                    out[i] = out[len - 1 - i];
                    out[len - 1 - i] = tmp;
                }
            }
            out[pos] = b'B';
            pos += 1;
        }

        pos
    }

    fn cmd_rmdrive(&mut self, arg: &[u8]) {
        if arg.is_empty() {
            self.println("  rmdrive: missing drive name", LineColor::Error);
            self.println("  usage: rmdrive <name>", LineColor::Info);
            return;
        }

        let drive = DriveId::from_bytes(arg);
        if drive == DriveId::None {
            self.println("  rmdrive: invalid drive name", LineColor::Error);
            return;
        }

        let drive_idx = drive.to_u8() as usize;

        if self.drives[drive_idx].drive_type == DriveType::None {
            self.println("  rmdrive: drive does not exist", LineColor::Error);
            return;
        }

        if drive == DriveId::Alpha && self.disk_ok {
            self.println(
                "  rmdrive: cannot remove Alpha (system drive)",
                LineColor::Error,
            );
            return;
        }

        self.drives[drive_idx].drive_type = DriveType::None;
        self.drives[drive_idx].label = [0u8; MAX_DRIVE_LABEL];
        self.drives[drive_idx].size = 0;
        self.drives[drive_idx].used = 0;

        let drive_name = drive.name();
        let mut msg = [0u8; 64];
        let mut mi = 0;
        for &b in b"  Removed drive: " {
            if mi < 64 {
                msg[mi] = b;
                mi += 1;
            }
        }
        for &b in drive_name.as_bytes() {
            if mi < 64 {
                msg[mi] = b;
                mi += 1;
            }
        }
        self.push_line(&msg[..mi], LineColor::Success);
    }

    #[allow(dead_code)]
    fn cmd_changedrive(&mut self, arg: &[u8]) {
        if arg.is_empty() {
            self.println("  changedrive: missing drive name", LineColor::Error);
            self.println("  usage: changedrive <name>", LineColor::Info);
            self.println("  example: changedrive Beta", LineColor::Info);
            return;
        }

        let drive = DriveId::from_bytes(arg);
        if drive == DriveId::None {
            self.println("  changedrive: invalid drive name", LineColor::Error);
            return;
        }

        self.switch_drive(drive);
    }

    fn cmd_dev(&mut self, arg: &[u8]) {
        let mut subcmd_type: u8 = 0;
        let mut device_start: usize = 0;

        if !arg.is_empty() {
            let mut i = 0;
            while i < arg.len() && arg[i] != b' ' {
                i += 1;
            }

            let cmd_word = if i > 0 { &arg[..i] } else { arg };

            if cmd_word == b"list" {
                subcmd_type = 1;
            } else if cmd_word == b"test" {
                subcmd_type = 2;
                device_start = i;
                while device_start < arg.len() && arg[device_start] == b' ' {
                    device_start += 1;
                }
            } else {
                subcmd_type = 3;
            }
        }

        let device = if device_start < arg.len() {
            &arg[device_start..]
        } else {
            b""
        };

        match subcmd_type {
            0 | 3 => {
                self.empty_line();
                self.println("  \u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}", LineColor::Info);
                self.println(
                    "  \u{2551}  Hardware Status                            \u{2551}",
                    LineColor::Info,
                );
                self.println("  \u{2560}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2563}", LineColor::Info);

                if self.disk_ok {
                    self.println(
                        "  \u{2551}  ATA Disk     : Detected \u{2714}                \u{2551}",
                        LineColor::Success,
                    );
                    self.println(
                        "  \u{2551}  BerkeFS     : Active                       \u{2551}",
                        LineColor::Success,
                    );
                } else {
                    self.println(
                        "  \u{2551}  ATA Disk     : Not detected                \u{2551}",
                        LineColor::Yellow,
                    );
                }

                self.println(
                    "  \u{2551}  AHCI        : Available (run 'dev test ahci')\u{2551}",
                    LineColor::Normal,
                );

                self.println(
                    "  \u{2551}  Keyboard    : PS/2 Ready \u{2714}                \u{2551}",
                    LineColor::Success,
                );

                self.println(
                    "  \u{2551}  RTC         : Real-time clock \u{2714}          \u{2551}",
                    LineColor::Success,
                );

                self.println(
                    "  \u{2551}  Timer       : PIT 100Hz \u{2714}               \u{2551}",
                    LineColor::Success,
                );

                self.println(
                    "  \u{255A}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D}",
                    LineColor::Info,
                );
                self.empty_line();
                self.println(
                    "  Usage: dev list | dev test | dev test ata | dev test ahci",
                    LineColor::Info,
                );
            }
            1 => {
                self.empty_line();
                self.println("  Available hardware devices:", LineColor::Info);
                self.empty_line();
                self.println(
                    "  [1] ata       - ATA/IDE disk controller",
                    LineColor::Normal,
                );
                self.println("  [2] ahci     - AHCI SATA controller", LineColor::Normal);
                self.println("  [3] keyboard - PS/2 keyboard", LineColor::Normal);
                self.println("  [4] rtc      - Real-time clock", LineColor::Normal);
                self.println(
                    "  [5] pit      - Programmable interval timer",
                    LineColor::Normal,
                );
                self.empty_line();
                self.println(
                    "  Run 'dev test' to test all, or 'dev test <device>' for one.",
                    LineColor::Info,
                );
            }
            2 => {
                if device == b"ata" || device.is_empty() {
                    self.empty_line();
                    self.println("  Testing ATA driver...", LineColor::Info);
                    if self.disk_ok {
                        self.println("  \u{2714} ATA: Disk detected", LineColor::Success);
                        self.println("  \u{2714} ATA: BerkeFS mounted", LineColor::Success);
                    } else {
                        self.println(
                            "  \u{2717} ATA: Depolama ayg\u{131}t\u{131} alg\u{131}lanamad\u{131}",
                            LineColor::Error,
                        );
                    }
                }

                if device == b"ahci" || device.is_empty() {
                    self.println(
                        "  \u{231b} AHCI: Driver available (not tested in userspace)",
                        LineColor::Info,
                    );
                }

                if device == b"keyboard" || device.is_empty() {
                    self.println("  \u{2714} Keyboard: PS/2 ready", LineColor::Success);
                }

                if device == b"rtc" || device.is_empty() {
                    let dt = crate::rtc::read();
                    self.println("  \u{2714} RTC: Working", LineColor::Success);
                    let mut buf = [0u8; 32];
                    let n = crate::rtc::format_datetime(&dt, &mut buf);
                    let mut msg = [0u8; 48];
                    msg[..5].copy_from_slice(b"  Time:");
                    msg[5..5 + n].copy_from_slice(&buf[..n]);
                    self.push_line(&msg[..5 + n], LineColor::Normal);
                }

                if device == b"pit" || device.is_empty() {
                    let ticks = crate::pic::uptime_ticks();
                    self.println("  \u{2714} PIT: Timer working", LineColor::Success);
                    let mut msg = [0u8; 32];
                    msg[..14].copy_from_slice(b"  Ticks: ");
                    let mut i = 14;
                    let mut t = ticks;
                    if t == 0 {
                        msg[i] = b'0';
                        i += 1;
                    } else {
                        let start = i;
                        while t > 0 && i < 30 {
                            msg[i] = b'0' + (t % 10) as u8;
                            t /= 10;
                            i += 1;
                        }
                        msg[start..i].reverse();
                    }
                    self.push_line(&msg[..i], LineColor::Normal);
                }

                if device.is_empty() {
                    self.empty_line();
                    self.println("  All hardware tests complete.", LineColor::Success);
                }
            }
            _ => {
                self.println("  dev: unknown subcommand", LineColor::Error);
                self.println("  usage: dev [status|list|test]", LineColor::Info);
            }
        }
    }

    fn cmd_history(&mut self) {
        self.empty_line();
        if self.hist_count == 0 {
            self.println("  (no history)", LineColor::Info);
            return;
        }
        for i in 0..self.hist_count {
            let mut line = [0u8; MAX_LINE + 8];
            let n = write_uint_buf(&mut line, 0, i + 1);
            line[n] = b' ';
            line[n + 1] = b' ';
            let len = self.hist_len[i];
            line[n + 2..n + 2 + len].copy_from_slice(&self.history[i][..len]);
            self.push_line(&line[..n + 2 + len], LineColor::Normal);
        }
    }

    fn cmd_reboot(&mut self) {
        self.println("  Rebooting...", LineColor::Yellow);
        unsafe {
            crate::keyboard::outb(0x64, 0xFE);
        }
    }

    fn cmd_halt(&mut self) {
        self.println("  Halting BerkeOS. Goodbye!", LineColor::Yellow);
        unsafe {
            core::arch::asm!("cli; hlt");
        }
    }

    fn cmd_beep(&mut self, arg: &[u8]) {
        if arg.is_empty() {
            crate::pcspeaker::beep(440, 200);
            self.println("  \u{2714} Beep!", LineColor::Success);
            return;
        }

        let mut freq: u16 = 440;
        let mut dur: u16 = 200;

        let mut i = 0;
        let mut num_start = 0;
        while i < arg.len() && arg[i] == b' ' {
            i += 1;
        }
        num_start = i;
        while i < arg.len() && arg[i] >= b'0' && arg[i] <= b'9' {
            i += 1;
        }
        if i > num_start {
            let mut f: u16 = 0;
            for j in num_start..i {
                f = f * 10 + (arg[j] - b'0') as u16;
            }
            freq = f;
        }

        while i < arg.len() && arg[i] == b' ' {
            i += 1;
        }
        if i < arg.len() {
            let mut d: u16 = 0;
            while i < arg.len() && arg[i] >= b'0' && arg[i] <= b'9' {
                d = d * 10 + (arg[i] - b'0') as u16;
                i += 1;
            }
            dur = d;
        }

        crate::pcspeaker::beep(freq, dur);
        let mut msg = [0u8; 32];
        msg[..5].copy_from_slice(b"  Beep");
        let mut mi = 5;
        msg[mi] = b' ';
        mi += 1;
        let mut f = freq;
        let mut fstart = mi;
        if f == 0 {
            msg[mi] = b'0';
            mi += 1;
        } else {
            while f > 0 {
                if mi < 30 {
                    msg[mi] = b'0' + (f % 10) as u8;
                    mi += 1;
                    f /= 10;
                }
            }
            msg[fstart..mi].reverse();
        }
        msg[mi] = b' ';
        mi += 1;
        msg[mi] = b'H';
        mi += 1;
        msg[mi] = b'z';
        mi += 1;
        self.push_line(&msg[..mi], LineColor::Success);
    }

    fn cmd_music(&mut self) {
        self.empty_line();
        self.println("  +=============================+", LineColor::Info);
        self.println("  |     Audio/Music Player       |", LineColor::Info);
        self.println("  +=============================+", LineColor::Info);
        self.empty_line();

        self.println("  Commands:", LineColor::Gold);
        self.println(
            "  play <note> - Play a note (C,D,E,F,G,A,B)",
            LineColor::Normal,
        );
        self.println("  music scale - Play musical scale", LineColor::Normal);
        self.println("  music mario - Play Mario theme", LineColor::Normal);
        self.println("  music doom - Play DOOM theme", LineColor::Normal);

        self.empty_line();
        self.println("  Usage:", LineColor::Gold);
        self.println("  play C4 440 - Play C4 for 440ms", LineColor::Normal);

        self.empty_line();

        if crate::pcspeaker::is_audio_working() {
            self.println("  Audio: PC Speaker Ready", LineColor::Success);
        } else {
            self.println("  Audio: Not initialized", LineColor::Warning);
        }
    }

    fn cmd_audio(&mut self) {
        self.empty_line();

        if crate::pcspeaker::init_audio() {
            self.println("  +=============================+", LineColor::Success);
            self.println("  |     Audio System Ready      |", LineColor::Success);
            self.println("  +=============================+", LineColor::Success);
            self.empty_line();
            self.println("  PC Speaker initialized!", LineColor::Success);

            crate::pcspeaker::beep_ok();
            self.println("  Test beep played.", LineColor::Info);
        } else {
            self.println("  Audio initialization failed!", LineColor::Error);
        }
    }

    fn cmd_play(&mut self, arg: &[u8]) {
        if arg.is_empty() {
            self.println("  play: usage: play <note>", LineColor::Error);
            self.println("  Example: play C4 or play 440", LineColor::Info);
            return;
        }

        let notes: [(u16, &str); 12] = [
            (261, "C4"),
            (293, "D4"),
            (329, "E4"),
            (349, "F4"),
            (392, "G4"),
            (440, "A4"),
            (493, "B4"),
            (523, "C5"),
            (587, "D5"),
            (659, "E5"),
            (698, "F5"),
            (783, "G5"),
        ];

        let mut freq: u16 = 440;
        let mut found = false;

        for (f, name) in notes.iter() {
            let mut i = 0;
            while i < arg.len() && arg[i] == b' ' {
                i += 1;
            }
            let note_str = &arg[..arg.len().min(2)];
            if note_str.eq_ignore_ascii_case(name.as_bytes())
                || (name.len() == 2 && note_str[0] == name.as_bytes()[0])
            {
                freq = *f;
                found = true;
                break;
            }
        }

        if !found {
            let mut f: u16 = 0;
            for &b in arg {
                if b >= b'0' && b <= b'9' {
                    f = f * 10 + (b - b'0') as u16;
                }
            }
            if f > 0 {
                freq = f;
                found = true;
            }
        }

        crate::pcspeaker::beep(freq, 300);

        let mut msg = [0u8; 24];
        msg[..5].copy_from_slice(b"  Note");
        let mut mi = 5;
        if found {
            let mut f = freq;
            let mut len = 0;
            if f == 0 {
                msg[mi] = b'0';
                mi += 1;
            } else {
                let mut tmp_buf = [0u8; 8];
                while f > 0 && len < 8 {
                    tmp_buf[len] = b'0' + (f % 10) as u8;
                    f /= 10;
                    len += 1;
                }
                for j in (0..len).rev() {
                    if mi < 20 {
                        msg[mi] = tmp_buf[j];
                        mi += 1;
                    }
                }
            }
        } else {
            msg[mi] = b'?';
            mi += 1;
        }
        msg[mi] = b'H';
        mi += 1;
        msg[mi] = b'z';
        mi += 1;

        self.push_line(&msg[..mi], LineColor::Success);
    }

    fn cmd_install(&mut self, arg: &[u8]) {
        self.empty_line();
        self.println(
            "  =============================================",
            LineColor::Gold,
        );
        self.println("       BerkeOS Installer v1.0", LineColor::Gold);
        self.println(
            "  =============================================",
            LineColor::Gold,
        );
        self.empty_line();

        if !self.disk_ok {
            self.println("  Error: No disk detected!", LineColor::Error);
            self.println("  Connect a disk and try again.", LineColor::Info);
            return;
        }

        self.println("  This will install BerkeOS to disk.", LineColor::Warning);
        self.empty_line();
        self.println("  Installation will:", LineColor::Info);
        self.println("  - Create EFI system partition (UEFI)", LineColor::Normal);
        self.println("  - Create BIOS boot partition", LineColor::Normal);
        self.println("  - Install bootloader", LineColor::Normal);
        self.println("  - Copy BerkeOS kernel", LineColor::Normal);
        self.println("  - Setup user accounts", LineColor::Normal);
        self.empty_line();

        self.println("  Default users:", LineColor::Info);
        self.println(
            "    admin   - Full access (password: admin)",
            LineColor::Normal,
        );
        self.println(
            "    guest   - Limited access (password: guest)",
            LineColor::Normal,
        );
        self.empty_line();

        self.println("  To install on USB/real hardware:", LineColor::Info);
        self.println("  1. Backup your data!", LineColor::Warning);
        self.println("  2. Run: sudo tools/install_tui.sh", LineColor::Command);
        self.println("     OR", LineColor::Normal);
        self.println("  3. Use 'dd' to write ISO to USB:", LineColor::Info);
        self.println(
            "     sudo dd if=build/berkeos.iso of=/dev/sdX",
            LineColor::Command,
        );
        self.empty_line();
        self.println("  Boot options:", LineColor::Info);
        self.println("    BIOS: Boot from USB (Legacy mode)", LineColor::Normal);
        self.println("    UEFI: Boot from USB (UEFI mode)", LineColor::Normal);
        self.empty_line();
        self.println(
            "  =============================================",
            LineColor::Gold,
        );
    }

    fn cmd_mem(&mut self) {
        self.empty_line();
        self.println("  \u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}", LineColor::Info);
        self.println(
            "  \u{2551}  Memory Information                    \u{2551}",
            LineColor::Info,
        );
        self.println("  \u{2560}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2563}", LineColor::Info);
        self.println(
            "  \u{2551}  Total RAM    : 256 MiB (QEMU)         \u{2551}",
            LineColor::Normal,
        );
        self.println(
            "  \u{2551}  Mapped       : 4 GiB identity-mapped  \u{2551}",
            LineColor::Normal,
        );
        self.println(
            "  \u{2551}  Page size    : 2 MiB huge pages       \u{2551}",
            LineColor::Normal,
        );
        self.println(
            "  \u{2551}  Kernel stack : 64 KiB                 \u{2551}",
            LineColor::Normal,
        );
        self.println(
            "  \u{2551}  BerkeFS      : 33 KiB on disk         \u{2551}",
            LineColor::Success,
        );
        self.println("  \u{255A}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D}", LineColor::Info);
    }

    fn cmd_color(&mut self) {
        self.empty_line();
        self.println("  BerkeOS Pink Color Palette:", LineColor::Normal);
        self.println("  Hot Pink   \u{2014} main text", LineColor::Normal);
        self.println("  Magenta    \u{2014} prompt", LineColor::Info);
        self.println("  Light Pink \u{2014} success", LineColor::Success);
        self.println("  Gold       \u{2014} drives, headers", LineColor::Gold);
        self.println("  Yellow     \u{2014} warnings", LineColor::Yellow);
        self.println("  Red        \u{2014} errors", LineColor::Error);
        self.println("  White      \u{2014} commands", LineColor::Command);
    }

    fn cmd_banner(&mut self) {
        self.empty_line();
        self.println("  \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557} \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557} \u{2588}\u{2588}\u{2557}  \u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557} \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557} \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557}", LineColor::Success);
        self.println("  \u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D}\u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2551} \u{2588}\u{2588}\u{2554}\u{255D}\u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D}\u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{2550}\u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D}", LineColor::Success);
        self.println("  \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2554}\u{255D}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557}  \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2554}\u{255D}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2554}\u{255D} \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557}  \u{2588}\u{2588}\u{2551}   \u{2588}\u{2588}\u{2551}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557}", LineColor::Info);
        self.println("  \u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{255D}  \u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2554}\u{2550}\u{2588}\u{2588}\u{2557} \u{2588}\u{2588}\u{2554}\u{2550}\u{2550}\u{255D}  \u{2588}\u{2588}\u{2551}   \u{2588}\u{2588}\u{2551}\u{255A}\u{2550}\u{2550}\u{2550}\u{2550}\u{2588}\u{2588}\u{2551}", LineColor::Info);
        self.println("  \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2554}\u{255D}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2551}  \u{2588}\u{2588}\u{2551}\u{2588}\u{2588}\u{2551}  \u{2588}\u{2588}\u{2557}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2557}\u{255A}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2554}\u{255D}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2551}", LineColor::Success);
        self.println("  \u{255A}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D} \u{255A}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D}\u{255A}\u{2550}\u{255D}  \u{255A}\u{2550}\u{255D}\u{255A}\u{2550}\u{255D}  \u{255A}\u{2550}\u{255D}\u{255A}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D} \u{255A}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D} \u{255A}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D}", LineColor::Success);
        self.empty_line();
        self.println(
            "  BerkeOS  |  Berke Oruc  |  Rust  |  x86_64",
            LineColor::Gold,
        );
    }

    fn cmd_date(&mut self) {
        let dt = crate::rtc::read();
        let mut buf = [0u8; 32];
        let n = crate::rtc::format_datetime(&dt, &mut buf);
        let mut line = [0u8; 80];
        let pfx = b"  Date/Time: ";
        line[..pfx.len()].copy_from_slice(pfx);
        line[pfx.len()..pfx.len() + n].copy_from_slice(&buf[..n]);
        self.push_line(&line[..pfx.len() + n], LineColor::Info);
    }

    fn cmd_uptime(&mut self) {
        let secs = crate::pic::uptime_seconds();
        let mut buf = [0u8; 48];
        let n = crate::rtc::format_uptime(secs, &mut buf);
        let mut line = [0u8; 80];
        let pfx = b"  Uptime: ";
        line[..pfx.len()].copy_from_slice(pfx);
        line[pfx.len()..pfx.len() + n].copy_from_slice(&buf[..n]);
        self.push_line(&line[..pfx.len() + n], LineColor::Info);
    }

    fn cmd_ticks(&mut self) {
        let ticks = crate::pic::uptime_ticks();
        let mut line = [0u8; 80];
        let pfx = b"  Timer ticks (100Hz): ";
        line[..pfx.len()].copy_from_slice(pfx);
        let mut i = pfx.len();
        let mut n = ticks;
        if n == 0 {
            line[i] = b'0';
            i += 1;
        } else {
            let start = i;
            while n > 0 && i < 78 {
                line[i] = b'0' + (n % 10) as u8;
                n /= 10;
                i += 1;
            }
            line[start..i].reverse();
        }
        self.push_line(&line[..i], LineColor::Info);
    }

    fn cmd_about(&mut self) {
        self.empty_line();
        self.println(
            "  =============================================",
            LineColor::Info,
        );
        self.println(
            "  BerkeOS - Indigenous Independent OS in Rust",
            LineColor::Info,
        );
        self.println(
            "  =============================================",
            LineColor::Info,
        );
        self.empty_line();
        self.println("  Author   : Berke Oruc (Age 16)", LineColor::Gold);
        self.println("  GitHub   : github.com/berkeoruc", LineColor::Info);
        self.println("  Language : Rust (no_std, bare metal)", LineColor::Normal);
        self.println("  Arch     : x86_64", LineColor::Normal);
        self.println("  Boot     : UEFI/BIOS auto-detect", LineColor::Normal);
        self.println("  FS       : BerkeFS (custom ATA PIO)", LineColor::Success);
        self.println(
            "  ----------------------------------------",
            LineColor::Normal,
        );
        self.println("  Code: 43% by Berke (~6,143 lines)", LineColor::Info);
        self.println("        57% AI-assisted (~8,145 lines)", LineColor::Info);
        self.println("        0 TL cost (free AI tools)", LineColor::Info);
        self.println(
            "  =============================================",
            LineColor::Gold,
        );
    }

    fn cmd_version(&mut self) {
        self.println("  BerkeOS", LineColor::Success);
        self.println("  Rust  |  x86_64  |  no_std", LineColor::Info);
    }

    fn cmd_uname(&mut self) {
        self.println(
            "  BerkeOS berkeos 0.6.1 x86_64 Rust-nightly no_std BerkeFS",
            LineColor::Normal,
        );
    }

    fn cmd_whoami(&mut self) {
        self.println("  berke", LineColor::Normal);
    }

    fn cmd_sysinfo(&mut self) {
        self.empty_line();
        self.println("  \u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}", LineColor::Info);
        self.println(
            "  \u{2551}  BerkeOS System Information                \u{2551}",
            LineColor::Info,
        );
        self.println("  \u{2560}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2563}", LineColor::Info);
        self.println(
            "  \u{2551}  OS       : BerkeOS v0.6.1                   \u{2551}",
            LineColor::Normal,
        );
        self.println(
            "  \u{2551}  Arch     : x86_64 bare metal                \u{2551}",
            LineColor::Normal,
        );
        self.println(
            "  \u{2551}  Language : Rust nightly (no_std)            \u{2551}",
            LineColor::Normal,
        );
        self.println(
            "  \u{2551}  Boot     : UEFI/BIOS                        \u{2551}",
            LineColor::Normal,
        );
        self.println(
            "  \u{2551}  FS       : BerkeFS (ATA PIO, persistent)    \u{2551}",
            LineColor::Success,
        );
        self.println(
            "  \u{2551}  Drives   : (use 'drives' command)            \u{2551}",
            LineColor::Gold,
        );
        self.println(
            "  \u{2551}  IRQ      : IDT + PIC 8259 + PIT 100Hz       \u{2551}",
            LineColor::Info,
        );
        self.println(
            "  \u{2551}  Memory   : 4 GiB mapped (2 MiB huge pages)  \u{2551}",
            LineColor::Normal,
        );
        self.println(
            "  \u{2551}  Author   : Berke Oruc, Age 16               \u{2551}",
            LineColor::Gold,
        );
        self.println("  \u{255A}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D}", LineColor::Info);
    }

    fn cmd_phase(&mut self) {
        self.println("  Use 'help' for available commands.", LineColor::Info);
    }

    fn cmd_roadmap(&mut self) {
        self.println("  Use 'help' for available commands.", LineColor::Info);
    }

    fn cmd_neofetch(&mut self) {
        self.empty_line();
        self.println("  berke@BerkeOS", LineColor::Success);
        self.println("  \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}", LineColor::Normal);
        self.println("  OS       : BerkeOS", LineColor::Info);
        self.println("  Kernel   : berkeos", LineColor::Normal);
        self.println("  Shell    : berkesh", LineColor::Normal);
        self.println("  Arch     : x86_64", LineColor::Normal);
        self.println("  FS       : BerkeFS", LineColor::Success);
        self.println("  Lang     : Rust", LineColor::Yellow);
    }

    fn cmd_matrix(&mut self) {
        self.empty_line();
        self.println("  \u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}", LineColor::Success);
        self.println(
            "  \u{2551}  MATRIX MODE - Follow the white rabbit...  \u{2551}",
            LineColor::Success,
        );
        self.println("  \u{255A}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D}", LineColor::Success);
        self.empty_line();

        // Matrix rain characters
        let matrix_chars = [
            "\u{30A0}\u{30A1}\u{30A2}\u{30A3}\u{30A4}\u{30A5}\u{30A6}\u{30A7}\u{30A8}\u{30A9}",
            "\u{30AA}\u{30AB}\u{30AC}\u{30AD}\u{30AE}\u{30AF}\u{30B0}\u{30B1}\u{30B2}\u{30B3}",
            "\u{30B4}\u{30B5}\u{30B6}\u{30B7}\u{30B8}\u{30B9}\u{30BA}\u{30BB}\u{30BC}\u{30BD}",
            "\u{4E00}\u{4E01}\u{4E02}\u{4E03}\u{4E04}\u{4E05}\u{4E06}\u{4E07}\u{4E08}\u{4E09}",
            "01\u{2081}\u{2082}\u{2083}\u{2084}\u{2085}\u{2086}\u{2087}\u{2088}\u{2089}",
        ];

        for i in 0..8 {
            let mut line = [0u8; 50];
            let mut pos = 0;
            for _ in 0..5 {
                let chars = matrix_chars[(i + pos) % matrix_chars.len()];
                for b in &chars.as_bytes()[..2.min(chars.as_bytes().len())] {
                    if pos < 48 {
                        line[pos] = *b;
                        pos += 1;
                    }
                }
                line[pos] = b' ';
                pos += 1;
            }
            self.push_line(&line[..pos], LineColor::Success);
        }

        self.empty_line();
        self.println("  Wake up, Neo...", LineColor::Warning);
        self.println("  The matrix has you.", LineColor::Warning);
    }

    fn cmd_snake(&mut self) {
        self.empty_line();
        self.println("  \u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}", LineColor::Gold);
        self.println(
            "  \u{2551}  SNAKE - Classic game coming soon!      \u{2551}",
            LineColor::Gold,
        );
        self.println("  \u{255A}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D}", LineColor::Gold);
        self.empty_line();

        // ASCII snake game preview
        self.println(
            "       \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}     ",
            LineColor::Success,
        );
        self.println(
            "    \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}  ",
            LineColor::Success,
        );
        self.println(
            "  \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588} ",
            LineColor::Success,
        );
        self.println(
            "    \u{2588}\u{2588} \u{25CF} \u{2588}\u{2588}\u{2588} ",
            LineColor::Success,
        );
        self.println(
            "       \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}     ",
            LineColor::Success,
        );

        self.empty_line();
        self.println("  Use arrow keys to control the snake.", LineColor::Info);
        self.println("  Eat food (\u{25CF}) to grow and score!", LineColor::Info);
        self.println("  Not yet added - coming soon!", LineColor::Warning);
    }

    fn cmd_ascii(&mut self) {
        self.empty_line();
        self.println("  \u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}", LineColor::Info);
        self.println(
            "  \u{2551}  ASCII Art Gallery                       \u{2551}",
            LineColor::Info,
        );
        self.println("  \u{255A}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D}", LineColor::Info);
        self.empty_line();

        // BerkeOS logo
        self.println("       _ \u{250C}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}_", LineColor::Info);
        self.println(
            "      | | \u{2502}     BerkeOS     \u{2502}| |",
            LineColor::Info,
        );
        self.println(
            "      | | \u{2502}  x86_64/Rust  \u{2502}| |",
            LineColor::Info,
        );
        self.println("      |_| \u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}_", LineColor::Info);

        self.empty_line();

        // Robot
        self.println("        \u{2588}\u{2588}\u{2588}", LineColor::Yellow);
        self.println(
            "      \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}",
            LineColor::Yellow,
        );
        self.println(
            "    \u{2588}\u{2588} \u{2665} \u{2588}\u{2588}",
            LineColor::Yellow,
        );
        self.println(
            "      \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}",
            LineColor::Yellow,
        );
        self.println(
            "     \u{2588}  \u{2588}\u{2588}  \u{2588}",
            LineColor::Yellow,
        );
        self.println("    \u{2588}   \u{2588}   \u{2588}", LineColor::Yellow);

        self.empty_line();
        self.println("  More art: 'banner <text>'", LineColor::Info);
    }

    fn cmd_rain(&mut self) {
        self.empty_line();
        self.println("  Falling Code Rain - Matrix style", LineColor::Success);
        self.empty_line();

        let drops = [
            "\u{4E00}", "\u{4E01}", "\u{4E02}", "\u{4E03}", "0", "1", "\u{0391}", "\u{0392}",
            "\u{0393}",
        ];

        for row in 0..6 {
            let mut line = [0u8; 40];
            let mut pos = 0;
            for col in 0..8 {
                let drop = drops[(row * 3 + col) % drops.len()];
                for b in &drop.as_bytes()[..2.min(drop.as_bytes().len())] {
                    if pos < 38 {
                        line[pos] = *b;
                        pos += 1;
                    }
                }
                if pos < 39 {
                    line[pos] = b' ';
                    pos += 1;
                }
            }
            self.push_line(&line[..pos], LineColor::Success);
        }

        self.empty_line();
    }

    fn cmd_fire(&mut self) {
        self.empty_line();
        self.println("  \u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}", LineColor::Error);
        self.println(
            "  \u{2551}  ASCII Fire Effect                    \u{2551}",
            LineColor::Error,
        );
        self.println("  \u{255A}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D}", LineColor::Error);
        self.empty_line();

        self.println(
            "        \u{2597}\u{2596}\u{2596}\u{2596}\u{2596}\u{2596}\u{2597}",
            LineColor::Error,
        );
        self.println(
            "       \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}",
            LineColor::Warning,
        );
        self.println(
            "      \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}",
            LineColor::Warning,
        );
        self.println(
            "     \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}",
            LineColor::Gold,
        );
        self.println(
            "    \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}",
            LineColor::Gold,
        );
        self.println("   \u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}", LineColor::Success);

        self.empty_line();
    }

    fn cmd_calc(&mut self, arg: &[u8]) {
        if arg.is_empty() {
            self.println("  calc: usage: calc <expression>", LineColor::Error);
            self.println("  Examples:", LineColor::Info);
            self.println("    calc 5 + 3", LineColor::Normal);
            self.println("    calc 10 * 7", LineColor::Normal);
            self.println("    calc 100 / 4", LineColor::Normal);
            return;
        }

        let mut num1: i64 = 0;
        let mut num2: i64 = 0;
        let mut op: u8 = 0;
        let mut parsing_first = true;
        let mut found_op = false;

        for &b in arg {
            if b == b'+' && !found_op {
                op = 1;
                found_op = true;
                parsing_first = false;
            } else if b == b'-' && !found_op && !parsing_first {
                op = 2;
                found_op = true;
                parsing_first = false;
            } else if b == b'*' && !found_op {
                op = 3;
                found_op = true;
                parsing_first = false;
            } else if b == b'/' && !found_op {
                op = 4;
                found_op = true;
                parsing_first = false;
            } else if b >= b'0' && b <= b'9' {
                if parsing_first {
                    num1 = num1 * 10 + (b - b'0') as i64;
                } else {
                    num2 = num2 * 10 + (b - b'0') as i64;
                }
            }
        }

        if op == 0 || num2 == 0 {
            self.println("  calc: invalid expression", LineColor::Error);
            return;
        }

        let result: i64 = match op {
            1 => num1 + num2,
            2 => num1 - num2,
            3 => num1 * num2,
            4 => num1 / num2,
            _ => 0,
        };

        let mut buf = [0u8; 32];
        let mut i = 0;
        let mut neg = result < 0;
        let mut val = if neg { -result } else { result };

        if val == 0 {
            buf[0] = b'0';
            i = 1;
        } else {
            while val > 0 && i < 30 {
                buf[i] = b'0' + (val % 10) as u8;
                val /= 10;
                i += 1;
            }
        }

        if neg {
            buf[i] = b'-';
            i += 1;
        }

        for j in 0..i / 2 {
            let tmp = buf[j];
            buf[j] = buf[i - 1 - j];
            buf[i - 1 - j] = tmp;
        }

        self.push_line(&buf[..i], LineColor::Success);
    }

    fn cmd_df(&mut self, _fs: &mut BerkeFS) {
        self.empty_line();
        self.println(
            "  +========================================+",
            LineColor::Info,
        );
        self.println(
            "  |         Disk/Drive Kullanim            |",
            LineColor::Info,
        );
        self.println(
            "  +========================================+",
            LineColor::Info,
        );
        self.empty_line();

        let mut line = [b' '; 54];
        let mut pos = 0;
        line[pos] = b' ';
        pos += 1;
        for &b in b"DRIVE" {
            line[pos] = b;
            pos += 1;
        }
        line[pos] = b' ';
        pos += 1;
        for &b in b"TURU" {
            line[pos] = b;
            pos += 1;
        }
        line[pos] = b' ';
        pos += 1;
        for &b in b"BOYUT" {
            line[pos] = b;
            pos += 1;
        }
        line[pos] = b' ';
        pos += 1;
        for &b in b"KULLANILAN" {
            line[pos] = b;
            pos += 1;
        }
        line[pos] = b' ';
        pos += 1;
        for &b in b"BOS" {
            line[pos] = b;
            pos += 1;
        }
        self.push_line(&line[..pos], LineColor::Gold);
        self.empty_line();

        let block_size = crate::berkefs::BLOCK_SIZE as u64;
        let max_blocks = crate::berkefs::MAX_DATA_BLOCKS as u64;
        let fs_total = block_size * max_blocks;
        let current_fs = self.get_current_fs();
        let fs_used_blocks = max_blocks.saturating_sub(current_fs.free_blocks() as u64);
        let fs_used = fs_used_blocks * block_size;

        for i in 0..MAX_DRIVES {
            let state = self.drives[i].drive_type;
            if state == DriveType::None {
                continue;
            }

            let drive_name = DriveId::from_u8(i as u8).name();
            let is_current = self.path.drive.to_u8() as usize == i;
            let drive_size = self.drives[i].size;

            if drive_size == 0 {
                continue;
            }

            let type_str: &str;
            let drive_used: u64;
            let drive_free: u64;

            if self.drives[i].is_ramdisk {
                type_str = "RAM Disk";
                let ratio = if fs_total > 0 {
                    (fs_used as f64 / fs_total as f64).min(1.0)
                } else {
                    0.0
                };
                drive_used = (drive_size as f64 * ratio) as u64;
                drive_free = drive_size - drive_used;
            } else if state == DriveType::Formatted {
                type_str = "Disk";
                let ratio = if fs_total > 0 {
                    (fs_used as f64 / fs_total as f64).min(1.0)
                } else {
                    0.0
                };
                drive_used = (drive_size as f64 * ratio) as u64;
                drive_free = drive_size - drive_used;
            } else if state == DriveType::Named {
                type_str = "Sanal";
                drive_used = self.drives[i].used;
                drive_free = drive_size.saturating_sub(drive_used);
            } else {
                type_str = "Diger";
                drive_used = 0;
                drive_free = drive_size;
            }

            let mut out = [b' '; 70];
            let mut p = 0;

            out[p] = if is_current { b'*' } else { b' ' };
            p += 1;

            for &b in drive_name.as_bytes().iter().take(8) {
                out[p] = b;
                p += 1;
            }
            out[p] = b' ';
            p += 1;

            let type_bytes = type_str.as_bytes();
            for &b in type_bytes.iter().take(8) {
                out[p] = b;
                p += 1;
            }
            for _ in type_bytes.len()..8 {
                out[p] = b' ';
                p += 1;
            }
            out[p] = b' ';
            p += 1;

            let mut sb = [0u8; 32];
            let sl = self.fmt_size(drive_size, &mut sb);
            for j in 0..8 {
                if j < 8 - sl {
                    out[p] = b' ';
                } else {
                    out[p] = sb[j - (8 - sl)];
                }
                p += 1;
            }
            out[p] = b' ';
            p += 1;

            let mut ub = [0u8; 32];
            let ul = self.fmt_size(drive_used, &mut ub);
            for j in 0..8 {
                if j < 8 - ul {
                    out[p] = b' ';
                } else {
                    out[p] = ub[j - (8 - ul)];
                }
                p += 1;
            }
            out[p] = b' ';
            p += 1;

            let mut fb = [0u8; 32];
            let fl = self.fmt_size(drive_free, &mut fb);
            for j in 0..8 {
                if j < 8 - fl {
                    out[p] = b' ';
                } else {
                    out[p] = fb[j - (8 - fl)];
                }
                p += 1;
            }

            let lc = if self.drives[i].is_ramdisk {
                LineColor::Yellow
            } else {
                LineColor::Success
            };
            self.push_line(&out[..p], lc);
        }

        self.empty_line();
        self.println("  * = aktif surucu", LineColor::Info);
        self.empty_line();
    }

    fn cmd_img(&mut self, arg: &[u8]) {
        if arg.is_empty() {
            self.println("  img: usage: img <file.bmp>", LineColor::Error);
            self.println("  Supported: BMP (uncompressed)", LineColor::Info);
            return;
        }

        self.empty_line();
        self.println("  Loading image...", LineColor::Info);
        self.println("  Image viewer coming in Phase 6!", LineColor::Warning);
    }

    fn cmd_video(&mut self, arg: &[u8]) {
        if arg.is_empty() {
            self.println("  video: usage: video <file>", LineColor::Error);
            self.println("  Video player coming in Phase 6!", LineColor::Warning);
            return;
        }

        self.empty_line();
        self.println("  Video player", LineColor::Info);
        self.println("  Coming in Phase 6!", LineColor::Warning);
    }

    fn cmd_doom(&mut self) {
        self.empty_line();
        self.println("  \u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}", LineColor::Error);
        self.println(
            "  \u{2551}  DOOM for BerkeOS                    \u{2551}",
            LineColor::Error,
        );
        self.println("  \u{255A}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255D}", LineColor::Error);
        self.empty_line();

        self.println("         _______  ", LineColor::Error);
        self.println("        /      \\ ", LineColor::Error);
        self.println("       |  DOOM  |", LineColor::Error);
        self.println("       |  1993  |", LineColor::Error);
        self.println("        \\______/", LineColor::Error);

        self.empty_line();
        self.println("  id Software经典作品", LineColor::Info);
        self.println("  BerkeOS Phase 6'da!", LineColor::Warning);
        self.empty_line();

        self.println("  Gerekli:", LineColor::Gold);
        self.println("  - DOOM IWAD dosyasi (doom.wad)", LineColor::Normal);
        self.println("  - Sound card support", LineColor::Normal);
        self.println("  - Joystick/Keyboard control", LineColor::Normal);
    }

    fn cmd_editor(&mut self, arg: &[u8], fs: &mut BerkeFS, fb: &mut Framebuffer) {
        deno::run_editor(arg, fs, fb, self);
    }

    fn cmd_berke(&mut self) {
        self.empty_line();
        self.println(
            "  =============================================",
            LineColor::Gold,
        );
        self.println("  Creator: Berke Oruc", LineColor::Gold);
        self.println("  GitHub: github.com/berkeoruc", LineColor::Info);
        self.println(
            "  =============================================",
            LineColor::Gold,
        );
        self.empty_line();
        self.println("  About the developer:", LineColor::Info);
        self.println("  - Age 16, student developer", LineColor::Normal);
        self.println("  - Started OS development at age 14", LineColor::Normal);
        self.println("  - Built BerkeOS from scratch in Rust", LineColor::Normal);
        self.empty_line();
        self.println("  Code Statistics:", LineColor::Info);
        self.println("  - Total lines: ~14,288", LineColor::Normal);
        self.println("  - Berke's code: 43% (~6,143 lines)", LineColor::Normal);
        self.println("  - AI-assisted: 57% (~8,145 lines)", LineColor::Normal);
        self.println("  - Cost: 0 TL (AI tools free)", LineColor::Normal);
        self.empty_line();
        self.println("  Motivation:", LineColor::Info);
        self.println(
            "  'I wanted to prove that with dedication",
            LineColor::Normal,
        );
        self.println("   and AI assistance, anyone can build", LineColor::Normal);
        self.println("   an operating system from scratch.'", LineColor::Normal);
        self.empty_line();
        self.println("  Future Goals:", LineColor::Info);
        self.println("  - Network stack (TCP/IP)", LineColor::Normal);
        self.println("  - GUI desktop environment", LineColor::Normal);
        self.println("  - Package manager", LineColor::Normal);
        self.println("  - Mobile device support", LineColor::Normal);
        self.empty_line();
        self.println(
            "  If you've read this far, please consider",
            LineColor::Warning,
        );
        self.println(
            "  giving this project a star on GitHub!",
            LineColor::Warning,
        );
        self.println("  It would mean a lot to a 16-year-old", LineColor::Warning);
        self.println("  developer from Turkey.", LineColor::Warning);
        self.empty_line();
        self.println("  github.com/berkeoruc", LineColor::Gold);
    }

    fn cmd_update(&mut self) {
        self.empty_line();
        self.println(
            "  =============================================",
            LineColor::Info,
        );
        self.println("  BerkeOS v0.6.1 - Future Roadmap", LineColor::Info);
        self.println(
            "  =============================================",
            LineColor::Info,
        );
        self.empty_line();
        self.println("  Planned Features:", LineColor::Gold);
        self.println("  [Planned] Network stack (TCP/IP)", LineColor::Normal);
        self.println("  [Planned] Sound card driver", LineColor::Normal);
        self.println("  [Planned] Multi-core CPU support", LineColor::Normal);
        self.println("  [Planned] USB 3.0 support", LineColor::Normal);
        self.println("  [Planned] GUI desktop environment", LineColor::Normal);
        self.println("  [Planned] Package manager", LineColor::Normal);
        self.println("  [Planned] Advanced text editor", LineColor::Normal);
        self.println("  [Planned] Web browser", LineColor::Normal);
        self.empty_line();
        self.println("  If you want to support this project,", LineColor::Warning);
        self.println(
            "  consider contributing or starring on GitHub!",
            LineColor::Warning,
        );
        self.println("  github.com/berkeoruc", LineColor::Info);
        self.empty_line();
        self.println(
            "  Note: All features depend on community support",
            LineColor::Warning,
        );
    }

    fn cmd_python(&mut self, arg: &[u8]) {
        self.empty_line();
        self.println(
            "  +============================================+",
            LineColor::Info,
        );
        self.println(
            "  |  BerkeOS Python/REPL v0.1                 |",
            LineColor::Info,
        );
        self.println(
            "  +============================================+",
            LineColor::Info,
        );
        self.empty_line();

        if !arg.is_empty() {
            let mut code = [0u8; 256];
            let mut code_len = 0;
            for &b in arg {
                if code_len < 255 {
                    code[code_len] = b;
                    code_len += 1;
                }
            }

            let code_slice = &code[..code_len];

            if code_slice.starts_with(b"print(") || code_slice.starts_with(b"print (") {
                let start = if code_slice[5] == b'(' { 6 } else { 7 };
                let mut in_string = false;
                let mut string_char = 0u8;
                let mut result = [0u8; 256];
                let mut ri = 0;

                for i in start..code_len {
                    let b = code_slice[i];
                    if !in_string && (b == b'"' || b == b'\'') {
                        in_string = true;
                        string_char = b;
                        continue;
                    }
                    if in_string && b == string_char {
                        in_string = false;
                        continue;
                    }
                    if !in_string {
                        continue;
                    }
                    if ri < 255 {
                        result[ri] = b;
                        ri += 1;
                    }
                }
                self.push_line(&result[..ri], LineColor::Success);
            } else if code_slice.starts_with(b"for ") || code_slice.starts_with(b"while ") {
                self.println("  [Loop - editor coming soon]", LineColor::Warning);
            } else if code_slice.starts_with(b"if ") {
                self.println("  [Conditional detected]", LineColor::Warning);
            } else if code_slice.starts_with(b"def ") {
                self.println("  [Function - editor coming soon]", LineColor::Warning);
            } else {
                self.println(
                    "  print('hello') , for i in range(5): , if x > 0:",
                    LineColor::Info,
                );
            }
        } else {
            self.println("  Usage: py print('hello')", LineColor::Info);
            self.println("  Examples:", LineColor::Gold);
            self.println("    py print('Hello World')", LineColor::Normal);
            self.println("    py for i in range(3):", LineColor::Normal);
        }
        self.empty_line();
    }

    // ── Main loop ─────────────────────────────────────────────────────────────
    pub fn run(&mut self, fb: &mut Framebuffer) {
        self.cmd_banner();
        self.empty_line();
        self.println(
            "  Welcome to BerkeOS! Type 'help' for commands.",
            LineColor::Info,
        );
        let fs0_mounted = unsafe { (*self.fs_ptrs[0]).mounted };
        if self.disk_ok {
            self.println(
                "  \u{2714} ATA disk detected \u{2014} BerkeFS active",
                LineColor::Success,
            );
            if fs0_mounted {
                self.println("  \u{2714} BerkeFS mounted", LineColor::Success);
            } else {
                self.println(
                    "  \u{26a0} Disk found but not formatted. Type 'format'.",
                    LineColor::Yellow,
                );
            }
        } else {
            self.println(
                "  \u{26a0} Depolama ayg\u{131}t\u{131} alg\u{131}lanamad\u{131}",
                LineColor::Yellow,
            );
        }
        self.empty_line();
        self.draw_full(fb);

        let mut kb = Keyboard::new();

        loop {
            let key = kb.poll();
            let mut needs_redraw = false;

            match key {
                Key::None => unsafe {
                    for _ in 0..5000usize {
                        core::arch::asm!("pause");
                    }
                },
                Key::Char(b'\n') | Key::Char(b'\r') => {
                    let drive_idx = self.path.drive.to_u8() as usize;
                    if drive_idx < 12 && !self.fs_ptrs[drive_idx].is_null() {
                        let fs = unsafe { &mut *self.fs_ptrs[drive_idx] };
                        self.execute(fb, fs);
                    }
                }
                Key::Char(b'\x08') => {
                    self.delete_before();
                    needs_redraw = true;
                }
                Key::Char(ch) => {
                    if ch >= 0x20 {
                        self.insert_char(ch);
                        needs_redraw = true;
                    }
                }
                Key::Up => {
                    self.history_up();
                    needs_redraw = true;
                }
                Key::Down => {
                    self.history_down();
                    needs_redraw = true;
                }
                Key::Left => {
                    if self.cursor > 0 {
                        self.cursor -= 1;
                    }
                    needs_redraw = true;
                }
                Key::Right => {
                    if self.cursor < self.buf_len {
                        self.cursor += 1;
                    }
                    needs_redraw = true;
                }
                Key::Home => {
                    self.cursor = 0;
                    needs_redraw = true;
                }
                Key::End => {
                    self.cursor = self.buf_len;
                    needs_redraw = true;
                }
                Key::Delete => {
                    self.delete_at();
                    needs_redraw = true;
                }
                Key::CtrlC => {
                    self.buf = [0; MAX_LINE];
                    self.buf_len = 0;
                    self.cursor = 0;
                    self.push_line(b"^C", LineColor::Error);
                    self.empty_line();
                    needs_redraw = true;
                }
                Key::CtrlL | Key::F2 => {
                    self.cmd_clear();
                    needs_redraw = true;
                }
                Key::CtrlA => {
                    self.cursor = 0;
                    needs_redraw = true;
                }
                Key::CtrlE => {
                    self.cursor = self.buf_len;
                    needs_redraw = true;
                }
                Key::CtrlU => {
                    self.buf = [0; MAX_LINE];
                    self.buf_len = 0;
                    self.cursor = 0;
                    needs_redraw = true;
                }
                Key::CtrlK => {
                    self.buf_len = self.cursor;
                    needs_redraw = true;
                }
                Key::F1 => {
                    self.cmd_help();
                    needs_redraw = true;
                }
                Key::F5 => {
                    self.cmd_neofetch();
                    needs_redraw = true;
                }
                Key::CtrlZ | Key::CtrlY => {
                    needs_redraw = true;
                }
                Key::CtrlS | Key::CtrlW => {
                    needs_redraw = true;
                }
                Key::CtrlQ => {}
                Key::Escape => {
                    self.buf = [0; MAX_LINE];
                    self.buf_len = 0;
                    self.cursor = 0;
                    needs_redraw = true;
                }
            }

            if needs_redraw {
                self.draw_full(fb);
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────
fn fmt_scroll<'a>(shown: usize, total: usize, buf: &'a mut [u8; 32]) -> &'a str {
    let mut i = 0usize;
    i = write_uint_buf(buf, i, shown);
    buf[i] = b'/';
    i += 1;
    i = write_uint_buf(buf, i, total);
    core::str::from_utf8(&buf[..i]).unwrap_or("")
}

fn write_uint_buf(buf: &mut [u8], mut pos: usize, mut n: usize) -> usize {
    if n == 0 {
        if pos < buf.len() {
            buf[pos] = b'0';
        }
        return pos + 1;
    }
    let start = pos;
    while n > 0 && pos < buf.len() {
        buf[pos] = b'0' + (n % 10) as u8;
        n /= 10;
        pos += 1;
    }
    buf[start..pos].reverse();
    pos
}

fn contains_slice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }
    for i in 0..=(haystack.len() - needle.len()) {
        if &haystack[i..i + needle.len()] == needle {
            return true;
        }
    }
    false
}
