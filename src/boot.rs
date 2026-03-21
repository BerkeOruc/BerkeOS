// BerkeOS — boot.rs
// Boot information detection

use spin::Mutex;

#[derive(Copy, Clone)]
pub struct BootInfo {
    pub boot_disk: u8,
    pub boot_device: u32,
    pub kernel_loaded: bool,
    pub multiboot_magic: u32,
}

impl BootInfo {
    pub const fn new() -> Self {
        BootInfo {
            boot_disk: 0,
            boot_device: 0,
            kernel_loaded: false,
            multiboot_magic: 0,
        }
    }

    pub fn detect_boot_device(&mut self) {
        // BIOS boot disk is passed in DL register by bootloader
        // For now, default to 0x80 (first HDD)
        self.boot_disk = 0x80;
        self.boot_device = 0;
    }

    pub fn get_boot_type(&self) -> &'static str {
        match self.boot_disk {
            0x00..=0x7F => "Floppy",
            0x80..=0x8F => "HDD",
            0xE0..=0xEF => "USB",
            _ => "Unknown",
        }
    }

    pub fn is_usb_boot(&self) -> bool {
        self.boot_disk >= 0xE0 && self.boot_disk <= 0xEF
    }
}

pub static BOOT_INFO: Mutex<BootInfo> = Mutex::new(BootInfo::new());

pub fn boot_init(boot_disk: u8, magic: u32) {
    let mut info = BOOT_INFO.lock();
    info.boot_disk = boot_disk;
    info.multiboot_magic = magic;
    info.kernel_loaded = magic == 0x36d76289;
    info.detect_boot_device();
}

pub fn get_boot_info() -> spin::MutexGuard<'static, BootInfo> {
    BOOT_INFO.lock()
}
