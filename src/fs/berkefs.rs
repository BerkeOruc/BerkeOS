// BerkeOS — berkefs.rs
// BerkeFS — Custom Filesystem v3 (Improved)
// Layout (per disk):
//   Sector 0      : Superblock - butce bilgileri tutar, diskte ilk okunan sey (first thing diskten okunur)
//   Sectors 1-2   : Inode table (32 × 32 bytes) - dosya bilgileri burda saklanir
//   Sectors 3-130 : Data blocks (128 × 512 bytes = 64 KB) - gercek dosya datasi burda
// Drive mapping:
//   Drive 0 (Alpha) = QEMU ide0, Drive 1 (Beta) = QEMU ide1
//   Alpha = birinci disk (primary), Beta = ikinci disk (secondary)

use crate::drivers::ata::{read_sector, write_sector, SECTOR_SIZE};

pub const BERKEFS_MAGIC: u32 = 0xBE4BEF55; // sihirli numara - filesystem tanimak icin kullanilir (magic bytes)
pub const BERKEFS_VERSION: u16 = 3; // versiyon 3 - oncekilere göre iyilestirmeler var
pub const BLOCK_SIZE: usize = 512; // her block 512 byte - sector boyutu ile ayni
pub const SUPERBLOCK_LBA: u32 = 0; // superblock sektor 0'da baslar
pub const INODE_TABLE_LBA: u32 = 1; // inode tablo sector 1'den baslar
pub const INODE_TABLE_SECTORS: u32 = 2; // inode tablo 2 sektor kaplar
pub const DATA_START_LBA: u32 = 3; // data blocks sector 3'ten baslar
pub const MAX_INODES: usize = 128; // max 128 dosya/dizin olabilir
pub const MAX_DATA_BLOCKS: usize = 256; // max 256 data block
pub const MAX_NAME: usize = 60; // max dosya ismi uzunlugu
pub const INODE_SIZE: usize = 32; // her inode 32 byte

pub const FTYPE_FREE: u8 = 0; // bos inode - henuz kullanilmiyor
pub const FTYPE_FILE: u8 = 1; // regular dosya
pub const FTYPE_DIR: u8 = 2; // dizin - klasör

pub const INODE_FLAG_READONLY: u16 = 0x01; // sadece okunabilir - yazilamaz
pub const INODE_FLAG_HIDDEN: u16 = 0x02; // gizli dosya - normalde gözükmez
pub const INODE_FLAG_SYSTEM: u16 = 0x04; // sistem dosyasi - dikkatli ol

/// Inode - dosya/dizin bilgilerini saklayan yapi (inode = index node)
/// Her dosyanin bir inode'u var - dosya metadata burda tutulur
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Inode {
    pub ftype: u8,      // dosya tipi - bos mu, dosya mi, dizin mi?
    pub blocks: u8,     // kac block kullaniyor - dosya buyuklugu block sayisi
    pub size: u16,      // dosya boyutu byte olarak
    pub block: u16,     // ilk block numarasi - datanin nerede oldugunu gösterir
    pub flags: u16,     // özellikler - readonly, hidden, system
    pub name: [u8; 20], // kisaltilmis isim - ilk 20 karakter
    pub created: u32,   // olusturma zamani - timestamp
}

impl Inode {
    /// Bos (sifir) inode olustur - yeni dosya icin hazirlik
    pub const fn empty() -> Self {
        Inode {
            ftype: FTYPE_FREE,
            blocks: 0,
            size: 0,
            block: 0,
            flags: 0,
            name: [0u8; 20],
            created: 0,
        }
    }

    /// Inode'dan ismi al - null terminatore kadar oku
    pub fn get_name(&self) -> &[u8] {
        let mut len = 0;
        while len < 20 && self.name[len] != 0 {
            len += 1;
        }
        &self.name[..len]
    }

    /// Inode'a kisa isim ayarla - max 19 karakter
    pub fn set_name_short(&mut self, name: &[u8]) {
        self.name = [0u8; 20];
        let n = name.len().min(19);
        self.name[..n].copy_from_slice(&name[..n]);
    }
}

/// Superblock - disk hakkinda ana bilgiler (filesystem header)
/// Disk mount edildiginde ilk okunan sey - butce bilgiler burda
#[repr(C)]
pub struct Superblock {
    pub magic: u32,                // sihirli numara - BERKEFS_MAGIC olmali yoksa disk bos
    pub version: u16,              // filesystem versiyonu
    pub total_blocks: u16,         // toplam block sayisi
    pub free_blocks: u16,          // bos block sayisi - kac block müsait
    pub inode_count: u16,          // kullanilan inode sayisi
    pub label: [u8; 16],           // disk etiketi - kullanici tarafindan ayarlanabilir
    pub flags: u32,                // filesystem özellikleri
    pub checksum: u32,             // veri dogrulama icin - corruption kontrolü
    pub ext_names: [[u8; 30]; 32], // uzun dosya isimleri - ilk 32 inode icin
}

/// FSCK result - filesystem consistency check report
/// Uses fixed-size arrays instead of Vec (no_alloc requirement)
pub struct FsckResult {
    pub errors: [u8; 32],   // error messages (null-terminated, 32 bytes each)
    pub error_count: u8,    // number of errors found
    pub warnings: [u8; 32], // warning messages (null-terminated, 32 bytes each)
    pub warning_count: u8,  // number of warnings found
    pub is_clean: bool,     // true if no errors found
    pub summary: [u8; 64],  // human-readable summary string
}

impl FsckResult {
    /// Yeni bos sonuc olustur
    pub const fn clean() -> Self {
        FsckResult {
            errors: [0u8; 32],
            error_count: 0,
            warnings: [0u8; 32],
            warning_count: 0,
            is_clean: true,
            summary: [0u8; 64],
        }
    }

    /// Hatayi ekle
    fn add_error(&mut self, msg: &[u8]) {
        if self.error_count < 32 {
            let off = self.error_count as usize * 32;
            let len = msg.len().min(31);
            self.errors[off..off + len].copy_from_slice(&msg[..len]);
            self.error_count += 1;
        }
    }

    /// Uyari ekle
    fn add_warning(&mut self, msg: &[u8]) {
        if self.warning_count < 32 {
            let off = self.warning_count as usize * 32;
            let len = msg.len().min(31);
            self.warnings[off..off + len].copy_from_slice(&msg[..len]);
            self.warning_count += 1;
        }
    }

    /// Ozet dizisi ayarla
    fn set_summary(&mut self, msg: &[u8]) {
        let len = msg.len().min(63);
        self.summary[..len].copy_from_slice(&msg[..len]);
    }
}

/// BerkeFS - ana filesystem yapisi
/// Tum dosya sistemi islemleri bu struct uzerinden yapilir
pub struct BerkeFS {
    pub drive_id: u8,                        // hangi disk - Alpha(0) veya Beta(1)
    pub mounted: bool,                       // disk mount edilmis mi?
    pub version: u16,                        // filesystem versiyonu
    pub inodes: [Inode; MAX_INODES],         // tum inode'lar - bellekte cache
    pub block_used: [bool; MAX_DATA_BLOCKS], // block kullanim durumu
    pub ext_names: [[u8; 30]; MAX_INODES],   // uzun dosya isimleri
    pub flags: u32,                          // filesystem özellikleri
    pub checksum: u32,                       // veri dogrulama
}

impl BerkeFS {
    /// Yeni BerkeFS olustur - disk belirtilmeli
    pub const fn new(drive_id: u8) -> Self {
        BerkeFS {
            drive_id,
            mounted: false,
            version: BERKEFS_VERSION,
            inodes: [Inode::empty(); MAX_INODES],
            block_used: [false; MAX_DATA_BLOCKS],
            ext_names: [[0u8; 30]; MAX_INODES],
            flags: 0,
            checksum: 0,
        }
    }

    /// Beta RAM disk'i mount edilmis olarak isaretle (I/O yok, FS RAM'de)
    pub fn set_mounted(&mut self) {
        self.mounted = true;
        self.version = BERKEFS_VERSION;
    }

    /// Tam dosya ismini al - 20+30 = max 50 karakter desteklenir
    pub fn get_full_name<'a>(&self, i: usize, buf: &'a mut [u8; 64]) -> &'a [u8] {
        let mut len = 0;
        for &b in self.inodes[i].get_name() {
            if len < 63 {
                buf[len] = b;
                len += 1;
            }
        }
        for j in 0..30 {
            if self.ext_names[i][j] == 0 {
                break;
            }
            if len < 63 {
                buf[len] = self.ext_names[i][j];
                len += 1;
            }
        }
        &buf[..len]
    }

    /// Tam dosya ismini ayarla - 20 char + 30 char ext name
    fn set_full_name(&mut self, i: usize, name: &[u8]) {
        self.inodes[i].name = [0u8; 20];
        self.ext_names[i] = [0u8; 30];
        let short_len = name.len().min(19);
        self.inodes[i].name[..short_len].copy_from_slice(&name[..short_len]);
        if name.len() > 19 {
            let ext = &name[19..];
            let ext_len = ext.len().min(29);
            self.ext_names[i][..ext_len].copy_from_slice(&ext[..ext_len]);
        }
    }

    /// Yol var mi kontrol et - dosya veya dizin mevcut mu? (path exists check)
    pub fn path_exists(&self, path: &[u8]) -> bool {
        if !self.mounted {
            return false;
        }
        for i in 0..MAX_INODES {
            if self.inodes[i].ftype == FTYPE_FREE {
                continue;
            }
            let mut buf = [0u8; 64];
            let name = self.get_full_name(i, &mut buf);
            if name == path {
                return true;
            }
        }
        false
    }

    /// Disk formatla - tamamen sil ve yeni filesystem olustur (low level format)
    /// Butun veriler silinir! Dikkatli ol!
    pub fn format(&mut self, label: &[u8]) -> bool {
        self.version = BERKEFS_VERSION;
        self.flags = 0;
        self.checksum = 0;

        let mut sector = [0u8; SECTOR_SIZE];
        let mb = BERKEFS_MAGIC.to_le_bytes();
        sector[0] = mb[0];
        sector[1] = mb[1];
        sector[2] = mb[2];
        sector[3] = mb[3];
        sector[4] = (BERKEFS_VERSION & 0xFF) as u8;
        sector[5] = (BERKEFS_VERSION >> 8) as u8;
        sector[6] = (MAX_DATA_BLOCKS & 0xFF) as u8;
        sector[7] = (MAX_DATA_BLOCKS >> 8) as u8;
        sector[8] = (MAX_DATA_BLOCKS & 0xFF) as u8;
        sector[9] = (MAX_DATA_BLOCKS >> 8) as u8;
        sector[10] = 0;
        sector[11] = 0;
        let n = label.len().min(16);
        sector[12..12 + n].copy_from_slice(&label[..n]);

        if !unsafe { write_sector(self.drive_id, SUPERBLOCK_LBA, &sector) } {
            return false;
        }

        let zero = [0u8; SECTOR_SIZE];
        for i in 0..INODE_TABLE_SECTORS {
            if !unsafe { write_sector(self.drive_id, INODE_TABLE_LBA + i, &zero) } {
                return false;
            }
        }

        for i in 0..MAX_DATA_BLOCKS as u32 {
            if !unsafe { write_sector(self.drive_id, DATA_START_LBA + i, &zero) } {
                return false;
            }
        }

        self.mounted = true;
        self.inodes = [Inode::empty(); MAX_INODES];
        self.block_used = [false; MAX_DATA_BLOCKS];
        self.ext_names = [[0u8; 30]; MAX_INODES];

        self.save_all()
    }

    /// Diski mount et - filesystem'u ac ve bellege yukle (open filesystem)
    /// Oncelikle superblock okunur, sonra inode'lar yuklenir
    pub fn mount(&mut self) -> bool {
        let mut sector = [0u8; SECTOR_SIZE];
        if !unsafe { read_sector(self.drive_id, SUPERBLOCK_LBA, &mut sector) } {
            return false;
        }

        let magic = u32::from_le_bytes([sector[0], sector[1], sector[2], sector[3]]);
        if magic != BERKEFS_MAGIC {
            return false;
        }

        self.version = u16::from_le_bytes([sector[4], sector[5]]);
        self.flags = u32::from_le_bytes([sector[36], sector[37], sector[38], sector[39]]);
        self.checksum = u32::from_le_bytes([sector[40], sector[41], sector[42], sector[43]]);

        let ext_offset = 44usize;
        for i in 0..MAX_INODES {
            let off = ext_offset + i * 30;
            if off + 30 <= SECTOR_SIZE {
                self.ext_names[i].copy_from_slice(&sector[off..off + 30]);
            }
        }

        for sec in 0..INODE_TABLE_SECTORS {
            let mut isect = [0u8; SECTOR_SIZE];
            if !unsafe { read_sector(self.drive_id, INODE_TABLE_LBA + sec, &mut isect) } {
                return false;
            }

            let inode_start = ((sec as usize) * SECTOR_SIZE / INODE_SIZE) as usize;
            let inode_end =
                (((sec + 1) as usize) * SECTOR_SIZE / INODE_SIZE).min(MAX_INODES) as usize;

            for i in inode_start..inode_end {
                let off = (i - inode_start) * INODE_SIZE;
                self.inodes[i].ftype = isect[off];
                self.inodes[i].blocks = isect[off + 1];
                self.inodes[i].size = u16::from_le_bytes([isect[off + 2], isect[off + 3]]);
                self.inodes[i].block = u16::from_le_bytes([isect[off + 4], isect[off + 5]]);
                self.inodes[i].flags = u16::from_le_bytes([isect[off + 6], isect[off + 7]]);
                self.inodes[i].name = [0u8; 20];
                self.inodes[i]
                    .name
                    .copy_from_slice(&isect[off + 8..off + 28]);
                self.inodes[i].created = u32::from_le_bytes([
                    isect[off + 28],
                    isect[off + 29],
                    isect[off + 30],
                    isect[off + 31],
                ]);
            }
        }

        self.block_used = [false; MAX_DATA_BLOCKS];
        for i in 0..MAX_INODES {
            if self.inodes[i].ftype != FTYPE_FREE {
                let start = self.inodes[i].block as usize;
                let count = self.inodes[i].blocks as usize;
                for b in start..start + count {
                    if b < MAX_DATA_BLOCKS {
                        self.block_used[b] = true;
                    }
                }
            }
        }

        self.mounted = true;
        true
    }

    /// Her seyi kaydet - superblock ve inode tablo (flush to disk)
    fn save_all(&self) -> bool {
        self.save_superblock() && self.save_inodes()
    }

    /// Superblock'u diske yaz - butce bilgilerini kaydet
    fn save_superblock(&self) -> bool {
        let mut sector = [0u8; SECTOR_SIZE];
        let mb = BERKEFS_MAGIC.to_le_bytes();
        sector[0] = mb[0];
        sector[1] = mb[1];
        sector[2] = mb[2];
        sector[3] = mb[3];
        sector[4] = (self.version & 0xFF) as u8;
        sector[5] = (self.version >> 8) as u8;
        sector[6] = (MAX_DATA_BLOCKS & 0xFF) as u8;
        sector[7] = (MAX_DATA_BLOCKS >> 8) as u8;
        let free = self.free_blocks() as u8;
        sector[8] = free;
        sector[9] = 0;
        let used = self.used_inodes() as u8;
        sector[10] = used;
        sector[11] = 0;

        let flags_bytes = self.flags.to_le_bytes();
        sector[36] = flags_bytes[0];
        sector[37] = flags_bytes[1];
        sector[38] = flags_bytes[2];
        sector[39] = flags_bytes[3];

        let chk_bytes = self.checksum.to_le_bytes();
        sector[40] = chk_bytes[0];
        sector[41] = chk_bytes[1];
        sector[42] = chk_bytes[2];
        sector[43] = chk_bytes[3];

        let ext_offset = 44usize;
        for i in 0..MAX_INODES {
            let off = ext_offset + i * 30;
            if off + 30 <= SECTOR_SIZE {
                sector[off..off + 30].copy_from_slice(&self.ext_names[i]);
            }
        }
        unsafe { write_sector(self.drive_id, SUPERBLOCK_LBA, &sector) }
    }

    /// Inode tabloyu diske yaz - tum inode'lari sektorlere kaydet
    pub fn save_inodes(&self) -> bool {
        for sec in 0..INODE_TABLE_SECTORS {
            let mut sector = [0u8; SECTOR_SIZE];
            let inode_start = ((sec as usize) * SECTOR_SIZE / INODE_SIZE) as usize;
            let inode_end =
                (((sec + 1) as usize) * SECTOR_SIZE / INODE_SIZE).min(MAX_INODES) as usize;

            for i in inode_start..inode_end {
                let off = (i - inode_start) * INODE_SIZE;
                sector[off] = self.inodes[i].ftype;
                sector[off + 1] = self.inodes[i].blocks;
                let size_b = self.inodes[i].size.to_le_bytes();
                sector[off + 2] = size_b[0];
                sector[off + 3] = size_b[1];
                let block_b = self.inodes[i].block.to_le_bytes();
                sector[off + 4] = block_b[0];
                sector[off + 5] = block_b[1];
                let flags_b = self.inodes[i].flags.to_le_bytes();
                sector[off + 6] = flags_b[0];
                sector[off + 7] = flags_b[1];
                sector[off + 8..off + 28].copy_from_slice(&self.inodes[i].name);
                let ctime_b = self.inodes[i].created.to_le_bytes();
                sector[off + 28] = ctime_b[0];
                sector[off + 29] = ctime_b[1];
                sector[off + 30] = ctime_b[2];
                sector[off + 31] = ctime_b[3];
            }
            if !unsafe { write_sector(self.drive_id, INODE_TABLE_LBA + sec, &sector) } {
                return false;
            }
        }
        true
    }

    /// Bos inode bul ve ayir - yeni dosya icin yer bul
    fn alloc_inode(&mut self) -> Option<usize> {
        for i in 0..MAX_INODES {
            if self.inodes[i].ftype == FTYPE_FREE {
                return Some(i);
            }
        }
        None
    }

    /// Belli sayida art arda bos block bul ve ayir - contiguous allocation
    fn alloc_blocks(&mut self, count: usize) -> Option<usize> {
        if count == 0 {
            return Some(0);
        }
        let mut run_start = 0;
        let mut run_len = 0;
        for i in 0..MAX_DATA_BLOCKS {
            if !self.block_used[i] {
                if run_len == 0 {
                    run_start = i;
                }
                run_len += 1;
                if run_len >= count {
                    return Some(run_start);
                }
            } else {
                run_len = 0;
            }
        }
        None
    }

    /// Dizin olustur - yeni klasor yarat (make directory)
    pub fn create_dir(&mut self, path: &[u8]) -> bool {
        if !self.mounted {
            return false;
        }
        if self.path_exists(path) {
            return false;
        }
        let idx = match self.alloc_inode() {
            Some(i) => i,
            None => return false,
        };
        self.inodes[idx].ftype = FTYPE_DIR;
        self.inodes[idx].blocks = 0;
        self.inodes[idx].size = 0;
        self.inodes[idx].block = 0;
        self.inodes[idx].flags = 0;
        self.inodes[idx].created = 0;
        self.set_full_name(idx, path);
        self.save_all()
    }

    /// Dosya olustur - yeni dosya yarat ve datayi yaz (create file)
    /// path = dosya yolu, data = dosya icerigi
    pub fn create_file(&mut self, path: &[u8], data: &[u8]) -> bool {
        if !self.mounted {
            return false;
        }
        if self.path_exists(path) {
            return false;
        }
        let idx = match self.alloc_inode() {
            Some(i) => i,
            None => return false,
        };

        let blocks_needed = if data.is_empty() {
            1
        } else {
            (data.len() + SECTOR_SIZE - 1) / SECTOR_SIZE
        };
        let start_block = match self.alloc_blocks(blocks_needed) {
            Some(b) => b,
            None => return false,
        };

        for b in 0..blocks_needed {
            let mut sector = [0u8; SECTOR_SIZE];
            let cs = b * SECTOR_SIZE;
            let ce = (cs + SECTOR_SIZE).min(data.len());
            if cs < data.len() {
                sector[..ce - cs].copy_from_slice(&data[cs..ce]);
            }
            let lba = DATA_START_LBA + (start_block + b) as u32;
            if !unsafe { write_sector(self.drive_id, lba, &sector) } {
                return false;
            }
            self.block_used[start_block + b] = true;
        }

        self.inodes[idx].ftype = FTYPE_FILE;
        self.inodes[idx].size = data.len() as u16;
        self.inodes[idx].block = start_block as u16;
        self.inodes[idx].blocks = blocks_needed as u8;
        self.inodes[idx].flags = 0;
        self.inodes[idx].created = 0;
        self.set_full_name(idx, path);
        self.save_all()
    }

    /// Dosya sil - dosyayi inode ile birlikte sil (remove file)
    /// Block'lari da serbest birak
    pub fn delete_file(&mut self, path: &[u8]) -> bool {
        if !self.mounted {
            return false;
        }
        for i in 0..MAX_INODES {
            if self.inodes[i].ftype == FTYPE_FREE {
                continue;
            }
            let mut buf = [0u8; 64];
            let name = {
                let mut len = 0;
                for &b in self.inodes[i].get_name() {
                    if len < 63 {
                        buf[len] = b;
                        len += 1;
                    }
                }
                for j in 0..30 {
                    if self.ext_names[i][j] == 0 {
                        break;
                    }
                    if len < 63 {
                        buf[len] = self.ext_names[i][j];
                        len += 1;
                    }
                }
                &buf[..len]
            };
            if name == path {
                let start = self.inodes[i].block as usize;
                let count = self.inodes[i].blocks as usize;
                for b in start..start + count {
                    if b < MAX_DATA_BLOCKS {
                        self.block_used[b] = false;
                    }
                }
                self.inodes[i] = Inode::empty();
                self.ext_names[i] = [0u8; 30];
                return self.save_all();
            }
        }
        false
    }

    /// Dosya/dizin yeniden adlandir (rename file/directory)
    pub fn rename_entry(&mut self, old_path: &[u8], new_path: &[u8]) -> bool {
        if !self.mounted {
            return false;
        }
        for i in 0..MAX_INODES {
            if self.inodes[i].ftype == FTYPE_FREE {
                continue;
            }
            let mut buf = [0u8; 64];
            let name_len = {
                let mut len = 0;
                for &b in self.inodes[i].get_name() {
                    if len < 63 {
                        buf[len] = b;
                        len += 1;
                    }
                }
                for j in 0..30 {
                    if self.ext_names[i][j] == 0 {
                        break;
                    }
                    if len < 63 {
                        buf[len] = self.ext_names[i][j];
                        len += 1;
                    }
                }
                len
            };
            if &buf[..name_len] == old_path {
                self.set_full_name(i, new_path);
                return self.save_all();
            }
        }
        false
    }

    /// Dosya oku - dosya icerigini bellege oku (read file contents)
    /// path = dosya yolu, out = okunan veri icin tampon
    pub fn read_file(&self, path: &[u8], out: &mut [u8]) -> Option<usize> {
        if !self.mounted {
            return None;
        }
        for i in 0..MAX_INODES {
            if self.inodes[i].ftype != FTYPE_FILE {
                continue;
            }
            let mut buf = [0u8; 64];
            let name_len = {
                let mut len = 0;
                for &b in self.inodes[i].get_name() {
                    if len < 63 {
                        buf[len] = b;
                        len += 1;
                    }
                }
                for j in 0..30 {
                    if self.ext_names[i][j] == 0 {
                        break;
                    }
                    if len < 63 {
                        buf[len] = self.ext_names[i][j];
                        len += 1;
                    }
                }
                len
            };
            if &buf[..name_len] != path {
                continue;
            }

            let size = self.inodes[i].size as usize;
            let start = self.inodes[i].block as usize;
            let blocks = self.inodes[i].blocks as usize;
            let mut read = 0usize;

            for b in 0..blocks {
                let mut sector = [0u8; SECTOR_SIZE];
                let lba = DATA_START_LBA + (start + b) as u32;
                if !unsafe { read_sector(self.drive_id, lba, &mut sector) } {
                    return None;
                }
                let remaining = size - read;
                let chunk_len = remaining.min(SECTOR_SIZE).min(out.len() - read);
                out[read..read + chunk_len].copy_from_slice(&sector[..chunk_len]);
                read += chunk_len;
                if read >= size || read >= out.len() {
                    break;
                }
            }
            return Some(read);
        }
        None
    }

    /// Bos block sayisini dondur - kac block müsait?
    pub fn free_blocks(&self) -> usize {
        self.block_used.iter().filter(|&&b| !b).count()
    }

    /// Kullanilan inode sayisini dondur - kac dosya/dizin var?
    pub fn used_inodes(&self) -> usize {
        self.inodes.iter().filter(|i| i.ftype != FTYPE_FREE).count()
    }

    /// Filesystem consistency check - diskteki veriyi okur, yazmaz (read-only)
    /// Butun kontroleri tek tek gezer, sorunlari raporlar
    pub fn fsck_validate(&self, drive_id: u8) -> FsckResult {
        let mut result = FsckResult::clean();

        let mut sector = [0u8; SECTOR_SIZE];
        if !unsafe { read_sector(drive_id, SUPERBLOCK_LBA, &mut sector) } {
            result.is_clean = false;
            result.add_error(b"ERR: Cannot read superblock");
            result.set_summary(b"FSCK FAILED: Cannot read disk");
            return result;
        }

        let magic = u32::from_le_bytes([sector[0], sector[1], sector[2], sector[3]]);
        if magic != BERKEFS_MAGIC {
            result.is_clean = false;
            result.add_error(b"ERR: Bad magic number");
        }

        let version = u16::from_le_bytes([sector[4], sector[5]]);
        if version != BERKEFS_VERSION {
            result.is_clean = false;
            result.add_error(b"ERR: Version mismatch");
        }

        let flags = u32::from_le_bytes([sector[36], sector[37], sector[38], sector[39]]);
        let checksum = u32::from_le_bytes([sector[40], sector[41], sector[42], sector[43]]);

        if checksum != 0 {
            let mut calc: u32 = 0;
            for i in 0..SECTOR_SIZE {
                if i >= 40 && i < 44 {
                    continue;
                }
                calc = calc.wrapping_add(sector[i] as u32);
            }
            if calc != checksum {
                result.is_clean = false;
                result.add_error(b"ERR: Superblock checksum mismatch");
            }
        }

        for sec in 0..INODE_TABLE_SECTORS {
            let mut isect = [0u8; SECTOR_SIZE];
            if !unsafe { read_sector(drive_id, INODE_TABLE_LBA + sec, &mut isect) } {
                result.is_clean = false;
                result.add_error(b"ERR: Cannot read inode table");
                result.set_summary(b"FSCK FAILED: Cannot read inode table");
                return result;
            }

            let inode_start = ((sec as usize) * SECTOR_SIZE / INODE_SIZE) as usize;
            let inode_end =
                (((sec + 1) as usize) * SECTOR_SIZE / INODE_SIZE).min(MAX_INODES) as usize;

            for i in inode_start..inode_end {
                let off = (i - inode_start) * INODE_SIZE;
                let ftype = isect[off];
                let blocks = isect[off + 1];
                let block_lo = isect[off + 4];
                let block_hi = isect[off + 5];
                let block = u16::from_le_bytes([block_lo, block_hi]);
                let size = u16::from_le_bytes([isect[off + 2], isect[off + 3]]);

                if ftype == FTYPE_FREE {
                    continue;
                }

                if ftype != FTYPE_FILE && ftype != FTYPE_DIR {
                    result.add_warning(b"WRN: Unknown inode type");
                }

                if blocks > 0 {
                    if block as usize >= MAX_DATA_BLOCKS {
                        result.is_clean = false;
                        result.add_error(b"ERR: Inode block out of bounds");
                    } else if (block as usize) + (blocks as usize) > MAX_DATA_BLOCKS {
                        result.is_clean = false;
                        result.add_error(b"ERR: Inode block range overflow");
                    }
                }

                if size > 0 && blocks == 0 {
                    result.add_warning(b"WRN: Non-empty file with zero blocks");
                }
            }
        }

        let mut seen_blocks = [false; MAX_DATA_BLOCKS];
        let mut duplicate_count: u8 = 0;

        for i in 0..MAX_INODES {
            let inode = &self.inodes[i];
            if inode.ftype == FTYPE_FREE {
                continue;
            }

            if inode.blocks > 0 {
                let start = inode.block as usize;
                let count = inode.blocks as usize;

                if start < MAX_DATA_BLOCKS && count <= MAX_DATA_BLOCKS {
                    let end = (start + count).min(MAX_DATA_BLOCKS);
                    for b in start..end {
                        if seen_blocks[b] {
                            duplicate_count = duplicate_count.saturating_add(1);
                        } else {
                            seen_blocks[b] = true;
                        }
                    }
                }
            }
        }

        if duplicate_count > 0 {
            result.is_clean = false;
            result.add_error(b"ERR: Duplicate block allocations");
        }

        let actual_free: usize = seen_blocks.iter().filter(|&&b| !b).count();
        let reported_free = (sector[8] as usize) + ((sector[9] as usize) << 8);
        if sector[8] != 0 || sector[9] != 0 {
            if actual_free != reported_free {
                result.add_warning(b"WRN: Free block count mismatch");
            }
        }

        if result.is_clean {
            let cnt = self.used_inodes();
            let mut s = *b"FSCK CLEAN: X inodes OK   ";
            s[13] = (b'0' as u8).wrapping_add((cnt / 100) as u8 % 10);
            s[14] = (b'0' as u8).wrapping_add((cnt / 10) as u8 % 10);
            s[15] = (b'0' as u8).wrapping_add((cnt % 10) as u8);
            result.set_summary(&s);
        } else {
            result.set_summary(b"FSCK FOUND ERRORS - RUN RECOVERY");
        }

        let _ = flags;
        result
    }
}
