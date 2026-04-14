# BerkeOS Filesystem (BerkeFS)

## Overview

BerkeFS is a custom filesystem designed specifically for BerkeOS, optimized for embedded systems with limited storage. It supports up to 12 drives (Alpha through Mu), with each drive having a maximum capacity of approximately 128 GiB (limited by 28-bit LBA addressing).

## Architecture

### Disk Layout

Each BerkeFS drive has a fixed layout:

```
Sector 0       : Superblock (filesystem metadata)
Sectors 1-2     : Inode Table (128 inodes × 32 bytes)
Sectors 3-130   : Data Blocks (128 blocks × 512 bytes = 64 KiB)
```

### Key Parameters

- **Sector Size**: 512 bytes
- **Block Size**: 512 bytes (1:1 mapping with sectors)
- **Max Files/Dirs**: 128 per drive
- **Max File Size**: 64 KiB (128 blocks × 512 bytes)
- **Max Filename**: 50 characters (20 + 30 byte extension)
- **Magic Number**: `0xBE4BEF55` (BERKEFS)
- **Version**: 3 (current)

## Data Structures

### Inode Structure

Each file/directory is represented by an inode:

```rust
#[repr(C)]
pub struct Inode {
    pub ftype: u8,         // File type (0=free, 1=file, 2=directory)
    pub blocks: u8,        // Number of blocks allocated
    pub size: u16,         // File size in bytes
    pub block: u16,        // Starting block number
    pub flags: u16,        // File attributes (readonly, hidden, system)
    pub name: [u8; 20],    // Short filename (first 20 chars)
    pub created: u32,      // Creation timestamp
}
```

### Superblock Structure

The superblock contains filesystem metadata:

```rust
#[repr(C)]
pub struct Superblock {
    pub magic: u32,                    // BERKEFS_MAGIC
    pub version: u16,                  // Filesystem version
    pub total_blocks: u16,             // Total data blocks
    pub free_blocks: u16,              // Available blocks
    pub inode_count: u16,              // Used inodes
    pub label: [u8; 16],               // Volume label
    pub flags: u32,                    // Filesystem flags
    pub checksum: u32,                 // Data integrity check
    pub ext_names: [[u8; 30]; 32],     // Extended filenames
}
```

## File Operations

### File Creation

```rust
pub fn create_file(&mut self, path: &[u8], data: &[u8]) -> bool
```

- Allocates contiguous blocks for file data
- Creates inode with metadata
- Writes data to allocated blocks
- Updates inode table and superblock

### File Reading

```rust
pub fn read_file(&self, path: &[u8], out: &mut [u8]) -> Option<usize>
```

- Locates file by name in inode table
- Reads data from allocated blocks
- Returns bytes read or None if file not found

### File Deletion

```rust
pub fn delete_file(&mut self, path: &[u8]) -> bool
```

- Locates file inode
- Marks allocated blocks as free
- Clears inode entry
- Updates metadata

### Directory Operations

```rust
pub fn create_dir(&mut self, path: &[u8]) -> bool
```

- Creates directory inode (no data blocks allocated)
- Directories are metadata-only entries

## Allocation Strategy

### Block Allocation

- **Contiguous Allocation**: Files occupy consecutive blocks
- **First-Fit**: Allocates first suitable free block sequence
- **No Fragmentation**: Prevents scattered file storage

### Inode Allocation

- **Linear Search**: Finds first free inode slot
- **No Reuse Policy**: Deleted inodes become immediately available

## Filesystem Operations

### Mount Operation

```rust
pub fn mount(&mut self) -> bool
```

1. Reads superblock and validates magic number
2. Loads inode table into memory cache
3. Reconstructs block usage bitmap
4. Verifies filesystem integrity

### Format Operation

```rust
pub fn format(&mut self, label: &[u8]) -> bool
```

1. Initializes superblock with metadata
2. Zeroes inode table sectors
3. Zeroes all data block sectors
4. Creates initial filesystem state

### FSCK (Filesystem Check)

```rust
pub fn fsck_validate(&self, drive_id: u8) -> FsckResult
```

- Validates superblock magic and version
- Checks inode table consistency
- Verifies block allocation integrity
- Reports errors and warnings

## Drive Management

### Multiple Drive Support

- **Drive Registry**: Manages 12 filesystem instances
- **Drive Names**: Alpha (0), Beta (1), Gamma (2), ..., Mu (11)
- **Independent Mounts**: Each drive mounts separately
- **Shared Interface**: Common API across all drives

### Drive Detection

- **ATA PIO**: Primary/secondary IDE channels
- **Automatic Detection**: Scans for available drives
- **Fallback Mode**: Live USB when no drives found

## Limitations

- **Fixed Size**: 64 KiB max file size
- **Limited Files**: 128 files/directories per drive
- **No Permissions**: Basic readonly/hidden flags only
- **No Timestamps**: Creation time only (no modify/access)
- **No Directories**: Flat namespace (no subdirectories)
- **No Journaling**: No crash recovery protection
- **No Caching**: All operations are synchronous

## Performance Characteristics

- **Fast Lookup**: Inode table cached in memory
- **Sequential I/O**: Contiguous block allocation
- **Small Footprint**: Minimal metadata overhead
- **Predictable Latency**: Fixed disk layout

## Future Enhancements

- **BerkeFS v2**: Hierarchical directories
- **Larger Files**: Support for fragmented allocation
- **Extended Attributes**: Additional metadata fields
- **Compression**: Optional data compression
- **Encryption**: File-level encryption support

---
Note: This documentation may not always be up-to-date or may contain inaccuracies.