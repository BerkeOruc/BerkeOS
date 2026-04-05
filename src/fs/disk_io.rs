// BerkeOS — disk_io.rs
// Disk image file I/O for mkdrive command
// Framework for creating BerkeFS disk images
// Actual file I/O requires host-side tooling or VirtIO

use crate::drivers::ata::SECTOR_SIZE;
use crate::fs::berkefs::MAX_DATA_BLOCKS;

const SUPERBLOCK_LBA: u32 = 0;
const DATA_START_LBA: u32 = 3;

const MIN_DATA_BLOCKS: usize = 32;
const BERKEFS_MAGIC: u32 = 0xBE4BEF55;
const BERKEFS_VERSION: u16 = 3;

const MAX_DISK_SECTORS: usize = (DATA_START_LBA as usize) + MAX_DATA_BLOCKS;
const MAX_DISK_SIZE: usize = MAX_DISK_SECTORS * SECTOR_SIZE;

const MIN_DISK_SECTORS: usize = (DATA_START_LBA as usize) + MIN_DATA_BLOCKS;
const MIN_DISK_SIZE: usize = MIN_DISK_SECTORS * SECTOR_SIZE;

pub struct DiskImage {
    data: [u8; MAX_DISK_SIZE],
    size: usize,
}

impl DiskImage {
    pub fn new(size: u64) -> Self {
        let size_bytes = size as usize;
        let required_sectors = (size_bytes + SECTOR_SIZE - 1) / SECTOR_SIZE;
        let data_blocks = required_sectors
            .saturating_sub(DATA_START_LBA as usize)
            .max(MIN_DATA_BLOCKS)
            .min(MAX_DATA_BLOCKS);
        let total_sectors = (DATA_START_LBA as usize) + data_blocks;
        let disk_size = total_sectors * SECTOR_SIZE;

        DiskImage {
            data: [0u8; MAX_DISK_SIZE],
            size: disk_size,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.size]
    }

    pub fn write_superblock(&mut self, label: &[u8], data_blocks: usize) {
        if self.size < SECTOR_SIZE {
            return;
        }
        self.data[0] = (BERKEFS_MAGIC & 0xFF) as u8;
        self.data[1] = ((BERKEFS_MAGIC >> 8) & 0xFF) as u8;
        self.data[2] = ((BERKEFS_MAGIC >> 16) & 0xFF) as u8;
        self.data[3] = ((BERKEFS_MAGIC >> 24) & 0xFF) as u8;

        self.data[4] = (BERKEFS_VERSION & 0xFF) as u8;
        self.data[5] = (BERKEFS_VERSION >> 8) as u8;

        let blocks_lo = (data_blocks & 0xFF) as u8;
        let blocks_hi = ((data_blocks >> 8) & 0xFF) as u8;
        self.data[6] = blocks_lo;
        self.data[7] = blocks_hi;
        self.data[8] = blocks_lo;
        self.data[9] = blocks_hi;

        let label_len = label.len().min(16);
        self.data[12..12 + label_len].copy_from_slice(&label[..label_len]);
    }
}

pub fn build_disk_image(label: &[u8], size: u64) -> DiskImage {
    let mut img = DiskImage::new(size);
    let data_blocks = (img.size / SECTOR_SIZE).saturating_sub(DATA_START_LBA as usize);
    img.write_superblock(label, data_blocks);
    img
}

#[cfg(feature = "disk_io")]
pub fn create_disk_image(path: &[u8], label: &[u8], size: u64) -> bool {
    let img = build_disk_image(label, size);

    let mut path_str = [0u8; 128];
    let plen = path.len().min(127);
    path_str[..plen].copy_from_slice(&path[..plen]);
    let path_str = core::str::from_utf8(&path_str[..plen]).unwrap_or("");

    let parent = get_parent_dir(path_str);
    if !parent.is_empty() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(path_str, img.as_slice()) {
        Ok(_) => true,
        Err(_) => false,
    }
}

fn get_parent_dir(path: &str) -> &str {
    for (i, c) in path.char_indices().rev() {
        if c == '/' {
            return &path[..i];
        }
    }
    ""
}

#[cfg(not(feature = "disk_io"))]
pub fn create_disk_image(_path: &[u8], _label: &[u8], _size: u64) -> bool {
    false
}

pub fn is_available() -> bool {
    #[cfg(feature = "disk_io")]
    return true;
    #[cfg(not(feature = "disk_io"))]
    return false;
}
