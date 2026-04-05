// BerkeOS — allocator.rs
// Kernel Heap Allocator (Bump Allocator)

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

pub const HEAP_START: usize = 0xFFFF_FFFF_8000_0000;
pub const HEAP_SIZE: usize = 16 * 1024 * 1024;
pub const HEAP_END: usize = HEAP_START + HEAP_SIZE;

static HEAP_OFFSET: AtomicUsize = AtomicUsize::new(HEAP_START);

pub struct KernelAllocator;

impl KernelAllocator {
    pub const fn new() -> Self {
        KernelAllocator
    }
}

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        if size == 0 {
            return core::ptr::null_mut();
        }

        let current = HEAP_OFFSET.load(Ordering::Acquire);
        let aligned = (current + align - 1) & !(align - 1);
        let new_offset = aligned + size;

        if new_offset > HEAP_END {
            core::ptr::null_mut()
        } else {
            HEAP_OFFSET.store(new_offset, Ordering::Release);
            aligned as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator doesn't support free
    }
}

pub fn init_heap() {
    HEAP_OFFSET.store(HEAP_START, Ordering::Release);
}

pub fn heap_used() -> usize {
    HEAP_OFFSET.load(Ordering::Acquire) - HEAP_START
}

pub fn heap_available() -> usize {
    HEAP_END - HEAP_OFFSET.load(Ordering::Acquire)
}
