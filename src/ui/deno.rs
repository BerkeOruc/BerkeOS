// BerkeOS — deno.rs
// Deno Text Editor Module

use crate::fs::berkefs::BerkeFS;
use crate::graphics::font;
use crate::drivers::framebuffer::{Color, Framebuffer};
use crate::drivers::keyboard::{Key, Keyboard};

const GW: usize = font::GLYPH_W;
const GH: usize = font::GLYPH_H;

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
fn col_gold() -> Color {
    Color::rgb(0xff, 0xcc, 0x00)
}
fn col_green() -> Color {
    Color::rgb(0x44, 0xff, 0x44)
}

fn write_uint_buf(buf: &mut [u8], _base: usize, mut n: usize) -> usize {
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut pos = 0;
    while n > 0 {
        buf[pos] = b'0' + (n % 10) as u8;
        n /= 10;
        pos += 1;
    }
    pos
}

pub fn run_editor(
    arg: &[u8],
    fs: &mut BerkeFS,
    fb: &mut Framebuffer,
    shell: &mut crate::ui::shell::Shell,
) {
    let mut is_help = false;

    if arg.is_empty() || arg.len() >= 2 {
        if arg.len() >= 2 && arg[0] == b'-' && arg[1] == b'-' {
            let mut check = &arg[2..];
            if check.len() >= 4
                && check[0] == b'h'
                && check[1] == b'e'
                && check[2] == b'l'
                && check[3] == b'p'
            {
                is_help = true;
            }
        }
    }

    if arg.is_empty() || is_help {
        shell.empty_line();
        shell.println(
            "  BerkeOS Deno Editor v0.3.8",
            crate::ui::shell::LineColor::Gold,
        );
        shell.println(
            "  Developer: Berke Oruc (Age 16)",
            crate::ui::shell::LineColor::Info,
        );
        shell.println(
            "  GitHub: github.com/berkeoruc/BerkeOS",
            crate::ui::shell::LineColor::Info,
        );
        shell.empty_line();
        shell.println("  Usage: deno <filename>", crate::ui::shell::LineColor::Normal);
        shell.println(
            "         deno --help                       ",
            crate::ui::shell::LineColor::Normal,
        );
        shell.empty_line();
        shell.println("  Commands:", crate::ui::shell::LineColor::Gold);
        shell.println(
            "    deno <file>     - Edit or create file",
            crate::ui::shell::LineColor::Normal,
        );
        shell.empty_line();
        shell.println("  Editor Keys:", crate::ui::shell::LineColor::Gold);
        shell.println(
            "    Arrow keys     - Move cursor",
            crate::ui::shell::LineColor::Normal,
        );
        shell.println(
            "    Any key        - Type directly",
            crate::ui::shell::LineColor::Normal,
        );
        shell.println(
            "    Ctrl+S         - Save file",
            crate::ui::shell::LineColor::Normal,
        );
        shell.println(
            "    Ctrl+Q         - Quit (force if modified)",
            crate::ui::shell::LineColor::Normal,
        );
        shell.empty_line();
        return;
    }

    let term_cols = (fb.width.saturating_sub(20)) / GW;
    let term_rows = fb.height.saturating_sub(40) / GH;

    let mut filename = [0u8; 64];
    let mut fn_len = 0;
    for &b in arg {
        if b == b' ' {
            break;
        }
        if fn_len < 63 {
            filename[fn_len] = b;
            fn_len += 1;
        }
    }

    let fname = core::str::from_utf8(&filename[..fn_len]).unwrap_or("untitled");

    let mut content = [0u8; 8192];
    let content_len = fs.read_file(&filename[..fn_len], &mut content).unwrap_or(0);

    let mut lines: [[u8; 128]; 256] = [[0; 128]; 256];
    let mut line_count = 1;
    let mut line_lens: [usize; 256] = [0; 256];

    lines[0] = [0; 128];
    line_lens[0] = 0;

    for i in 0..content_len {
        if content[i] == b'\n' || line_lens[line_count - 1] >= 127 {
            line_count += 1;
            if line_count >= 256 {
                break;
            }
            lines[line_count - 1] = [0; 128];
            line_lens[line_count - 1] = 0;
            if content[i] == b'\n' {
                continue;
            }
        }
        let idx = line_lens[line_count - 1];
        lines[line_count - 1][idx] = content[i];
        line_lens[line_count - 1] += 1;
    }

    let mut cursor_row = 0;
    let mut cursor_col = 0;
    let _insert_mode = true;
    let mut modified = false;
    let mut status_msg = [0u8; 64];
    let mut status_len: usize = 0;

    let mut kb = Keyboard::new();

    let cols = term_cols.max(50);
    let rows = term_rows.max(15);

    let line_num_width = 6;
    let gutter_x = 10;
    let text_x = gutter_x + (line_num_width * GW);
    let menu_height = GH + 10;

    let mut needs_redraw = true;

    loop {
        let key = kb.poll();

        if matches!(key, Key::None) {
            core::hint::black_box(());
        } else {
            needs_redraw = true;
        }

        if !matches!(key, Key::None) {
            match key {
                Key::Escape => {
                    cursor_col = 0;
                }
                Key::Up => {
                    if cursor_row > 0 {
                        cursor_row -= 1;
                    }
                    if cursor_col > line_lens[cursor_row] {
                        cursor_col = line_lens[cursor_row];
                    }
                }
                Key::Down => {
                    if cursor_row + 1 < line_count {
                        cursor_row += 1;
                    }
                    if cursor_col > line_lens[cursor_row] {
                        cursor_col = line_lens[cursor_row];
                    }
                }
                Key::Left => {
                    if cursor_col > 0 {
                        cursor_col -= 1;
                    }
                }
                Key::Right => {
                    if cursor_col < line_lens[cursor_row] {
                        cursor_col += 1;
                    }
                }
                Key::Home => {
                    cursor_col = 0;
                }
                Key::End => {
                    cursor_col = line_lens[cursor_row];
                }
                Key::Char(b'\x08') => {
                    if cursor_col > 0 {
                        cursor_col -= 1;
                        if cursor_col < line_lens[cursor_row] {
                            for p in cursor_col..line_lens[cursor_row] - 1 {
                                lines[cursor_row][p] = lines[cursor_row][p + 1];
                            }
                            line_lens[cursor_row] -= 1;
                        }
                        modified = true;
                    } else if cursor_row > 0 {
                        cursor_row -= 1;
                        cursor_col = line_lens[cursor_row];
                        modified = true;
                    }
                }
                Key::Char(b'\n') | Key::Char(b'\r') => {
                    let current_len = line_lens[cursor_row];
                    let split_pos = cursor_col;

                    if split_pos < current_len {
                        let mut new_line = [0u8; 128];
                        let mut new_len = 0;
                        for c in split_pos..current_len {
                            new_line[new_len] = lines[cursor_row][c];
                            new_len += 1;
                        }
                        line_lens[cursor_row] = split_pos;

                        if cursor_row + 1 < 256 {
                            for i in ((cursor_row + 1)..line_count).rev() {
                                lines[i] = lines[i - 1];
                                line_lens[i] = line_lens[i - 1];
                            }
                            line_count += 1;
                            lines[cursor_row + 1] = new_line;
                            line_lens[cursor_row + 1] = new_len;
                        }
                    } else {
                        if cursor_row + 1 < 256 {
                            for i in ((cursor_row + 1)..line_count).rev() {
                                lines[i] = lines[i - 1];
                                line_lens[i] = line_lens[i - 1];
                            }
                            line_count += 1;
                            lines[cursor_row + 1] = [0u8; 128];
                            line_lens[cursor_row + 1] = 0;
                        }
                    }

                    cursor_row += 1;
                    cursor_col = 0;
                    modified = true;
                }
                Key::Char(ch) => {
                    if line_lens[cursor_row] < 127 && ch >= 0x20 {
                        for p in (cursor_col..line_lens[cursor_row]).rev() {
                            lines[cursor_row][p + 1] = lines[cursor_row][p];
                        }
                        lines[cursor_row][cursor_col] = ch;
                        line_lens[cursor_row] += 1;
                        cursor_col += 1;
                        modified = true;
                    }
                }
                Key::CtrlS => {
                    let mut all_content = [0u8; 8192];
                    let mut total = 0;
                    for r in 0..line_count {
                        for c in 0..line_lens[r] {
                            if total < 8191 {
                                all_content[total] = lines[r][c];
                                total += 1;
                            }
                        }
                        if total < 8191 {
                            all_content[total] = b'\n';
                            total += 1;
                        }
                    }
                    fs.delete_file(&filename[..fn_len]);
                    fs.create_file(&filename[..fn_len], &all_content[..total]);
                    modified = false;
                    status_len = 0;
                    status_msg[..6].copy_from_slice(b"Saved!");
                    status_len = 6;
                }
                Key::CtrlQ => {
                    return;
                }
                _ => {}
            }
        }

        if needs_redraw {
            needs_redraw = false;

            let start_row = if cursor_row >= rows - 3 {
                cursor_row - rows + 4
            } else {
                0
            };

            fb.fill_rect(0, 0, fb.width, fb.height, col_bg());
            fb.fill_rect(0, 0, fb.width, menu_height, col_gray());

            let menu_items = "-- EDITOR --";
            for (j, &b) in menu_items.as_bytes().iter().enumerate() {
                fb.draw_glyph(10 + j * GW, 5, b as char, col_cyan(), col_gray());
            }

            let menu_mid = fb.width / 2;
            let fname_display = if fname.len() > 30 {
                &fname[fname.len() - 30..]
            } else {
                fname
            };
            for (j, &b) in fname_display.as_bytes().iter().enumerate() {
                fb.draw_glyph(
                    menu_mid - (fname_display.len() * GW / 2) + j * GW,
                    5,
                    b as char,
                    col_gold(),
                    col_gray(),
                );
            }

            if modified {
                fb.draw_glyph(
                    menu_mid + (fname_display.len() * GW / 2) + 15,
                    5,
                    '[' as u8 as char,
                    col_yellow(),
                    col_gray(),
                );
                fb.draw_glyph(
                    menu_mid + (fname_display.len() * GW / 2) + 16,
                    5,
                    '+' as u8 as char,
                    col_yellow(),
                    col_gray(),
                );
                fb.draw_glyph(
                    menu_mid + (fname_display.len() * GW / 2) + 17,
                    5,
                    ']' as u8 as char,
                    col_yellow(),
                    col_gray(),
                );
            }

            let help_right = "F1:Help | Ctrl+S:Save | Ctrl+Q:Quit";
            for (j, &b) in help_right.as_bytes().iter().enumerate() {
                fb.draw_glyph(
                    fb.width.saturating_sub(help_right.len() * GW) - 10 + j * GW,
                    5,
                    b as char,
                    col_ltpnk(),
                    col_gray(),
                );
            }

            fb.fill_rect(
                gutter_x - 5,
                menu_height,
                line_num_width * GW + 10,
                fb.height - menu_height,
                col_bg(),
            );

            for r in 0..rows.saturating_sub(2) {
                let line_idx = start_row + r;
                if line_idx >= line_count {
                    break;
                }

                let mut num_buf = [0u8; 8];
                let num_len = write_uint_buf(&mut num_buf, 0, line_idx + 1);
                let line_color = if line_idx == cursor_row {
                    col_cyan()
                } else {
                    col_dkpnk()
                };

                for j in 0..num_len {
                    fb.draw_glyph(
                        gutter_x + (num_len - 1 - j) * GW,
                        menu_height + 5 + r * GH,
                        num_buf[j] as char,
                        line_color,
                        col_bg(),
                    );
                }

                if line_idx < line_count {
                    let line = &lines[line_idx];
                    let len = line_lens[line_idx];

                    let max_text_cols = cols.saturating_sub(line_num_width);

                    for c in 0..max_text_cols.min(len) {
                        let ch = line[c];
                        if ch != 0 {
                            let is_cursor = cursor_row == line_idx && c == cursor_col;
                            if is_cursor {
                                fb.draw_glyph(
                                    text_x + c * GW,
                                    menu_height + 5 + r * GH,
                                    ch as char,
                                    col_white(),
                                    col_pink(),
                                );
                            } else {
                                fb.draw_glyph(
                                    text_x + c * GW,
                                    menu_height + 5 + r * GH,
                                    ch as char,
                                    col_white(),
                                    col_bg(),
                                );
                            }
                        }
                    }

                    if cursor_row == line_idx && cursor_col >= len {
                        let cursor_x = text_x + cursor_col * GW;
                        let cursor_y = menu_height + 5 + r * GH;
                        fb.fill_rect(cursor_x, cursor_y, GW, GH, col_pink());
                    }
                }
            }

            let status_y = fb.height.saturating_sub(GH + 10);
            fb.fill_rect(0, status_y, fb.width, GH + 10, col_gray());

            let mut info_buf = [0u8; 24];
            let mut info_len = 0;

            let lnn = cursor_row + 1;
            if lnn >= 100 {
                info_buf[info_len] = b'0' + ((lnn / 100) % 10) as u8;
                info_len += 1;
            }
            if lnn >= 10 {
                info_buf[info_len] = b'0' + ((lnn / 10) % 10) as u8;
                info_len += 1;
            }
            info_buf[info_len] = b'0' + (lnn % 10) as u8;
            info_len += 1;

            info_buf[info_len] = b'/';
            info_len += 1;
            let tc = line_count;
            if tc >= 100 {
                info_buf[info_len] = b'0' + ((tc / 100) % 10) as u8;
                info_len += 1;
            }
            if tc >= 10 {
                info_buf[info_len] = b'0' + ((tc / 10) % 10) as u8;
                info_len += 1;
            }
            info_buf[info_len] = b'0' + (tc % 10) as u8;
            info_len += 1;

            let mut cpy = b", Col ";
            for &b in cpy.iter() {
                info_buf[info_len] = b;
                info_len += 1;
            }

            let cc = cursor_col + 1;
            if cc >= 100 {
                info_buf[info_len] = b'0' + ((cc / 100) % 10) as u8;
                info_len += 1;
            }
            if cc >= 10 {
                info_buf[info_len] = b'0' + ((cc / 10) % 10) as u8;
                info_len += 1;
            }
            info_buf[info_len] = b'0' + (cc % 10) as u8;
            info_len += 1;

            for j in 0..info_len {
                fb.draw_glyph(
                    10 + j * GW,
                    status_y + 5,
                    info_buf[j] as char,
                    col_ltpnk(),
                    col_gray(),
                );
            }

            let info_right = "READY";
            for (j, &b) in info_right.as_bytes().iter().enumerate() {
                fb.draw_glyph(
                    fb.width.saturating_sub(info_right.len() * GW) - 10 + j * GW,
                    status_y + 5,
                    b as char,
                    col_cyan(),
                    col_gray(),
                );
            }

            if status_len > 0 {
                let msg_x = (fb.width / 2) - (status_len * GW / 2);
                for (j, &b) in status_msg[..status_len].iter().enumerate() {
                    fb.draw_glyph(
                        msg_x + j * GW,
                        status_y + 5,
                        b as char,
                        col_green(),
                        col_gray(),
                    );
                }
            }
        }

        core::hint::black_box(&key);
    }
}
