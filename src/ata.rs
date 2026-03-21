// BerkeOS — ata.rs
// ATA PIO Mode 28-bit LBA disk driver
// Polls the ATA bus — no IRQ, no DMA, no BIOS
// Works on QEMU's default IDE controller (primary bus, master drive)

use core::hint::spin_loop;
use core::sync::atomic::{AtomicUsize, Ordering};

// ── ATA I/O port base addresses ───────────────────────────────────────────────
// Primary IDE channel ports - standart IDE controller portlari bunlar
// Bunlar QEMU'da default olarak boyle, degistirmeye gerek yok
const ATA_PRIMARY_DATA: u16 = 0x1F0; // Data port - okuma yazma burdan oluyor
const ATA_PRIMARY_ERR: u16 = 0x1F1; // Error register - bir seyler yanlissa buraya yazar
const ATA_PRIMARY_COUNT: u16 = 0x1F2; // Sector count - kac sector okuyacagiz
const ATA_PRIMARY_LBA_LO: u16 = 0x1F3; // LBA low byte - adresin dusuk 8 biti
const ATA_PRIMARY_LBA_MID: u16 = 0x1F4; // LBA middle byte - adresin ortanca 8 biti
const ATA_PRIMARY_LBA_HI: u16 = 0x1F5; // LBA high byte - adresin yuksek 8 biti
const ATA_PRIMARY_DRIVE: u16 = 0x1F6; // Drive/head select - Alpha veya Beta'yi sec
const ATA_PRIMARY_CMD: u16 = 0x1F7; // Command register - komut yolladigimiz yer
const ATA_PRIMARY_STATUS: u16 = 0x1F7; // Status register - ne oluyor bak burdan
const ATA_PRIMARY_CTRL: u16 = 0x3F6; // Control register - reset vs isler burda

// ── ATA status bits ───────────────────────────────────────────────────────────
// Status bitleri - Device mesajlari buraya yaziyor
// Bakis acisindan onemli: BSY en kritik, onu ilk kontrol et
const ATA_STATUS_ERR: u8 = 0x01; // Error - bir seyler yanlis gitti
const ATA_STATUS_DRQ: u8 = 0x08; // Data Request - disk veri istiyor, oku/yaz zamani
const ATA_STATUS_SRV: u8 = 0x10; // Service - pek kullanmiyoruz
const ATA_STATUS_DF: u8 = 0x20; // Drive Fault - fiziksel sorun var
const ATA_STATUS_RDY: u8 = 0x40; // Ready - disk hazir
const ATA_STATUS_BSY: u8 = 0x80; // Busy - disk meşgul, bekle!

// ── ATA commands ──────────────────────────────────────────────────────────────
// Disk komutlari - bunlari CMD portuna yaziyorz
// PIO modunda calisiyoruz, DMA yok o yuzden biseyler elle yap
const ATA_CMD_READ_SECTORS: u8 = 0x20; // Sector oku - en temel okuma komutu
const ATA_CMD_WRITE_SECTORS: u8 = 0x30; // Sector yaz - yazma isleri icin
const ATA_CMD_FLUSH_CACHE: u8 = 0xE7; // Cache flush - yazma sonrasi mutlaka cagir
const ATA_CMD_IDENTIFY: u8 = 0xEC; // Drive kimligi - disk var mi yok mu anlamak icin

pub const SECTOR_SIZE: usize = 512;

// ── Drive IDs ──────────────────────────────────────────────────────────────────
// Iki disk var: Alpha ve Beta - ide0 ve ide1 master drive'lari
// Alpha = QEMU ide0 master (register 0xA0) - birincil disk, genelde boot disk
// Beta = QEMU ide1 master (register 0xB0) - ikincil disk, data icin
pub const DRIVE_ALPHA: u8 = 0; // Alpha - ilk disk, boot icin kullanilan genelde
pub const DRIVE_BETA: u8 = 1; // Beta - ikinci disk, extra alan istersen

/// Get LBA offset for a given drive ID (always 0 — separate IDE disks, no offset)
// LBA offset - simdilik 0, cunku ayri IDE bus'larda iki disk var
// ileride partition table gelirse burasi degisebilr
#[inline]
const fn get_lba_offset(drive_id: u8) -> u32 {
    match drive_id {
        DRIVE_ALPHA => 0, // Alpha'da offset yok, direkt LBA kullan
        DRIVE_BETA => 0,  // Beta'da da yok, her disk kendi LBA uzayinda
        _ => 0,           // bilinmeyen drive icin 0 don, sakin crash yeme
    }
}

// ── I/O helpers ───────────────────────────────────────────────────────────────
// Port okuma/yazma fonksiyonlar - inline cunku cok kucuk ve hizli olmali
// Bunlar bare metal'de calistigimiz icin gerekli - std yok yoksa
// in = input (oku), out = output (yaz)
#[inline]
unsafe fn inb(port: u16) -> u8 {
    // Port'tan 1 byte oku
    let val: u8;
    core::arch::asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack));
    val
}

#[inline]
unsafe fn outb(port: u16, val: u8) {
    // Port'a 1 byte yaz
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
}

#[inline]
unsafe fn inw(port: u16) -> u16 {
    // Port'tan 2 byte (1 word) oku - sector verisi icin lazim
    let val: u16;
    core::arch::asm!("in ax, dx", out("ax") val, in("dx") port, options(nomem, nostack));
    val
}

#[inline]
unsafe fn outw(port: u16, val: u16) {
    // Port'a 2 byte yaz - sector yazarken kullaniriz
    core::arch::asm!("out dx, ax", in("dx") port, in("ax") val, options(nomem, nostack));
}

// ── Wait for BSY to clear ─────────────────────────────────────────────────────
// BSY (busy) biti 0 olana kadar bekle - disk meşgulse beklememiz lazim
// Timeout var yoksa sonsuza kadar takilabilir - embedded hayat bu
unsafe fn wait_not_busy() -> bool {
    let mut timeout = 0u32;
    loop {
        let status = inb(ATA_PRIMARY_STATUS);
        if status & ATA_STATUS_BSY == 0 {
            // BSY 0 oldu mu? disk hazir
            return true;
        }
        timeout += 1;
        if timeout > 100_000_000 {
            // timeout oldu, disk yanit vermiyor
            return false;
        }
        spin_loop(); // CPU'yu uyut, enerji tasarrufu
    }
}

// ── Wait for DRQ (data ready) ─────────────────────────────────────────────────
// DRQ = Data Request, disk veri aktarimi icin hazir oldugunu soyluyor
// Ayni zamanda error ve drive fault kontrolu de yap
unsafe fn wait_drq() -> bool {
    let mut timeout = 0u32;
    loop {
        let status = inb(ATA_PRIMARY_STATUS);
        if status & ATA_STATUS_ERR != 0 {
            // Hata var mi bak
            return false; // Error varsa direkt cik
        }
        if status & ATA_STATUS_DF != 0 {
            // Drive fault mu?
            return false; // O da cikis
        }
        if status & ATA_STATUS_DRQ != 0 {
            // DRQ 1 ise veri aktarabiliriz
            return true;
        }
        timeout += 1;
        if timeout > 100_000_000 {
            // Timeout oldu yine
            return false;
        }
        spin_loop();
    }
}

// ── 400ns delay (read alt status 4 times) ─────────────────────────────────────
// 400 nanosaniye bekleme - ATA spesifikasyonu boyle istiyor
// Port okumasiyla zamanlama yapioruz, basit ama etkili
unsafe fn delay400ns() {
    inb(ATA_PRIMARY_CTRL); // 4 kere oku = ~400ns
    inb(ATA_PRIMARY_CTRL);
    inb(ATA_PRIMARY_CTRL);
    inb(ATA_PRIMARY_CTRL);
}

// ── Detect if ATA drive present ───────────────────────────────────────────────
// Drive tespiti - IDENTIFY komutu gonderip disk var mi kontrol et
// Bu biraz uzun bir surec ama mecburen yapmamiz lazim
pub static DISK_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe fn detect_drive(drive_sel: u8) -> bool {
    outb(ATA_PRIMARY_DRIVE, drive_sel); // Drive secimi yap (0xA0 veya 0xB0)
    delay400ns();

    outb(ATA_PRIMARY_CTRL, 0x04); // SRST biti - software reset
    delay400ns();
    outb(ATA_PRIMARY_CTRL, 0x00); // Reseti kaldir
    delay400ns();

    let mut retries = 10;
    while retries > 0 {
        if wait_not_busy() {
            // Disk hazir mi bak
            break;
        }
        retries -= 1;
    }

    outb(ATA_PRIMARY_COUNT, 0); // Identify komutu icin 0 olmali
    outb(ATA_PRIMARY_LBA_LO, 0); // Tüm LBA registerlarini 0'la
    outb(ATA_PRIMARY_LBA_MID, 0);
    outb(ATA_PRIMARY_LBA_HI, 0);
    outb(ATA_PRIMARY_CMD, ATA_CMD_IDENTIFY); // IDENTIFY komutunu gonder

    delay400ns();
    let status = inb(ATA_PRIMARY_STATUS);

    if status == 0 {
        // Status 0 ise disk yok
        return false;
    }

    retries = 100; // BSY biti 0 olana kadar bekle
    while retries > 0 {
        let s = inb(ATA_PRIMARY_STATUS);
        if s & ATA_STATUS_BSY == 0 {
            break;
        }
        retries -= 1;
    }

    if status & ATA_STATUS_ERR != 0 {
        // Error olduysa disk yok
        return false;
    }

    // ATAPI mi yoksa ATA mi kontrol et - farkli device'lar farkli yanit verir
    let mid = inb(ATA_PRIMARY_LBA_MID);
    let hi = inb(ATA_PRIMARY_LBA_HI);
    if mid == 0x14 && hi == 0xEB {
        // ATAPI signature - CD-ROM vs
        return false;
    }

    retries = 100; // DRQ bekle - veri aktarimi zamani
    while retries > 0 {
        let s = inb(ATA_PRIMARY_STATUS);
        if s & ATA_STATUS_DRQ != 0 {
            break;
        }
        if s & ATA_STATUS_ERR != 0 {
            return false;
        }
        retries -= 1;
    }

    // 256 word oku - identify data'yi atlat, yoksa sonraki islem bozulur
    for _ in 0..256 {
        inw(ATA_PRIMARY_DATA);
    }

    true // Disk var, basarili!
}

// ── Main detect function ──────────────────────────────────────────────────────
// Iki channel'daki diskleri tara - Alpha (ide0 master, 0xA0) ve Beta (ide1 master, 0xB0)
pub unsafe fn ata_detect() -> bool {
    DISK_COUNT.store(0, Ordering::Relaxed); // Taze basla, temiz tut

    if detect_drive(0xA0) {
        // Alpha'yi dene - 0xA0 = ide0 master
        DISK_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    if detect_drive(0xB0) {
        // Beta'yi dene - 0xB0 = ide1 master
        DISK_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    DISK_COUNT.load(Ordering::Relaxed) > 0 // En az bir disk varsa true don
}

// ── Get disk count ────────────────────────────────────────────────────────────
// Kac disk var bilgisini al - filesystem falan icin lazim olacak
pub fn get_disk_count() -> usize {
    DISK_COUNT.load(Ordering::Relaxed)
}

// ── Read one sector (512 bytes) from LBA address ──────────────────────────────
// Sektor okuma fonksiyonu - 28-bit LBA adresleme kullaniyoruz (max 128GB falan)
// Bu OS'nin en temel disk okuma fonksiyonu, hersey buna bagli
pub unsafe fn read_sector(drive_id: u8, lba: u32, buf: &mut [u8; SECTOR_SIZE]) -> bool {
    let offset_lba = lba + get_lba_offset(drive_id); // Drive'e gore LBA offset ekle
                                                     // Drive select + LBA high nibble (bit 24-27) - 0xE0 LBA mode bit'i set
    outb(ATA_PRIMARY_DRIVE, 0xE0 | ((offset_lba >> 24) as u8 & 0x0F));
    delay400ns();

    if !wait_not_busy() {
        // Disk hazir mi once bak
        return false;
    }

    // Register'lara parametreleri yaz
    outb(ATA_PRIMARY_ERR, 0); // Error register'i temizle
    outb(ATA_PRIMARY_COUNT, 1); // 1 sector oku diyoruz
    outb(ATA_PRIMARY_LBA_LO, (offset_lba & 0xFF) as u8); // LBA 0-7
    outb(ATA_PRIMARY_LBA_MID, ((offset_lba >> 8) & 0xFF) as u8); // LBA 8-15
    outb(ATA_PRIMARY_LBA_HI, ((offset_lba >> 16) & 0xFF) as u8); // LBA 16-23
    outb(ATA_PRIMARY_CMD, ATA_CMD_READ_SECTORS); // OKU KOMUTU!

    delay400ns();

    if !wait_drq() {
        // Disk veri gondermeye hazir mi?
        return false;
    }

    // 256 word (512 byte) oku - PIO mode boyle calisiyor, elle okumamiz lazim
    let ptr = buf.as_mut_ptr() as *mut u16;
    for i in 0..256 {
        let word = inw(ATA_PRIMARY_DATA);
        ptr.add(i).write_volatile(word); // volatile cunku DMA degil, elle yaziyoruz
    }

    true // Okuma basarili!
}

// ── Write one sector (512 bytes) to LBA address ───────────────────────────────
// Sektor yazma fonksiyonu - okumaya benziyor ama yazma ve cache flush var
// Yazma sonrasi cache flush sart, yoksa veri kaybedebiliriz
pub unsafe fn write_sector(drive_id: u8, lba: u32, buf: &[u8; SECTOR_SIZE]) -> bool {
    let offset_lba = lba + get_lba_offset(drive_id);
    // Drive select + LBA high nibble - okuma ile ayni
    outb(ATA_PRIMARY_DRIVE, 0xE0 | ((offset_lba >> 24) as u8 & 0x0F));
    delay400ns();

    if !wait_not_busy() {
        return false;
    }

    // Parametreleri yaz - okuma ile ayni
    outb(ATA_PRIMARY_ERR, 0);
    outb(ATA_PRIMARY_COUNT, 1);
    outb(ATA_PRIMARY_LBA_LO, (offset_lba & 0xFF) as u8);
    outb(ATA_PRIMARY_LBA_MID, ((offset_lba >> 8) & 0xFF) as u8);
    outb(ATA_PRIMARY_LBA_HI, ((offset_lba >> 16) & 0xFF) as u8);
    outb(ATA_PRIMARY_CMD, ATA_CMD_WRITE_SECTORS); // YAZ KOMUTU!

    delay400ns();

    if !wait_drq() {
        // Disk veri almaya hazir mi?
        return false;
    }

    // 256 word (512 byte) yaz - veriyi diske gonder
    let ptr = buf.as_ptr() as *const u16;
    for i in 0..256 {
        outw(ATA_PRIMARY_DATA, ptr.add(i).read_volatile());
    }

    // Cache flush - YAZMA SONRASI MUTLAKA CIAGIR
    // Bu olmadan veri sadece cache'de kalir, elektrik kesilse gider!
    outb(ATA_PRIMARY_CMD, ATA_CMD_FLUSH_CACHE);
    if !wait_not_busy() {
        return false;
    }

    true // Yazma basarili!
}
