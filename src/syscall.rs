// BerkeOS — syscall.rs
// System call interface

pub const SYS_EXIT: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_READ: u64 = 2;
pub const SYS_GETPID: u64 = 3;
pub const SYS_SLEEP: u64 = 4;
pub const SYS_YIELD: u64 = 5;
pub const SYS_OPEN: u64 = 6;
pub const SYS_CLOSE: u64 = 7;
pub const SYS_STAT: u64 = 8;
pub const SYS_MKDIR: u64 = 9;
pub const SYS_UNLINK: u64 = 10;
pub const SYS_UPTIME: u64 = 11;

pub const SYS_FOPEN: u64 = 12;
pub const SYS_FREAD: u64 = 13;
pub const SYS_FWRITE: u64 = 14;
pub const SYS_FCLOSE: u64 = 15;
pub const SYS_FSEEK: u64 = 16;
pub const SYS_FTELL: u64 = 17;
pub const SYS_MKDIR2: u64 = 18;
pub const SYS_DELETE: u64 = 19;
pub const SYS_RENAME: u64 = 20;
pub const SYS_EXISTS: u64 = 21;

pub const SYS_FB_INIT: u64 = 30;
pub const SYS_FB_PIXEL: u64 = 31;
pub const SYS_FB_RECT: u64 = 32;
pub const SYS_FB_CLEAR: u64 = 33;
pub const SYS_FB_TEXT: u64 = 34;
pub const SYS_FB_WIDTH: u64 = 35;
pub const SYS_FB_HEIGHT: u64 = 36;

pub const SYS_READ_KEY: u64 = 40;
pub const SYS_KEY_DOWN: u64 = 41;

pub const SYS_TTY_CLEAR: u64 = 50;
pub const SYS_TTY_GOTO: u64 = 51;
pub const SYS_TTY_COLOR: u64 = 52;

pub const SYS_INPUT: u64 = 53;

pub const SYS_WINDOW_NEW: u64 = 60;
pub const SYS_WINDOW_DRAW: u64 = 61;
pub const SYS_BUTTON_NEW: u64 = 62;
pub const SYS_LABEL_NEW: u64 = 63;
pub const SYS_INPUT_NEW: u64 = 64;

#[derive(Copy, Clone)]
pub struct SyscallResult {
    pub value: i64,
    pub error: i64,
}

impl SyscallResult {
    pub fn ok(v: i64) -> Self {
        SyscallResult { value: v, error: 0 }
    }
    pub fn err(e: i64) -> Self {
        SyscallResult {
            value: -1,
            error: e,
        }
    }
}

pub const ENOENT: i64 = 2;
pub const EBADF: i64 = 9;
pub const ENOMEM: i64 = 12;
pub const EINVAL: i64 = 22;
pub const ENOSYS: i64 = 38;

pub fn dispatch(num: u64, arg0: u64, arg1: u64, arg2: u64) -> SyscallResult {
    match num {
        SYS_EXIT => {
            sys_exit(arg0 as i32);
            SyscallResult::ok(0)
        }
        SYS_GETPID => {
            let pid = crate::scheduler::current_pid();
            SyscallResult::ok(pid as i64)
        }
        SYS_YIELD => {
            unsafe {
                crate::scheduler::schedule();
            }
            SyscallResult::ok(0)
        }
        SYS_UPTIME => {
            let ticks = crate::pic::uptime_ticks();
            SyscallResult::ok(ticks as i64)
        }
        SYS_WRITE => {
            if arg0 == 1 || arg0 == 2 {
                let len = (arg2 as usize).min(4096);
                SyscallResult::ok(len as i64)
            } else {
                SyscallResult::err(EBADF)
            }
        }

        SYS_FOPEN => {
            let path_ptr = arg0 as *const u8;
            let path_len = arg1 as usize;
            if path_ptr as usize == 0 {
                return SyscallResult::err(EINVAL);
            }
            unsafe {
                let path = core::slice::from_raw_parts(path_ptr, path_len.min(64));
                let mut path_buf = [0u8; 64];
                let n = path.len().min(63);
                path_buf[..n].copy_from_slice(&path[..n]);

                let fs = &mut *(&raw mut crate::FS0);
                let result = fs.lock().create_file(&path_buf, &[]);
                if result {
                    for fd in 3..10 {
                        if fs.lock().inodes[fd].ftype == crate::berkefs::FTYPE_FREE {
                            return SyscallResult::ok(fd as i64);
                        }
                    }
                    SyscallResult::ok(3)
                } else {
                    SyscallResult::err(ENOENT)
                }
            }
        }

        SYS_FREAD => {
            let fd = arg0 as usize;
            let buf_ptr = arg1 as *mut u8;
            let len = arg2 as usize;
            if fd < 3 || buf_ptr as usize == 0 {
                return SyscallResult::err(EBADF);
            }
            unsafe {
                let fs = &*(&raw mut crate::FS0);
                let fs_lock = fs.lock();
                if fd < fs_lock.inodes.len()
                    && fs_lock.inodes[fd].ftype == crate::berkefs::FTYPE_FILE
                {
                    let name = fs_lock.inodes[fd].get_name();
                    let mut read_buf = [0u8; 512];
                    if let Some(size) = fs_lock.read_file(name, &mut read_buf) {
                        let n = size.min(len);
                        core::slice::from_raw_parts_mut(buf_ptr, n).copy_from_slice(&read_buf[..n]);
                        return SyscallResult::ok(n as i64);
                    }
                }
                SyscallResult::ok(0)
            }
        }

        SYS_FWRITE => {
            let fd = arg0 as usize;
            let buf_ptr = arg1 as *const u8;
            let len = arg2 as usize;
            if fd < 3 || buf_ptr as usize == 0 {
                return SyscallResult::err(EBADF);
            }
            unsafe {
                let fs = &mut *(&raw mut crate::FS0);
                let mut fs_lock = fs.lock();
                if fd < fs_lock.inodes.len()
                    && fs_lock.inodes[fd].ftype == crate::berkefs::FTYPE_FILE
                {
                    let name = fs_lock.inodes[fd].get_name();
                    let mut name_buf = [0u8; 64];
                    let n = name.len().min(63);
                    name_buf[..n].copy_from_slice(&name[..n]);
                    let data = core::slice::from_raw_parts(buf_ptr, len.min(512));
                    let result = fs_lock.create_file(&name_buf, data);
                    if result {
                        return SyscallResult::ok(len as i64);
                    }
                }
                SyscallResult::err(EINVAL)
            }
        }

        SYS_FCLOSE => {
            let fd = arg0 as usize;
            if fd < 3 {
                return SyscallResult::err(EBADF);
            }
            SyscallResult::ok(0)
        }

        SYS_FSEEK => {
            let offset = arg1 as i64;
            SyscallResult::ok(offset)
        }

        SYS_FTELL => {
            let _fd = arg0 as usize;
            SyscallResult::ok(0)
        }

        SYS_MKDIR2 => {
            let path_ptr = arg0 as *const u8;
            let path_len = arg1 as usize;
            if path_ptr as usize == 0 {
                return SyscallResult::err(EINVAL);
            }
            unsafe {
                let path = core::slice::from_raw_parts(path_ptr, path_len.min(64));
                let mut path_buf = [0u8; 64];
                let n = path.len().min(63);
                path_buf[..n].copy_from_slice(&path[..n]);
                let fs = &mut *(&raw mut crate::FS0);
                let result = fs.lock().create_dir(&path_buf);
                if result {
                    SyscallResult::ok(0)
                } else {
                    SyscallResult::err(EINVAL)
                }
            }
        }

        SYS_DELETE => {
            let path_ptr = arg0 as *const u8;
            let path_len = arg1 as usize;
            if path_ptr as usize == 0 {
                return SyscallResult::err(EINVAL);
            }
            unsafe {
                let path = core::slice::from_raw_parts(path_ptr, path_len.min(64));
                let mut path_buf = [0u8; 64];
                let n = path.len().min(63);
                path_buf[..n].copy_from_slice(&path[..n]);
                let fs = &mut *(&raw mut crate::FS0);
                let result = fs.lock().delete_file(&path_buf);
                if result {
                    SyscallResult::ok(0)
                } else {
                    SyscallResult::err(ENOENT)
                }
            }
        }

        SYS_RENAME => SyscallResult::err(ENOSYS),

        SYS_EXISTS => {
            let path_ptr = arg0 as *const u8;
            let path_len = arg1 as usize;
            if path_ptr as usize == 0 {
                return SyscallResult::err(EINVAL);
            }
            unsafe {
                let path = core::slice::from_raw_parts(path_ptr, path_len.min(64));
                let mut path_buf = [0u8; 64];
                let n = path.len().min(63);
                path_buf[..n].copy_from_slice(&path[..n]);
                let fs = &*(&raw mut crate::FS0);
                let exists = fs.lock().path_exists(&path_buf);
                SyscallResult::ok(if exists { 1 } else { 0 })
            }
        }

        SYS_FB_INIT => SyscallResult::ok(0),
        SYS_FB_PIXEL => {
            let x = arg0 as usize;
            let y = arg1 as usize;
            let color = arg2 as u32;
            let width = 800;
            let height = 600;
            let bpp = 32;
            if x < width && y < height {
                let offset = (y * width + x) * (bpp as usize / 8);
                unsafe {
                    let fb_ptr = 0xe0000000 as *mut u8;
                    let r = ((color >> 16) & 0xFF) as u8;
                    let g = ((color >> 8) & 0xFF) as u8;
                    let b = (color & 0xFF) as u8;
                    fb_ptr.add(offset).write_volatile(b);
                    fb_ptr.add(offset + 1).write_volatile(g);
                    fb_ptr.add(offset + 2).write_volatile(r);
                    fb_ptr.add(offset + 3).write_volatile(0);
                }
            }
            SyscallResult::ok(0)
        }
        SYS_FB_RECT => {
            let x = arg0 as usize;
            let y = arg1 as usize;
            let w = arg2 as usize;
            let h = arg0 as usize;
            let color = arg1 as u32;
            let width = 800;
            let bpp = 32;
            for py in y..(y + h).min(600) {
                for px in x..(x + w).min(800) {
                    let offset = (py * width + px) * (bpp / 8);
                    unsafe {
                        let fb_ptr = 0xe0000000 as *mut u8;
                        let r = ((color >> 16) & 0xFF) as u8;
                        let g = ((color >> 8) & 0xFF) as u8;
                        let b = (color & 0xFF) as u8;
                        fb_ptr.add(offset).write_volatile(b);
                        fb_ptr.add(offset + 1).write_volatile(g);
                        fb_ptr.add(offset + 2).write_volatile(r);
                        fb_ptr.add(offset + 3).write_volatile(0);
                    }
                }
            }
            SyscallResult::ok(0)
        }
        SYS_FB_CLEAR => {
            let color = arg0 as u32;
            let r = ((color >> 16) & 0xFF) as u8;
            let g = ((color >> 8) & 0xFF) as u8;
            let b = (color & 0xFF) as u8;
            unsafe {
                let fb_ptr = 0xe0000000 as *mut u8;
                for i in 0..(800 * 600 * 4) {
                    fb_ptr.add(i * 4).write_volatile(b);
                    fb_ptr.add(i * 4 + 1).write_volatile(g);
                    fb_ptr.add(i * 4 + 2).write_volatile(r);
                    fb_ptr.add(i * 4 + 3).write_volatile(0);
                }
            }
            SyscallResult::ok(0)
        }
        SYS_FB_TEXT => SyscallResult::ok(0),
        SYS_FB_WIDTH => SyscallResult::ok(800),
        SYS_FB_HEIGHT => SyscallResult::ok(600),

        SYS_READ_KEY => {
            let mut key: u8 = 0;
            unsafe {
                let status = crate::keyboard::inb(0x64);
                if status & 1 != 0 {
                    key = crate::keyboard::inb(0x60);
                }
            }
            SyscallResult::ok(key as i64)
        }
        SYS_KEY_DOWN => {
            let key_code = arg0 as u8;
            let mut down = false;
            unsafe {
                let status = crate::keyboard::inb(0x64);
                if status & 1 != 0 {
                    let k = crate::keyboard::inb(0x60);
                    if k == key_code {
                        down = true;
                    }
                }
            }
            SyscallResult::ok(if down { 1 } else { 0 })
        }

        SYS_TTY_CLEAR => {
            for row in 0..25 {
                for col in 0..80 {
                    unsafe {
                        let ptr = (0xb8000 + row * 160 + col * 2) as *mut u8;
                        ptr.write_volatile(b' ');
                        ptr.add(1).write_volatile(0x07);
                    }
                }
            }
            SyscallResult::ok(0)
        }
        SYS_TTY_GOTO => {
            let x = arg0 as usize;
            let y = arg1 as usize;
            if x < 80 && y < 25 {
                let offset = y * 80 + x;
                unsafe {
                    let ptr = 0xb8000 as *mut u16;
                    ptr.add(offset).write_volatile(0x0720);
                }
            }
            SyscallResult::ok(0)
        }
        SYS_TTY_COLOR => SyscallResult::ok(0),

        SYS_INPUT => SyscallResult::ok(0),
        SYS_SLEEP => {
            let ticks = arg0 as u64;
            let end = crate::pic::uptime_ticks() + ticks * 100;
            while crate::pic::uptime_ticks() < end {
                unsafe {
                    crate::scheduler::schedule();
                }
            }
            SyscallResult::ok(0)
        }
        SYS_EXIT => {
            sys_exit(arg0 as i32);
            SyscallResult::ok(0)
        }

        SYS_WINDOW_NEW => SyscallResult::ok(0),
        SYS_WINDOW_DRAW => SyscallResult::ok(0),
        SYS_BUTTON_NEW => SyscallResult::ok(0),
        SYS_LABEL_NEW => SyscallResult::ok(0),
        SYS_INPUT_NEW => SyscallResult::ok(0),

        _ => SyscallResult::err(ENOSYS),
    }
}

fn sys_exit(code: i32) {
    let pid = crate::scheduler::current_pid();
    crate::scheduler::PTABLE.lock().kill(pid, code);
    unsafe {
        crate::scheduler::schedule();
    }
}

pub fn syscall_name(num: u64) -> &'static str {
    match num {
        SYS_EXIT => "exit",
        SYS_WRITE => "write",
        SYS_READ => "read",
        SYS_GETPID => "getpid",
        SYS_SLEEP => "sleep",
        SYS_YIELD => "yield",
        SYS_OPEN => "open",
        SYS_CLOSE => "close",
        SYS_STAT => "stat",
        SYS_MKDIR => "mkdir",
        SYS_UNLINK => "unlink",
        SYS_UPTIME => "uptime",
        SYS_FOPEN => "fopen",
        SYS_FREAD => "fread",
        SYS_FWRITE => "fwrite",
        SYS_FCLOSE => "fclose",
        SYS_FSEEK => "fseek",
        SYS_FTELL => "ftell",
        SYS_MKDIR2 => "mkdir2",
        SYS_DELETE => "delete",
        SYS_RENAME => "rename",
        SYS_EXISTS => "exists",
        SYS_FB_INIT => "fb_init",
        SYS_FB_PIXEL => "fb_pixel",
        SYS_FB_RECT => "fb_rect",
        SYS_FB_CLEAR => "fb_clear",
        SYS_FB_TEXT => "fb_text",
        SYS_FB_WIDTH => "fb_width",
        SYS_FB_HEIGHT => "fb_height",
        SYS_READ_KEY => "read_key",
        SYS_KEY_DOWN => "key_down",
        SYS_TTY_CLEAR => "tty_clear",
        SYS_TTY_GOTO => "tty_goto",
        SYS_TTY_COLOR => "tty_color",
        SYS_INPUT => "input",
        SYS_WINDOW_NEW => "window_new",
        SYS_WINDOW_DRAW => "window_draw",
        SYS_BUTTON_NEW => "button_new",
        SYS_LABEL_NEW => "label_new",
        SYS_INPUT_NEW => "input_new",
        _ => "unknown",
    }
}
