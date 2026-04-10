// BerkeOS — idt.rs
// Interrupt Descriptor Table — 256 entries

use core::arch::asm;
use spin::Mutex;

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct IdtEntry {
    offset_lo: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_hi: u32,
    zero: u32,
}

impl IdtEntry {
    pub const fn missing() -> Self {
        IdtEntry {
            offset_lo: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_mid: 0,
            offset_hi: 0,
            zero: 0,
        }
    }

    pub fn set_handler(&mut self, handler: unsafe extern "C" fn()) {
        let addr = handler as *const () as u64;
        self.offset_lo = (addr & 0xFFFF) as u16;
        self.offset_mid = ((addr >> 16) & 0xFFFF) as u16;
        self.offset_hi = ((addr >> 32) & 0xFFFF_FFFF) as u32;
        self.selector = 0x08;
        self.ist = 0;
        self.type_attr = 0x8E;
        self.zero = 0;
    }
}

#[repr(C, packed)]
pub struct IdtPtr {
    pub limit: u16,
    pub base: u64,
}

pub static IDT: Mutex<[IdtEntry; 256]> = Mutex::new([IdtEntry::missing(); 256]);

// ── Exception handlers ────────────────────────────────────────────────────────
macro_rules! exception_handler {
    ($name:ident, $msg:expr) => {
        pub unsafe extern "C" fn $name() {
            let vga = 0xb8000 as *mut u8;
            let msg = concat!("EXCEPTION: ", $msg, "                              ");
            for (i, b) in msg.bytes().enumerate() {
                if i >= 80 {
                    break;
                }
                vga.add(i * 2).write_volatile(b);
                vga.add(i * 2 + 1).write_volatile(0x4F);
            }
            loop {
                asm!("hlt");
            }
        }
    };
}

exception_handler!(exc_divide_error, "Divide by Zero (#DE)");
exception_handler!(exc_debug, "Debug (#DB)");
exception_handler!(exc_nmi, "NMI Interrupt");
exception_handler!(exc_breakpoint, "Breakpoint (#BP)");
exception_handler!(exc_overflow, "Overflow (#OF)");
exception_handler!(exc_bound_range, "BOUND Range Exceeded (#BR)");
exception_handler!(exc_invalid_opcode, "Invalid Opcode (#UD)");
exception_handler!(exc_device_na, "Device Not Available (#NM)");
exception_handler!(exc_double_fault, "Double Fault (#DF)");
exception_handler!(exc_invalid_tss, "Invalid TSS (#TS)");
exception_handler!(exc_segment_np, "Segment Not Present (#NP)");
exception_handler!(exc_stack_fault, "Stack Fault (#SS)");
exception_handler!(exc_gpf, "General Protection Fault (#GP)");
exception_handler!(exc_page_fault, "Page Fault (#PF)");
exception_handler!(exc_x87_fp, "x87 FP Exception (#MF)");
exception_handler!(exc_alignment, "Alignment Check (#AC)");
exception_handler!(exc_machine_check, "Machine Check (#MC)");
exception_handler!(exc_simd_fp, "SIMD FP Exception (#XM)");

// ── IRQ handlers from pic.rs ──────────────────────────────────────────────────
extern "C" {
    pub fn irq0_handler();
    pub fn irq1_handler();
    pub fn irq_spurious();
}

// ── Init ──────────────────────────────────────────────────────────────────────
pub fn init() {
    let mut idt = IDT.lock();

    idt[0].set_handler(exc_divide_error);
    idt[1].set_handler(exc_debug);
    idt[2].set_handler(exc_nmi);
    idt[3].set_handler(exc_breakpoint);
    idt[4].set_handler(exc_overflow);
    idt[5].set_handler(exc_bound_range);
    idt[6].set_handler(exc_invalid_opcode);
    idt[7].set_handler(exc_device_na);
    idt[8].set_handler(exc_double_fault);
    idt[10].set_handler(exc_invalid_tss);
    idt[11].set_handler(exc_segment_np);
    idt[12].set_handler(exc_stack_fault);
    idt[13].set_handler(exc_gpf);
    idt[14].set_handler(exc_page_fault);
    idt[16].set_handler(exc_x87_fp);
    idt[17].set_handler(exc_alignment);
    idt[18].set_handler(exc_machine_check);
    idt[19].set_handler(exc_simd_fp);

    // IRQ0=timer IRQ1=keyboard after PIC remap to 32+
    let irq0 = irq0_handler as *const () as u64;
    let irq1 = irq1_handler as *const () as u64;
    let irqs = irq_spurious as *const () as u64;

    set_raw(&mut idt[32], irq0);
    set_raw(&mut idt[33], irq1);
    for i in 34..48usize {
        set_raw(&mut idt[i], irqs);
    }

    let ptr = IdtPtr {
        limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: idt.as_ptr() as u64,
    };
    drop(idt);
    unsafe {
        asm!("lidt [{}]", in(reg) &ptr, options(nostack));
    }
}

fn set_raw(entry: &mut IdtEntry, addr: u64) {
    entry.offset_lo = (addr & 0xFFFF) as u16;
    entry.offset_mid = ((addr >> 16) & 0xFFFF) as u16;
    entry.offset_hi = ((addr >> 32) & 0xFFFF_FFFF) as u32;
    entry.selector = 0x08;
    entry.ist = 0;
    entry.type_attr = 0x8E;
    entry.zero = 0;
}
