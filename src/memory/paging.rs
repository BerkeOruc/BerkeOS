// BerkeOS — paging.rs
// Virtual Memory and Paging

// Page table flags
pub const PAGE_PRESENT: u64 = 1 << 0;
pub const PAGE_WRITABLE: u64 = 1 << 1;
pub const PAGE_USER: u64 = 1 << 2;
pub const PAGE_PWT: u64 = 1 << 3;
pub const PAGE_PCD: u64 = 1 << 4;
pub const PAGE_ACCESSED: u64 = 1 << 5;
pub const PAGE_DIRTY: u64 = 1 << 6;
pub const PAGE_GB: u64 = 1 << 7; // 1GB page
pub const PAGE_NX: u64 = 1 << 63; // No-execute

// Memory layout
pub const KERNEL_VIRT_BASE: u64 = 0xFFFF800000000000;
pub const USER_VIRT_BASE: u64 = 0x0000000000400000;
pub const PAGE_SIZE: u64 = 4096;

// Page Map Level 4
#[repr(align(4096))]
pub struct PageMapLevel4 {
    pub entries: [u64; 512],
}

impl PageMapLevel4 {
    pub fn new() -> Self {
        PageMapLevel4 { entries: [0; 512] }
    }
}

// Page Directory Pointer Table
#[repr(align(4096))]
pub struct PageDirectoryPtr {
    pub entries: [u64; 512],
}

impl PageDirectoryPtr {
    pub fn new() -> Self {
        PageDirectoryPtr { entries: [0; 512] }
    }
}

// Page Directory
#[repr(align(4096))]
pub struct PageDirectory {
    pub entries: [u64; 512],
}

impl PageDirectory {
    pub fn new() -> Self {
        PageDirectory { entries: [0; 512] }
    }
}

// Page Table
#[repr(align(4096))]
pub struct PageTable {
    pub entries: [u64; 512],
}

impl PageTable {
    pub fn new() -> Self {
        PageTable { entries: [0; 512] }
    }
}

// Page fault error codes
pub const PF_PRESENT: u64 = 1 << 0;
pub const PF_WRITE: u64 = 1 << 1;
pub const PF_USER: u64 = 1 << 2;
pub const PF_RSVD: u64 = 1 << 3;
pub const PF_INSTR: u64 = 1 << 4;

// Virtual memory manager
pub struct VirtualMemory {
    pub pml4: *mut PageMapLevel4,
}

impl VirtualMemory {
    pub const fn new() -> Self {
        VirtualMemory {
            pml4: 0 as *mut PageMapLevel4,
        }
    }

    pub fn init(&mut self) {
        unsafe {
            self.pml4 = &mut *(0x200000 as *mut PageMapLevel4);
        }
    }

    pub fn map_page(&mut self, virt: u64, phys: u64, flags: u64) {
        if self.pml4.is_null() {
            return;
        }
        let pml4_idx = ((virt >> 39) & 0x1FF) as usize;
        unsafe {
            (*self.pml4).entries[pml4_idx] = (phys & !0xFFF) | flags | PAGE_PRESENT | PAGE_WRITABLE;
        }
    }

    pub fn get_fault_addr() -> u64 {
        unsafe {
            let addr: u64;
            core::arch::asm!("mov %cr2, {}", out(reg) addr);
            addr
        }
    }
}

pub fn enable_paging() {
    unsafe {
        let cr4: u64;
        core::arch::asm!("mov %cr4, {}", out(reg) cr4);
        let cr4 = cr4 | (1 << 5) | (1 << 7); // PAE + PSE
        core::arch::asm!("mov {}, %cr4", in(reg) cr4);
        let cr0: u64;
        core::arch::asm!("mov %cr0, {}", out(reg) cr0);
        let cr0 = cr0 | 0x80000000; // PG bit
        core::arch::asm!("mov {}, %cr0", in(reg) cr0);
    }
}

pub fn read_cr2() -> u64 {
    unsafe {
        let cr2: u64;
        core::arch::asm!("mov %cr2, {}", out(reg) cr2);
        cr2
    }
}

pub fn read_cr3() -> u64 {
    unsafe {
        let cr3: u64;
        core::arch::asm!("mov %cr3, {}", out(reg) cr3);
        cr3
    }
}

pub fn write_cr3(cr3: u64) {
    unsafe {
        core::arch::asm!("mov {}, %cr3", in(reg) cr3);
    }
}
