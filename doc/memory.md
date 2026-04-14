# BerkeOS Memory Management

## Overview

BerkeOS implements a simple but functional memory management system with virtual memory paging and a bump heap allocator. The system uses x86_64's 4-level paging with 2 MiB huge pages for identity mapping the first 4 GiB of physical memory.

## Virtual Memory Architecture

### Page Table Structure

BerkeOS uses the standard x86_64 4-level paging hierarchy:

- **PML4 (Page Map Level 4)**: Top-level table (512 entries)
- **PDPT (Page Directory Pointer Table)**: Second level (512 entries)
- **PD (Page Directory)**: Third level (512 entries)
- **PT (Page Table)**: Fourth level (512 entries, 4 KiB pages)

### Page Table Flags

```rust
pub const PAGE_PRESENT: u64 = 1 << 0;      // Page is present in memory
pub const PAGE_WRITABLE: u64 = 1 << 1;    // Page is writable
pub const PAGE_USER: u64 = 1 << 2;        // Page is accessible from user mode
pub const PAGE_PWT: u64 = 1 << 3;         // Page write-through caching
pub const PAGE_PCD: u64 = 1 << 4;         // Page cache disabled
pub const PAGE_ACCESSED: u64 = 1 << 5;    // Page has been accessed
pub const PAGE_DIRTY: u64 = 1 << 6;       // Page has been written to
pub const PAGE_GB: u64 = 1 << 7;          // 1 GiB page
pub const PAGE_NX: u64 = 1 << 63;         // No-execute page
```

### Memory Layout

```
Virtual Address Space:
0x0000000000000000 - 0x00000000003FFFFF: User space (4 MiB)
0x0000000000400000 - 0xFFFF7FFFFFFFFFFF: User space (vast majority)
0xFFFF800000000000 - 0xFFFFFFFFFFFFFFFF: Kernel space (128 TiB)

Physical Memory Mapping:
0x00000000 - 0xFFFFFFFF: Identity mapped (4 GiB) using 2 MiB huge pages
```

### Boot-time Page Tables

During boot, `boot.asm` sets up identity mapping for the first 4 GiB:

```nasm
setup_page_tables:
    ; Create 4 PDPT entries, each covering 1 GiB
    ; Each PD has 512 entries of 2 MiB pages
    ; Total: 4 GiB identity mapped
```

## Heap Allocator

### Bump Allocator Design

BerkeOS uses a simple bump allocator for kernel heap memory:

```rust
pub struct KernelAllocator;

static HEAP_OFFSET: AtomicUsize = AtomicUsize::new(HEAP_START);

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let current = HEAP_OFFSET.load(Ordering::Acquire);
        let aligned = (current + align - 1) & !(align - 1);
        let new_offset = aligned + size;
        
        if new_offset > HEAP_END {
            return null_mut();
        }
        
        HEAP_OFFSET.store(new_offset, Ordering::Release);
        aligned as *mut u8
    }
    
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // No-op: bump allocator doesn't support deallocation
    }
}
```

### Heap Configuration

- **Start Address**: `0xFFFF_FFFF_8000_0000`
- **Size**: 16 MiB
- **End Address**: `HEAP_START + HEAP_SIZE`
- **Allocation**: First-fit alignment, no fragmentation tracking
- **Deallocation**: Not supported (memory leak tolerant for kernel)

### Memory Statistics

```rust
pub fn heap_used() -> usize {
    HEAP_OFFSET.load(Ordering::Acquire) - HEAP_START
}

pub fn heap_available() -> usize {
    HEAP_END - HEAP_OFFSET.load(Ordering::Acquire)
}
```

## Page Fault Handling

### Page Fault Analysis

Page faults are handled by the CPU exception handler (#PF at interrupt 14):

```rust
// Page fault error code bits
pub const PF_PRESENT: u64 = 1 << 0;    // Page present in memory
pub const PF_WRITE: u64 = 1 << 1;     // Write access attempted
pub const PF_USER: u64 = 1 << 2;      // Fault in user mode
pub const PF_RSVD: u64 = 1 << 3;      // Reserved bit set
pub const PF_INSTR: u64 = 1 << 4;     // Instruction fetch
```

### Fault Address Retrieval

```rust
pub fn get_fault_addr() -> u64 {
    unsafe {
        let addr: u64;
        core::arch::asm!("mov %cr2, {}", out(reg) addr);
        addr
    }
}
```

## Virtual Memory Manager

### VMM Structure

```rust
pub struct VirtualMemory {
    pub pml4: *mut PageMapLevel4,
}

impl VirtualMemory {
    pub fn map_page(&mut self, virt: u64, phys: u64, flags: u64) {
        let pml4_idx = ((virt >> 39) & 0x1FF) as usize;
        unsafe {
            (*self.pml4).entries[pml4_idx] = (phys & !0xFFF) | flags | PAGE_PRESENT | PAGE_WRITABLE;
        }
    }
}
```

### Page Table Manipulation

- **CR3**: Contains physical address of PML4
- **TLB**: Translation Lookaside Buffer for caching translations
- **Invalidation**: `invlpg` instruction for single page, CR3 reload for full flush

## Memory Protection

### Kernel/User Separation

- **Kernel Space**: `0xFFFF800000000000+` - kernel code, data, heap
- **User Space**: `0x0000000000400000-` - user programs (BexVM)
- **NX Bit**: No-execute protection on data pages
- **Supervisor Mode**: Kernel runs in ring 0, user in ring 3 (future)

### Safety Features

- **Rust Ownership**: Compile-time memory safety
- **No Double Mapping**: Each physical page mapped once
- **Alignment**: Proper alignment for all allocations
- **Bounds Checking**: Heap bounds enforcement

## Limitations

- **No Swapping**: All memory must fit in physical RAM
- **No Deallocation**: Bump allocator leaks memory
- **No Demand Paging**: All pages allocated upfront
- **No User Space**: Current kernel doesn't support user processes
- **Fixed Heap Size**: 16 MiB kernel heap limit

## Future Enhancements

- **Slab Allocator**: Replace bump allocator with slab allocation
- **Virtual Memory**: Implement proper virtual address spaces
- **Swapping**: Add disk-based page swapping
- **Memory Protection**: User/kernel space isolation
- **NUMA Support**: Multi-socket memory awareness

---
Note: This documentation may not always be up-to-date or may contain inaccuracies.