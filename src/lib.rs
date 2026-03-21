#![no_std]
#![no_main]
#![allow(dead_code)]
#![feature(global_allocator)]

extern crate spin;

mod ahci;
mod allocator;
mod ata;
mod berkefs;
mod boot;
mod deno;
mod disk_io;
mod disk_paths;
mod editor;
mod font;
mod framebuffer;
mod idt;
mod keyboard;

mod net;
mod paging;
mod pcspeaker;
mod pic;
mod pit;
mod process;
mod rtc;
mod rtl8139;
mod scheduler;
mod shell;
mod syscall;
mod term;
mod usb;
mod vga;

use berkefs::BerkeFS;
use core::panic::PanicInfo;
use framebuffer::Framebuffer;
use shell::Shell;
use spin::{Mutex, MutexGuard};

use allocator::{KernelAllocator, HEAP_SIZE, HEAP_START};

#[global_allocator]
static KERNEL_ALLOCATOR: KernelAllocator = KernelAllocator::new();

#[repr(C)]
pub struct FbInfo {
    pub addr: u64,
    pub pitch: u32,
    pub width: u32,
    pub height: u32,
    pub bpp: u8,
    pub fb_type: u8,
}

// ── VGA diagnostic helpers ────────────────────────────────────────────────────
// Ekran karti var mi yokmu kontrol ediyoruz - checking if display card exists
fn vga_probe() -> bool {
    // Probe VGA: try to write and read back
    // VGA'ya yazip okuyoruz, tutarsa var - writing to vga, if it sticks then it exists
    unsafe {
        let vga = 0xb8000 as *mut u16;
        let orig = vga.read_volatile();
        vga.write_volatile(0xAA55);
        let result = vga.read_volatile();
        vga.write_volatile(orig);
        result == 0xAA55
    }
}

static VGA_AVAILABLE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(true);

// Ekrani temizliyoruz boslukla - clearing screen with spaces
unsafe fn vga_clear() {
    if !VGA_AVAILABLE.load(core::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let vga = 0xb8000 as *mut u8;
    for i in 0..80 * 25 {
        vga.add(i * 2).write_volatile(b' ');
        vga.add(i * 2 + 1).write_volatile(0x07);
    }
}

// Belirli pozisyona yaziyoruz - writing text to specific position
unsafe fn vga_print(row: usize, col: usize, msg: &str, attr: u8) {
    if !VGA_AVAILABLE.load(core::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let vga = 0xb8000 as *mut u8;
    for (i, b) in msg.bytes().enumerate() {
        if col + i >= 80 {
            break;
        }
        let off = (row * 80 + col + i) * 2;
        vga.add(off).write_volatile(b);
        vga.add(off + 1).write_volatile(attr);
    }
}

// 32 bit hex yazdirma - printing 32bit hexadecimal value
unsafe fn vga_hex32(row: usize, col: usize, val: u32, attr: u8) {
    if !VGA_AVAILABLE.load(core::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let vga = 0xb8000 as *mut u8;
    for i in 0..8 {
        let nibble = ((val >> (28 - i * 4)) & 0xF) as u8;
        let ch = if nibble < 10 {
            b'0' + nibble
        } else {
            b'a' + nibble - 10
        };
        let off = (row * 80 + col + i) * 2;
        vga.add(off).write_volatile(ch);
        vga.add(off + 1).write_volatile(attr);
    }
}

// 64 bit hex yazdirma - printing 64bit hexadecimal
unsafe fn vga_hex64(row: usize, col: usize, val: u64, attr: u8) {
    vga_hex32(row, col, (val >> 32) as u32, attr);
    vga_hex32(row, col + 8, val as u32, attr);
}

// ── Static shell storage — avoids blowing the 64KiB stack ────────────────────
// Shell'i static yapmam lazim ki stack tasmasin - making shell static to avoid stack overflow
static SHELL: Mutex<Shell> = Mutex::new(Shell::new_static());

// ── Per-drive BerkeFS instances (12 drives: Alpha..Mu) ───────────────────────
// Drive index: 0=Alpha, 1=Beta, 2=Gamma, 3=Sigma, 4=Epsilon, 5=Zeta,
//              6=Eta, 7=Theta, 8=Iota, 9=Kappa, 10=Lambda, 11=Mu
static mut FS0: Mutex<BerkeFS> = Mutex::new(BerkeFS::new(0)); // Alpha (ATA/SATA disk)
static mut FS1: Mutex<BerkeFS> = Mutex::new(BerkeFS::new(1)); // Beta  (IDE disk)
static mut FS2: Mutex<BerkeFS> = Mutex::new(BerkeFS::new(2)); // Gamma
static mut FS3: Mutex<BerkeFS> = Mutex::new(BerkeFS::new(3)); // Sigma
static mut FS4: Mutex<BerkeFS> = Mutex::new(BerkeFS::new(4)); // Epsilon
static mut FS5: Mutex<BerkeFS> = Mutex::new(BerkeFS::new(5)); // Zeta
static mut FS6: Mutex<BerkeFS> = Mutex::new(BerkeFS::new(6)); // Eta
static mut FS7: Mutex<BerkeFS> = Mutex::new(BerkeFS::new(7)); // Theta
static mut FS8: Mutex<BerkeFS> = Mutex::new(BerkeFS::new(8)); // Iota
static mut FS9: Mutex<BerkeFS> = Mutex::new(BerkeFS::new(9)); // Kappa
static mut FS10: Mutex<BerkeFS> = Mutex::new(BerkeFS::new(10)); // Lambda
static mut FS11: Mutex<BerkeFS> = Mutex::new(BerkeFS::new(11)); // Mu

/// Returns a mutable reference to the BerkeFS instance for the given drive index.
/// Bu fonksiyon hangi drive'in filesystem'ini dondurdugunu soyluyor - this func returns the fs for the requested drive
fn get_fs(drive_idx: usize) -> Option<MutexGuard<'static, BerkeFS>> {
    match drive_idx {
        0 => Some(unsafe { FS0.lock() }),
        1 => Some(unsafe { FS1.lock() }),
        2 => Some(unsafe { FS2.lock() }),
        3 => Some(unsafe { FS3.lock() }),
        4 => Some(unsafe { FS4.lock() }),
        5 => Some(unsafe { FS5.lock() }),
        6 => Some(unsafe { FS6.lock() }),
        7 => Some(unsafe { FS7.lock() }),
        8 => Some(unsafe { FS8.lock() }),
        9 => Some(unsafe { FS9.lock() }),
        10 => Some(unsafe { FS10.lock() }),
        11 => Some(unsafe { FS11.lock() }),
        _ => None,
    }
}

// ── Kernel entry ──────────────────────────────────────────────────────────────
// ISTE TAS MALAM GIBI BASLADI - HERE WE GOOOO boot started right here
#[no_mangle]
pub extern "C" fn kernel_main(mb2_info_ptr: u32) -> ! {
    // Probe for VGA availability early
    // Once VGA var mi yok mu kontrol - first checking if vga exists or not
    let vga_exists = vga_probe();
    VGA_AVAILABLE.store(vga_exists, core::sync::atomic::Ordering::Relaxed);

    // Show loading spinner if VGA exists
    // Yukleniyor spinner'i gosterelim - showing loading spinner
    let spinner_chars = [b'-', b'\\', b'|', b'/'];

    for i in 0..20 {
        if vga_exists {
            unsafe {
                let vga = 0xb8000 as *mut u16;
                vga.offset(0)
                    .write_volatile((spinner_chars[(i as usize) % 4] as u16) | 0x0F00);
            }
        }
        for _ in 0..100000 {}
    }

    let fb_info = unsafe { parse_mb2_framebuffer(mb2_info_ptr) };
    // Multiboot2'den framebuffer bilgisi aldik - got framebuffer info from multiboot2

    match fb_info {
        Some(info) => {
            if vga_exists {
                unsafe {
                    vga_clear();
                }
                unsafe {
                    vga_print(0, 0, "BerkeOS v0.6.2 booting...", 0x0a);
                }
            }

            for i in 0..5 {
                if vga_exists {
                    unsafe {
                        vga_print(1, i * 10, "[==========]", 0x0a);
                    }
                }
                for _ in 0..500000 {}
            }

            let mut fb = unsafe { Framebuffer::new(info) };
            // Framebuffer'i olusturduk - created the framebuffer
            let w = fb.width;
            let h = fb.height;

            unsafe {
                vga_print(2, 0, "Launching shell...", 0x0a);
            }

            // INTERRUPT ZAMANI - INTERRUPT TIME!!! kesmeleri ayarliyoruz burda
            // IDT, PIC, PIT, scheduler hepsini burda baslatiyoruz
            unsafe {
                idt::init();
                // IDT: Interrupt Descriptor Table - tuslardan gelen sinyalleri isliyor
                pic::init();
                // PIC: Programmable Interrupt Controller - donanim kesmelerini yonetiyor
                pit::init(100);
                // PIT: Programmable Interval Timer - 100Hz clock tick
                scheduler::init();
                // Scheduler: process'leri zamanliyor, cpu'yu paylastiriyor
                pic::enable();
                vga_print(2, 20, "IDT+PIC+PIT+SCHED OK", 0x0a);
            }

            let mut fs = unsafe { FS0.lock() };
            // FS0 = Alpha drive - ilk diskimiz

            // DISK BULMA ZAMANI - DISK DETECTION TIME
            // Once ATA'ya bak, yoksa SATA/AHCI'ye gec - first check ATA, if not then try SATA/AHCI
            let ata_ok = unsafe { ata::ata_detect() };
            // ATA: eski paralel ATA/PATA disk - old parallel ATA/PATA disk

            let sata_ok = if !ata_ok {
                // ATA yok, SATA deneyelim - ATA not found, let's try SATA
                unsafe { ahci::ahci_init() }
                // AHCI: modern SATA arabirimi - modern SATA interface
            } else {
                false
            };

            let disk_ok = sata_ok || ata_ok;
            // Herhangi bir disk bulundu mu? - was any disk found?

            unsafe {
                vga_print(3, 0, "ATA:    ", 0x08);
                if ata_ok {
                    vga_print(3, 5, "OK   ", 0x0a);
                } else {
                    vga_print(3, 5, "FAIL ", 0x0c);
                }
                vga_print(3, 11, "SATA:   ", 0x08);
                if sata_ok {
                    vga_print(3, 17, "OK   ", 0x0a);
                } else {
                    vga_print(3, 17, "FAIL ", 0x0c);
                }
                vga_print(3, 23, "DISK:   ", 0x08);
                if disk_ok {
                    vga_print(3, 29, "OK   ", 0x0a);
                } else {
                    vga_print(3, 29, "FAIL ", 0x0c);
                }
            }

            // BERKEFS MOUNT ZAMANI - BerkeFS mount time
            // Dosya sistemini takmaya calisiyoruz - trying to mount the filesystem
            if disk_ok && ata_ok {
                // ATA disk var, BerkeFS'i takalim - ATA disk exists, let's mount BerkeFS
                unsafe {
                    vga_print(3, 20, "ATA disk OK", 0x0a);
                }
                if !fs.mount() {
                    // Disk bos veya tanimsiz, format atmamiz lazim
                    // Disk empty or undefined, we need to format it
                    unsafe {
                        vga_print(3, 40, "Formatting...", 0x0e);
                    }
                    fs.format(b"Alpha");
                    // Alpha disk olarak formatladik - formatted as Alpha disk
                } else {
                    unsafe {
                        vga_print(3, 40, "BerkeFS mounted", 0x0a);
                    }
                    // Basarili! BerkeFS takildi - success! BerkeFS mounted
                }
            } else if sata_ok {
                // SATA disk bulundu - SATA disk found
                unsafe {
                    vga_print(3, 0, "SATA (AHCI) detected!", 0x0a);
                }
                if !fs.mount() {
                    // Yine bos disk, format atalim - still empty disk, let's format
                    unsafe {
                        vga_print(3, 40, "Formatting...", 0x0e);
                    }
                    fs.format(b"Alpha");
                } else {
                    unsafe {
                        vga_print(3, 40, "BerkeFS mounted", 0x0a);
                    }
                }
            } else {
                unsafe {
                    vga_print(3, 40, "Live USB Mode!", 0x0a);
                }
            }

            // Beta is now a real IDE disk - just mark it as mounted
            // Beta disk'i de mount olarak isaretle - also mark Beta disk as mounted
            unsafe { FS1.lock().set_mounted() }

            if vga_exists {
                unsafe {
                    vga_print(4, 0, "[====================] DONE!", 0x0a);
                }
            }

            let mut shell = SHELL.lock();
            // SHELL'I BASLAT - STARTING THE SHELL
            let disk_count = ata::get_disk_count();
            // Kac disk var? - how many disks?
            shell.init(
                w,
                h,
                disk_ok,
                disk_count,
                unsafe { &mut FS0 },
                unsafe { &mut FS1 },
                unsafe { &mut FS2 },
                unsafe { &mut FS3 },
                unsafe { &mut FS4 },
                unsafe { &mut FS5 },
                unsafe { &mut FS6 },
                unsafe { &mut FS7 },
                unsafe { &mut FS8 },
                unsafe { &mut FS9 },
                unsafe { &mut FS10 },
                unsafe { &mut FS11 },
            );

            fb.clear(framebuffer::Color::rgb(0x00, 0x00, 0x00));
            // Ekrani siyaha siliyoruz - clearing screen to black

            // RENKLER - COLORS
            let green = framebuffer::Color::rgb(0x00, 0xFF, 0x00);
            let white = framebuffer::Color::rgb(0xFF, 0xFF, 0xFF);
            let red = framebuffer::Color::rgb(0xFF, 0x00, 0x00);
            let cyan = framebuffer::Color::rgb(0x00, 0xFF, 0xFF);

            // BERKEOS ASCCI ART BASLANGICI - BerkeOS ASCII art begins
            fb.draw_string(
                25,
                2,
                "██████╗ ███████╗████████╗██████╗  ██████╗ ",
                white,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            fb.draw_string(
                25,
                3,
                "██╔══██╗██╔════╝╚══██╔══╝██╔══██╗██╔═══██╗",
                white,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            fb.draw_string(
                25,
                4,
                "██████╔╝█████╗     ██║   ██████╔╝██║   ██║",
                white,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            fb.draw_string(
                25,
                5,
                "██╔══██╗██╔══╝     ██║   ██╔══██╗██║   ██║",
                white,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            fb.draw_string(
                25,
                6,
                "██║  ██║███████╗   ██║   ██║  ██║╚██████╔╝",
                white,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            fb.draw_string(
                25,
                7,
                "╚═╝  ╚═╝╚══════╝   ╚═╝   ╚═╝  ╚═╝ ╚═════╝ ",
                white,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );

            fb.draw_string(
                30,
                9,
                "  BerkeOS v0.6.2 - Boot Sequence  ",
                cyan,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );

            fb.draw_string(
                10,
                13,
                "Initializing BerkeOS...",
                white,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            pit::sleep_ms(1000);
            fb.draw_string(
                40,
                13,
                "[  OK  ]",
                green,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            // BerkeOS hazir! - BerkeOS is ready!

            fb.draw_string(
                10,
                14,
                "Loading memory manager...",
                white,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            pit::sleep_ms(1000);
            fb.draw_string(
                40,
                14,
                "[  OK  ]",
                green,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            // Hafiza yonetimi hazir - memory management ready

            fb.draw_string(
                10,
                15,
                "Setting up interrupts...",
                white,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            pit::sleep_ms(1000);
            fb.draw_string(
                40,
                15,
                "[  OK  ]",
                green,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            // Kesmeler ayarlandi - interrupts configured

            fb.draw_string(
                10,
                16,
                "Initializing keyboard...",
                white,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            pit::sleep_ms(1000);
            fb.draw_string(
                40,
                16,
                "[  OK  ]",
                green,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            // Klavye hazir, yazmaya hazir ol! - keyboard ready, get ready to type!

            fb.draw_string(
                10,
                17,
                "Initializing storage...",
                white,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            pit::sleep_ms(1000);
            fb.draw_string(
                40,
                17,
                "[  OK  ]",
                green,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            // Depolama hazir, dosyalarin agerisinde - storage ready, your files await

            fb.draw_string(
                10,
                18,
                "Starting shell...",
                white,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            pit::sleep_ms(1000);
            fb.draw_string(
                40,
                18,
                "[  OK  ]",
                green,
                framebuffer::Color::rgb(0x00, 0x00, 0x00),
            );
            // SHELL BASLADI! Artik komut gir! - SHELL STARTED! Now enter your commands!

            fb.clear(framebuffer::Color::rgb(0x00, 0x00, 0x00));
            // Boot tamamlandi, shell'e gecis yapildi - boot complete, transition to shell

            shell.run(&mut fb);
        }

        None => {
            // No framebuffer - use VGA text mode
            // FB yok, VGA text mode'a gecis yap - no framebuffer, switching to VGA text mode
            // Bu durumda daha az guzel ama calisiyor - less pretty but it works
            if vga_exists {
                let v = vga::Vga::new();
                v.clear(vga::Color::Black);
                v.fill_row(0, vga::Color::Blue);
                v.print_at(
                    1,
                    0,
                    "BerkeOS v0.6.2 booting...",
                    vga::Color::White,
                    vga::Color::Blue,
                );
                v.fill_row(2, vga::Color::Black);
                v.print_at(
                    1,
                    2,
                    "Detecting hardware...",
                    vga::Color::Yellow,
                    vga::Color::Black,
                );

                // Initialize basics
                // Temel seyleri baslat - starting the basics
                // VGA text mode olsa bile interruptlar lazim - still need interrupts even in VGA text mode
                unsafe {
                    idt::init();
                    pic::init();
                    pit::init(100);
                    scheduler::init();
                    pic::enable();
                }

                v.print_at(
                    1,
                    3,
                    "Checking storage...",
                    vga::Color::Yellow,
                    vga::Color::Black,
                );
                let sata_ok = unsafe { ahci::ahci_init() };
                let ata_ok = if !sata_ok {
                    unsafe { ata::ata_detect() }
                } else {
                    false
                };

                if sata_ok {
                    v.print_at(1, 4, "SATA (AHCI) OK", vga::Color::Green, vga::Color::Black);
                } else if ata_ok {
                    v.print_at(1, 4, "ATA disk OK", vga::Color::Green, vga::Color::Black);
                } else {
                    v.print_at(1, 4, "Live USB Mode", vga::Color::Cyan, vga::Color::Black);
                }

                v.fill_row(6, vga::Color::Blue);
                v.print_at(
                    1,
                    6,
                    "Loading BerkeOS...",
                    vga::Color::White,
                    vga::Color::Blue,
                );

                let mut fs = unsafe { FS0.lock() };
                if !fs.mount() {
                    v.print_at(
                        1,
                        7,
                        "Formatting drive...",
                        vga::Color::Yellow,
                        vga::Color::Black,
                    );
                    fs.format(b"Alpha");
                }

                v.fill_row(24, vga::Color::Blue);
                v.print_at(
                    1,
                    24,
                    "BerkeOS v0.6.2 | Berke Oruc | Rust | x86_64",
                    vga::Color::White,
                    vga::Color::Blue,
                );
            } else {
                // No VGA, no framebuffer - halt with error
                loop {}
            }
        }
    }

    loop {}
}

// ── Multiboot2 framebuffer tag parser ─────────────────────────────────────────
// Multiboot2 taglarini parse ediyoruz - parsing multiboot2 tags
// GRUB bize framebuffer bilgisi gonderiyor - GRUB sends us framebuffer info
// Multiboot2 taglarini parse ediyoruz - parsing multiboot2 tags
// GRUB bize framebuffer bilgisi gonderiyor - GRUB sends us framebuffer info
unsafe fn parse_mb2_framebuffer(mb2_ptr: u32) -> Option<FbInfo> {
    if mb2_ptr == 0 {
        return None;
    }

    let total_size = (mb2_ptr as *const u32).read_volatile();

    unsafe {
        vga_print(24, 0, "MB2 sz=0x", 0x08);
    }
    unsafe {
        vga_hex32(24, 9, total_size, 0x08);
    }

    if total_size < 8 || total_size > 0x20000 {
        return None;
    }

    let mut offset: u32 = 8;
    let mut tag_count: u32 = 0;

    while offset + 8 <= total_size {
        let tag_addr = mb2_ptr + offset;
        let tag_type = (tag_addr as *const u32).read_volatile();
        let tag_size = ((tag_addr + 4) as *const u32).read_volatile();

        if tag_count < 6 {
            let col = (tag_count * 13) as usize;
            if col + 8 < 80 {
                vga_hex32(23, col, tag_type, 0x08);
            }
        }
        tag_count += 1;

        if tag_type == 0 {
            break;
        }

        // Tag type 8 = framebuffer info
        if tag_type == 8 && tag_size >= 31 {
            let base = tag_addr as *const u8;
            let addr = (base.add(8) as *const u64).read_volatile();
            let pitch = (base.add(16) as *const u32).read_volatile();
            let width = (base.add(20) as *const u32).read_volatile();
            let height = (base.add(24) as *const u32).read_volatile();
            let bpp = base.add(28).read_volatile();
            let fb_type = base.add(29).read_volatile();

            vga_print(3, 0, "FB tag: addr=0x", 0x0b);
            vga_hex64(3, 15, addr, 0x0b);
            vga_print(3, 32, " bpp=", 0x0b);
            {
                let vga = 0xb8000 as *mut u8;
                vga.add((3 * 80 + 37) * 2).write_volatile(b'0' + bpp / 10);
                vga.add((3 * 80 + 37) * 2 + 1).write_volatile(0x0b);
                vga.add((3 * 80 + 38) * 2).write_volatile(b'0' + bpp % 10);
                vga.add((3 * 80 + 38) * 2 + 1).write_volatile(0x0b);
            }
            vga_print(3, 40, " type=", 0x0b);
            {
                let vga = 0xb8000 as *mut u8;
                vga.add((3 * 80 + 46) * 2).write_volatile(b'0' + fb_type);
                vga.add((3 * 80 + 46) * 2 + 1).write_volatile(0x0b);
            }

            // Accept any pixel framebuffer (type 0 or 1), any bpp >= 8
            // Do NOT check addr upper bound — framebuffer can be anywhere in 4GiB
            if addr != 0 && width > 0 && height > 0 && bpp >= 8 && fb_type != 2 {
                vga_print(4, 0, "FB ACCEPTED!", 0x0a);
                return Some(FbInfo {
                    addr,
                    pitch,
                    width,
                    height,
                    bpp,
                    fb_type,
                });
            } else {
                vga_print(4, 0, "FB rejected:", 0x0c);
                if addr == 0 {
                    vga_print(4, 13, "addr=0", 0x0c);
                }
                if width == 0 {
                    vga_print(4, 20, "w=0", 0x0c);
                }
                if height == 0 {
                    vga_print(4, 24, "h=0", 0x0c);
                }
                if bpp < 8 {
                    vga_print(4, 28, "bpp<8", 0x0c);
                }
                if fb_type == 2 {
                    vga_print(4, 34, "text", 0x0c);
                }
            }
        }

        let aligned = (tag_size + 7) & !7;
        if aligned == 0 {
            break;
        }
        offset += aligned;
    }

    None
}

// ── Panic handler ─────────────────────────────────────────────────────────────
// COK KOTU BIR HATA OLDU - SOMETHING WENT TERRIBLY WRONG
// Kernel panic! Sistem durdu. - Kernel panic! System halted.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe {
        let vga = 0xb8000 as *mut u8;
        for i in 0..80 {
            vga.add(i * 2).write_volatile(b' ');
            vga.add(i * 2 + 1).write_volatile(0x4f);
        }
        let msg = b"!!! KERNEL PANIC !!!";
        for (i, &b) in msg.iter().enumerate() {
            vga.add(i * 2).write_volatile(b);
            vga.add(i * 2 + 1).write_volatile(0x4f);
        }
    }
    loop {}
}
