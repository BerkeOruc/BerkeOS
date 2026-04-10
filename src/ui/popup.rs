// BerkeOS — popup.rs
// Blocking popup/warning dialog system for framebuffer display.

use crate::drivers::framebuffer::{Color, Framebuffer};
use crate::drivers::keyboard::{Key, Keyboard};
use crate::drivers::serial;

const POPUP_PADDING: usize = 20;
const POPUP_BORDER: usize = 2;
const BUTTON_HEIGHT: usize = 24;
const BUTTON_PADDING: usize = 16;

const POPUP_BG: Color = Color::rgb(0x1a, 0x1a, 0x2e);
const POPUP_BORDER_COLOR: Color = Color::rgb(0xe8, 0x79, 0x2b);
const POPUP_TITLE_BG: Color = Color::rgb(0xe8, 0x79, 0x2b);
const TEXT_COLOR: Color = Color::rgb(0xff, 0xff, 0xff);
const BUTTON_BG: Color = Color::rgb(0x2e, 0x2e, 0x4e);
const BUTTON_TEXT: Color = Color::rgb(0xff, 0xff, 0xff);

pub fn show_popup(fb: &mut Framebuffer, message: &str) {
    serial::write_str("[POPUP] ");
    serial::write_str(message);
    serial::write_str("\r\n");

    let screen_w = fb.width;
    let screen_h = fb.height;

    let msg_width = message.len() * 8;
    let title_text = "UYARI";
    let title_width = title_text.len() * 8;

    let content_width = msg_width.max(title_width) + POPUP_PADDING * 2;
    let content_height =
        16 + POPUP_PADDING + 16 + POPUP_PADDING + BUTTON_HEIGHT + POPUP_PADDING * 2;

    let popup_x = (screen_w.saturating_sub(content_width)) / 2;
    let popup_y = (screen_h.saturating_sub(content_height)) / 2;
    let popup_w = content_width;
    let popup_h = content_height;

    fb.fill_rect(popup_x, popup_y, popup_w, popup_h, POPUP_BG);
    fb.fill_rect(popup_x, popup_y, popup_w, POPUP_BORDER, POPUP_BORDER_COLOR);
    fb.fill_rect(
        popup_x,
        popup_y + popup_h - POPUP_BORDER,
        popup_w,
        POPUP_BORDER,
        POPUP_BORDER_COLOR,
    );
    fb.fill_rect(popup_x, popup_y, POPUP_BORDER, popup_h, POPUP_BORDER_COLOR);
    fb.fill_rect(
        popup_x + popup_w - POPUP_BORDER,
        popup_y,
        POPUP_BORDER,
        popup_h,
        POPUP_BORDER_COLOR,
    );

    let title_y = popup_y + POPUP_PADDING;
    fb.fill_rect(
        popup_x + POPUP_BORDER,
        title_y,
        popup_w - POPUP_BORDER * 2,
        16,
        POPUP_TITLE_BG,
    );
    fb.draw_string(
        popup_x + (popup_w - title_width) / 2,
        title_y,
        title_text,
        TEXT_COLOR,
        POPUP_TITLE_BG,
    );

    let msg_y = title_y + 16 + POPUP_PADDING;
    let msg_x = popup_x + (popup_w - msg_width) / 2;
    draw_string_safe(fb, msg_x, msg_y, message, TEXT_COLOR, POPUP_BG);

    let ok_text = "OK";
    let ok_text_w = ok_text.len() * 8;
    let btn_w = ok_text_w + BUTTON_PADDING * 2;
    let btn_h = BUTTON_HEIGHT;
    let btn_x = popup_x + (popup_w - btn_w) / 2;
    let btn_y = popup_y + popup_h - BUTTON_PADDING - btn_h;

    fb.fill_rect(btn_x, btn_y, btn_w, btn_h, BUTTON_BG);
    fb.fill_rect(btn_x, btn_y, btn_w, 1, POPUP_BORDER_COLOR);
    fb.fill_rect(btn_x, btn_y + btn_h - 1, btn_w, 1, POPUP_BORDER_COLOR);
    fb.fill_rect(btn_x, btn_y, 1, btn_h, POPUP_BORDER_COLOR);
    fb.fill_rect(btn_x + btn_w - 1, btn_y, 1, btn_h, POPUP_BORDER_COLOR);

    fb.draw_string(
        btn_x + (btn_w - ok_text_w) / 2,
        btn_y + (btn_h - 16) / 2,
        ok_text,
        BUTTON_TEXT,
        BUTTON_BG,
    );

    let mut keyboard = Keyboard::new();
    loop {
        match keyboard.poll() {
            Key::Escape | Key::Char(b'\n') | Key::Char(b'\r') | Key::Char(b' ') | Key::Char(_) => {
                break;
            }
            _ => {}
        }
        for _ in 0..1000 {
            unsafe {
                core::arch::asm!("nop", options(nomem, nostack));
            }
        }
    }
}

fn draw_string_safe(fb: &mut Framebuffer, x: usize, y: usize, s: &str, fg: Color, bg: Color) {
    let mut cx = x;
    for ch in s.chars() {
        if ch as u32 > 127 {
            fb.draw_glyph(cx, y, '?', fg, bg);
        } else {
            fb.draw_glyph(cx, y, ch, fg, bg);
        }
        cx += 8;
    }
}
