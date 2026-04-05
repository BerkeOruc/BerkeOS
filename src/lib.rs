#![no_std]
#![no_main]
#![allow(dead_code)]

extern crate spin;

mod boot;
mod drivers;
mod etc;
mod fs;
mod graphics;
mod memory;
mod net;
mod process;
mod syscall;
mod ui;
mod vm;

use fs::berkefs::BerkeFS;
use core::panic::PanicInfo;
use drivers::framebuffer::{Framebuffer, Color as FbColor};
use drivers::vga::{Vga, Color};
use drivers::serial;
use drivers::idt;
use drivers::pic;
use drivers::pit;
use drivers::ata;
use drivers::ahci;
use process::scheduler;
use ui::shell::Shell;
use spin::{Mutex, MutexGuard};

use memory::allocator::KernelAllocator;

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

/// Drive names indexed by drive number
/// Drive index: 0=Alpha, 1=Beta, 2=Gamma, 3=Sigma, 4=Epsilon, 5=Zeta,
///              6=Eta, 7=Theta, 8=Iota, 9=Kappa, 10=Lambda, 11=Mu
const DRIVE_NAMES: [&str; 12] = [
    "Alpha",   // 0
    "Beta",    // 1
    "Gamma",   // 2
    "Sigma",   // 3
    "Epsilon", // 4
    "Zeta",    // 5
    "Eta",     // 6
    "Theta",   // 7
    "Iota",    // 8
    "Kappa",   // 9
    "Lambda",  // 10
    "Mu",      // 11
];

/// Registry of all BerkeFS drive instances
/// Replaces the 12 individual static mut FS0..FS11 variables
struct DriveRegistry {
    /// Array of 12 drive instances (Alpha through Mu)
    drives: [Mutex<BerkeFS>; 12],
}

impl DriveRegistry {
    /// Creates a new DriveRegistry with all drives initialized
    const fn new() -> Self {
        Self {
            drives: [
                Mutex::new(BerkeFS::new(0)),  // Alpha (ATA/SATA disk)
                Mutex::new(BerkeFS::new(1)),  // Beta  (IDE disk)
                Mutex::new(BerkeFS::new(2)),  // Gamma
                Mutex::new(BerkeFS::new(3)),  // Sigma
                Mutex::new(BerkeFS::new(4)),  // Epsilon
                Mutex::new(BerkeFS::new(5)),  // Zeta
                Mutex::new(BerkeFS::new(6)),  // Eta
                Mutex::new(BerkeFS::new(7)),  // Theta
                Mutex::new(BerkeFS::new(8)),  // Iota
                Mutex::new(BerkeFS::new(9)),  // Kappa
                Mutex::new(BerkeFS::new(10)), // Lambda
                Mutex::new(BerkeFS::new(11)), // Mu
            ],
        }
    }

    /// Returns a mutable reference to the BerkeFS instance for the given drive index.
    /// Returns None if the index is out of bounds (> 11).
    fn get_fs(&self, drive_idx: usize) -> Option<MutexGuard<'_, BerkeFS>> {
        self.drives.get(drive_idx).map(|m| m.lock())
    }
}

/// Global drive registry holding all 12 filesystem instances
static mut DRIVE_REGISTRY: DriveRegistry = DriveRegistry::new();

/// Returns a mutable reference to the BerkeFS instance for the given drive index.
/// Bu fonksiyon hangi drive'in filesystem'ini dondurdugunu soyluyor - this func returns the fs for the requested drive
fn get_fs(drive_idx: usize) -> Option<MutexGuard<'static, BerkeFS>> {
    unsafe { DRIVE_REGISTRY.get_fs(drive_idx) }
}

/// Returns raw pointers to all drive Mutex instances for Shell initialization
pub fn get_drive_ptrs() -> [*mut Mutex<BerkeFS>; 12] {
    unsafe {
        let registry = &mut DRIVE_REGISTRY;
        [
            core::ptr::addr_of_mut!(registry.drives[0]),
            core::ptr::addr_of_mut!(registry.drives[1]),
            core::ptr::addr_of_mut!(registry.drives[2]),
            core::ptr::addr_of_mut!(registry.drives[3]),
            core::ptr::addr_of_mut!(registry.drives[4]),
            core::ptr::addr_of_mut!(registry.drives[5]),
            core::ptr::addr_of_mut!(registry.drives[6]),
            core::ptr::addr_of_mut!(registry.drives[7]),
            core::ptr::addr_of_mut!(registry.drives[8]),
            core::ptr::addr_of_mut!(registry.drives[9]),
            core::ptr::addr_of_mut!(registry.drives[10]),
            core::ptr::addr_of_mut!(registry.drives[11]),
        ]
    }
}

// ── Kernel entry ──────────────────────────────────────────────────────────────
// ISTE TAS MALAM GIBI BASLADI - HERE WE GOOOO boot started right here
#[no_mangle]
pub extern "C" fn kernel_main(mb2_info_ptr: u32) -> ! {
    // Initialize serial port FIRST - CI needs "BerkeOS" in serial output
    serial::init();
    serial::write_str("BerkeOS\r\n");

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
                    vga_print(0, 0, "BerkeOS v0.6.3 booting...", 0x0a);
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

            let mut fs = unsafe { DRIVE_REGISTRY.get_fs(0).expect("Drive 0 must exist") };
            // Alpha drive - ilk diskimiz

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
            if let Some(mut beta) = unsafe { DRIVE_REGISTRY.get_fs(1) } {
                beta.set_mounted();
            }

            if vga_exists {
                unsafe {
                    vga_print(4, 0, "[====================] DONE!", 0x0a);
                }
            }

            let mut shell = SHELL.lock();
            // SHELL'I BASLAT - STARTING THE SHELL
            let disk_count = ata::get_disk_count();
            // Kac disk var? - how many disks?
            let drive_ptrs = get_drive_ptrs();
            shell.init(
                w,
                h,
                disk_ok,
                disk_count,
                unsafe { &mut *drive_ptrs[0] },
                unsafe { &mut *drive_ptrs[1] },
                unsafe { &mut *drive_ptrs[2] },
                unsafe { &mut *drive_ptrs[3] },
                unsafe { &mut *drive_ptrs[4] },
                unsafe { &mut *drive_ptrs[5] },
                unsafe { &mut *drive_ptrs[6] },
                unsafe { &mut *drive_ptrs[7] },
                unsafe { &mut *drive_ptrs[8] },
                unsafe { &mut *drive_ptrs[9] },
                unsafe { &mut *drive_ptrs[10] },
                unsafe { &mut *drive_ptrs[11] },
            );

            fb.clear(FbColor::rgb(0x00, 0x00, 0x00));
            // Ekrani siyaha siliyoruz - clearing screen to black

            // RENKLER - COLORS
            let green = FbColor::rgb(0x00, 0xFF, 0x00);
            let white = FbColor::rgb(0xFF, 0xFF, 0xFF);
            let cyan = FbColor::rgb(0x00, 0xFF, 0xFF);

            // BERKEOS ASCCI ART BASLANGICI - BerkeOS ASCII art begins
            fb.draw_string(
                25,
                2,
                "██████╗ ███████╗████████╗██████╗  ██████╗ ",
                white,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            fb.draw_string(
                25,
                3,
                "██╔══██╗██╔════╝╚══██╔══╝██╔══██╗██╔═══██╗",
                white,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            fb.draw_string(
                25,
                4,
                "██████╔╝█████╗     ██║   ██████╔╝██║   ██║",
                white,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            fb.draw_string(
                25,
                5,
                "██╔══██╗██╔══╝     ██║   ██╔══██╗██║   ██║",
                white,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            fb.draw_string(
                25,
                6,
                "██║  ██║███████╗   ██║   ██║  ██║╚██████╔╝",
                white,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            fb.draw_string(
                25,
                7,
                "╚═╝  ╚═╝╚══════╝   ╚═╝   ╚═╝  ╚═╝ ╚═════╝ ",
                white,
                FbColor::rgb(0x00, 0x00, 0x00),
            );

            fb.draw_string(
                30,
                9,
                "  BerkeOS v0.6.3 - Boot Sequence  ",
                cyan,
                FbColor::rgb(0x00, 0x00, 0x00),
            );

            fb.draw_string(
                10,
                13,
                "Initializing BerkeOS...",
                white,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            pit::sleep_ms(1000);
            fb.draw_string(
                40,
                13,
                "[  OK  ]",
                green,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            // BerkeOS hazir! - BerkeOS is ready!

            fb.draw_string(
                10,
                14,
                "Loading memory manager...",
                white,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            pit::sleep_ms(1000);
            fb.draw_string(
                40,
                14,
                "[  OK  ]",
                green,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            // Hafiza yonetimi hazir - memory management ready

            fb.draw_string(
                10,
                15,
                "Setting up interrupts...",
                white,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            pit::sleep_ms(1000);
            fb.draw_string(
                40,
                15,
                "[  OK  ]",
                green,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            // Kesmeler ayarlandi - interrupts configured

            fb.draw_string(
                10,
                16,
                "Initializing keyboard...",
                white,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            pit::sleep_ms(1000);
            fb.draw_string(
                40,
                16,
                "[  OK  ]",
                green,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            // Klavye hazir, yazmaya hazir ol! - keyboard ready, get ready to type!

            fb.draw_string(
                10,
                17,
                "Initializing storage...",
                white,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            pit::sleep_ms(1000);
            fb.draw_string(
                40,
                17,
                "[  OK  ]",
                green,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            // Depolama hazir, dosyalarin agerisinde - storage ready, your files await

            fb.draw_string(
                10,
                18,
                "Starting shell...",
                white,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            pit::sleep_ms(1000);
            fb.draw_string(
                40,
                18,
                "[  OK  ]",
                green,
                FbColor::rgb(0x00, 0x00, 0x00),
            );
            // SHELL BASLADI! Artik komut gir! - SHELL STARTED! Now enter your commands!

            fb.clear(FbColor::rgb(0x00, 0x00, 0x00));
            // Boot tamamlandi, shell'e gecis yapildi - boot complete, transition to shell

            shell.run(&mut fb);
        }

        None => {
            // No framebuffer - use VGA text mode
            // FB yok, VGA text mode'a gecis yap - no framebuffer, switching to VGA text mode
            // Bu durumda daha az guzel ama calisiyor - less pretty but it works
            if vga_exists {
                let v = Vga::new();
                v.clear(Color::Black);
                v.fill_row(0, Color::Blue);
                v.print_at(
                    1,
                    0,
                    "BerkeOS v0.6.3 booting...",
                    Color::White,
                    Color::Blue,
                );
                v.fill_row(2, Color::Black);
                v.print_at(
                    1,
                    2,
                    "Detecting hardware...",
                    Color::Yellow,
                    Color::Black,
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
                    Color::Yellow,
                    Color::Black,
                );
                let sata_ok = unsafe { ahci::ahci_init() };
                let ata_ok = if !sata_ok {
                    unsafe { ata::ata_detect() }
                } else {
                    false
                };

                if sata_ok {
                    v.print_at(1, 4, "SATA (AHCI) OK", Color::Green, Color::Black);
                } else if ata_ok {
                    v.print_at(1, 4, "ATA disk OK", Color::Green, Color::Black);
                } else {
                    v.print_at(1, 4, "Live USB Mode", Color::Cyan, Color::Black);
                }

                v.fill_row(6, Color::Blue);
                v.print_at(
                    1,
                    6,
                    "Loading BerkeOS...",
                    Color::White,
                    Color::Blue,
                );

                let mut fs = unsafe { DRIVE_REGISTRY.get_fs(0).expect("Drive 0 must exist") };
                if !fs.mount() {
                    v.print_at(
                        1,
                        7,
                        "Formatting drive...",
                        Color::Yellow,
                        Color::Black,
                    );
                    fs.format(b"Alpha");
                }

                v.fill_row(24, Color::Blue);
                v.print_at(
                    1,
                    24,
                    "BerkeOS v0.6.3 | Berke Oruc | Rust | x86_64",
                    Color::White,
                    Color::Blue,
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
fn panic(info: &PanicInfo) -> ! {
    // Capture registers at panic time
    let rax: u64;
    let rbx: u64;
    let rcx: u64;
    let rdx: u64;
    let rbp: u64;
    let rsi: u64;
    let rdi: u64;
    let rsp: u64;
    let rip: u64;
    unsafe {
        core::arch::asm!(
            "mov {rax}, rax",
            "mov {rbx}, rbx",
            "mov {rcx}, rcx",
            "mov {rdx}, rdx",
            "mov {rbp}, rbp",
            "mov {rsi}, rsi",
            "mov {rdi}, rdi",
            "mov {rsp}, rsp",
            "lea {rip}, [rip]",
            rax = out(reg) rax,
            rbx = out(reg) rbx,
            rcx = out(reg) rcx,
            rdx = out(reg) rdx,
            rbp = out(reg) rbp,
            rsi = out(reg) rsi,
            rdi = out(reg) rdi,
            rsp = out(reg) rsp,
            rip = out(reg) rip,
        );
    }

    let msg: &str = "<panic>";

    // Extract location
    let (file, line) = if let Some(loc) = info.location() {
        (loc.file(), loc.line() as u64)
    } else {
        ("<unknown>", 0)
    };

    // VGA red screen
    unsafe {
        let vga = 0xb8000 as *mut u8;
        for i in 0..(80 * 25) {
            vga.add(i * 2).write_volatile(b' ');
            vga.add(i * 2 + 1).write_volatile(0x4f);
        }
        let panic_msg = b"!!! KERNEL PANIC !!!";
        for (i, &b) in panic_msg.iter().enumerate() {
            vga.add(i * 2).write_volatile(b);
            vga.add(i * 2 + 1).write_volatile(0x4f);
        }
    }

    // Serial output with full diagnostic info
    serial::write_str("=== KERNEL PANIC ===\r\n");
    serial::write_str("Message: ");
    serial::write_str(msg);
    serial::write_str("\r\n");
    serial::write_str("Location: ");
    serial::write_str(file);
    serial::write_str(":");
    write_u64_serial(line);
    serial::write_str("\r\n");
    serial::write_str("Stack: 0x");
    write_hex64_serial(rsp);
    serial::write_str("\r\n");
    serial::write_str("Registers:\r\n");
    serial::write_str("  RIP=");
    write_hex64_serial(rip);
    serial::write_str(" RAX=");
    write_hex64_serial(rax);
    serial::write_str(" RBX=");
    write_hex64_serial(rbx);
    serial::write_str("\r\n");
    serial::write_str("  RCX=");
    write_hex64_serial(rcx);
    serial::write_str(" RDX=");
    write_hex64_serial(rdx);
    serial::write_str("\r\n");
    serial::write_str("  RBP=");
    write_hex64_serial(rbp);
    serial::write_str(" RSI=");
    write_hex64_serial(rsi);
    serial::write_str("\r\n");
    serial::write_str("  RDI=");
    write_hex64_serial(rdi);
    serial::write_str(" RSP=");
    write_hex64_serial(rsp);
    serial::write_str("\r\n");
    serial::write_str("Halted.\r\n");

    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack));
        }
    }
}

// Write u64 as decimal to serial
fn write_u64_serial(mut val: u64) {
    if val == 0 {
        serial::write_str("0");
        return;
    }
    let mut buf = [0u8; 20];
    let mut len = 0;
    while val > 0 {
        buf[len] = b'0' + (val % 10) as u8;
        val /= 10;
        len += 1;
    }
    for i in (0..len).rev() {
        serial::write_byte(buf[i]);
    }
}

// Write u64 as 16-digit lowercase hex to serial
fn write_hex64_serial(val: u64) {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    for i in (0..16).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as usize;
        serial::write_byte(HEX_CHARS[nibble]);
    }
}
