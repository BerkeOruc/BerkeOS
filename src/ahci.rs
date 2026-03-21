// BerkeOS — SATA/AHCI Driver
// AHCI (Advanced Host Controller Interface) for modern storage

use core::hint::spin_loop;
use spin::Mutex;

const AHCI_BAR5: u32 = 0x0; // Will be discovered via PCI

// AHCI Registers (offset from base)
const HBA_CAP: u32 = 0x00; // Host Capabilities
const HBA_GHC: u32 = 0x04; // Global Host Control
const HBA_IS: u32 = 0x08; // Interrupt Status
const HBA_PI: u32 = 0x0C; // Ports Implemented
const HBA_VS: u32 = 0x10; // Version

// Port registers (0x100 + port * 0x80)
const PORT_CLD: u32 = 0x00; // Command List Base Address
const PORT_FIS: u32 = 0x08; // FIS Base Address
const PORT_IS: u32 = 0x10; // Interrupt Status
const PORT_IE: u32 = 0x14; // Interrupt Enable
const PORT_CMD: u32 = 0x18; // Command and Status
const PORT_TFD: u32 = 0x20; // Task File Data
const PORT_SIG: u32 = 0x24; // Signature
const PORT_SCTL: u32 = 0x2C; // SATA Control
const PORT_SERR: u32 = 0x30; // SATA Error
const PORT_SACT: u32 = 0x34; // SATA Active
const PORT_CI: u32 = 0x38; // Command Issue

// HBA_CAP bits
const CAP_S64A: u32 = 1 << 31; // 64-bit support
const CAP_SNCQ: u32 = 1 << 30; // Native Command Queuing
const CAP_SSNTF: u32 = 1 << 29; // SNotification
const CAP_SMPS: u32 = 1 << 28; // Port Multiplier
const CAP_SSS: u32 = 1 << 27; // Staggered Spin-up
const CAP_SALP: u32 = 1 << 26; // Activity LED
const CAP_SAL: u32 = 1 << 25; // Aggressive Link Power
const CAP_CLO: u32 = 1 << 24; // Command List Override
const CAP_HTAA: u32 = 1 << 17; // HBA Reset Assisted
const CAP_PMD: u32 = 1 << 15; // PIO Multiple DRQ Block
const CAP_SSCF: u32 = 1 << 14; // Slumber State Capable
const CAP_PSC: u32 = 1 << 13; // Partial State Capable
const CAP_NCS: u32 = 1 << 7; // Number of Command Slots
const CAP_PSC2: u32 = 1 << 6; // Partial State Capable 2
const CAP_FBSS: u32 = 1 << 5; // FIS-based switching
const CAP_SPM: u32 = 1 << 4; // Port Multiplier
const CAP_MAX_PORT: u32 = 0x1F << 0; // Max ports

// PORT_CMD bits
const CMD_ST: u32 = 1 << 0; // Start
const CMD_SUD: u32 = 1 << 1; // Spin-up Device
const CMD_POD: u32 = 1 << 2; // Power On Device
const CMD_CLO: u32 = 1 << 3; // Command List Override
const CMD_FRE: u32 = 1 << 4; // FIS Receive Enable
const CMD_WUE: u32 = 1 << 5; // Wake Enable
const CMD_U0: u32 = 1 << 14; // Device is in U0
const CMD_U1: u32 = 1 << 15; // Device is in U1
const CMD_U2: u32 = 1 << 16; // Device is in U2
const CMD_PMA: u32 = 1 << 17; // Port Multiplier Attach
const CMD_HPCP: u32 = 1 << 18; // Hot Plug Capable
const CMD_MPSP: u32 = 1 << 19; // Mechanical Presence State
const CMD_CPD: u32 = 1 << 20; // Cold Presence Detection
const CMD_ESP: u32 = 1 << 21; // External SATA Port
const CMD_ALPE: u32 = 1 << 26; // Aggressive Low Power
const CMD_DLAE: u32 = 1 << 27; // Device-led Activity
const CMD_ATAPI: u32 = 1 << 24; // ATAPI

// PORT_TFD bits
const TFD_ERR: u32 = 1 << 0; // Error
const TFD_DRQ: u32 = 1 << 3; // Data Request
const TFD_SDBY: u32 = 1 << 7; // Standby
const TFD_BUSY: u32 = 1 << 7; // Busy

// SATA signatures
const SATA_SIG_ATA: u32 = 0x00000101;
const SATA_SIG_ATAPI: u32 = 0xEB140101;
const SATA_SIG_SEMB: u32 = 0xC33C0101;
const SATA_SIG_PM: u32 = 0x96690101;

// ATA commands
const ATA_CMD_READ_DMA_EXT: u8 = 0x25;
const ATA_CMD_WRITE_DMA_EXT: u8 = 0x35;
const ATA_CMD_FLUSH_CACHE_EXT: u8 = 0xEA;
const ATA_CMD_IDENTIFY_PACKET: u8 = 0xA1;
const ATA_CMD_IDENTIFY: u8 = 0xEC;

// FIS types
const FIS_TYPE_H2D: u8 = 0x27;
const FIS_TYPE_D2H: u8 = 0x34;
const FIS_TYPE_DMA: u8 = 0x41;
const FIS_TYPE_PIO: u8 = 0x5F;
const FIS_TYPE_SETDEVICE: u8 = 0x0A;

// Command slot
#[repr(C)]
pub struct HBACmdSlot {
    pub cmd_fis_len: u8,
    pub prdt_len: u16,
    pub prd_byte_cnt: u32,
    pub cmd_table_base: u32,
    pub cmd_table_base_hi: u32,
    pub rsvd: [u32; 4],
}

// PRD entry
#[repr(C)]
pub struct HBAPrd {
    pub data_base: u32,
    pub data_base_hi: u32,
    pub rsvd: u32,
    pub byte_cnt: u32,
    pub prdt_len: u32,
}

// Command table (FIS + PRD table)
#[repr(C)]
pub struct HBACmdTable {
    pub fis: [u8; 64],
    pub atapi_cmd: [u8; 16],
    pub prdt: [HBAPrd; 1], // Simplified: 1 PRD entry
}

// I/O port helpers
#[inline]
unsafe fn inb(port: u32) -> u8 {
    let val: u8;
    core::arch::asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack));
    val
}

#[inline]
unsafe fn outb(port: u32, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack));
}

#[inline]
unsafe fn inl(port: u32) -> u32 {
    let val: u32;
    core::arch::asm!("in eax, dx", out("eax") val, in("dx") port, options(nomem, nostack));
    val
}

#[inline]
unsafe fn outl(port: u32, val: u32) {
    core::arch::asm!("out dx, eax", in("dx") port, in("eax") val, options(nomem, nostack));
}

// AHCI device
pub struct AhciDevice {
    pub base: u32,
    pub ports_implemented: u32,
    pub port_count: u8,
    pub found: bool,
}

impl AhciDevice {
    pub const fn new() -> Self {
        AhciDevice {
            base: 0,
            ports_implemented: 0,
            port_count: 0,
            found: false,
        }
    }

    pub unsafe fn detect() -> bool {
        // Scan PCI for AHCI controller
        // For now, try common BAR5 addresses
        let bases = [0xFE100000, 0xFE000000, 0xFD000000, 0x1000];

        for base in bases.iter() {
            if Self::probe(*base) {
                return true;
            }
        }
        false
    }

    unsafe fn probe(base: u32) -> bool {
        // Check for AHCI signature "AHCI" at BAR5 + 0x24
        let sig = inl(base + 0x24);
        if sig == 0x41484341 {
            // "AHCI" in LE
            return true;
        }
        false
    }

    pub unsafe fn init(&mut self, base: u32) -> bool {
        self.base = base;

        // Read capabilities
        let cap = inl(self.base + HBA_CAP);
        self.port_count = ((cap & 0x1F) + 1) as u8;
        self.ports_implemented = inl(self.base + HBA_PI);

        // Reset HBA
        let ghc = inl(self.base + HBA_GHC);
        outl(self.base + HBA_GHC, ghc | 1); // Set HBA Reset

        // Wait for reset
        let mut timeout = 1000000;
        while timeout > 0 {
            let ghc = inl(self.base + HBA_GHC);
            if ghc & 1 == 0 {
                break;
            }
            timeout -= 1;
            spin_loop();
        }

        if timeout == 0 {
            return false;
        }

        // Enable AHCI
        outl(self.base + HBA_GHC, 1 << 31); // AE = AHCI Enable

        self.found = true;
        true
    }

    pub unsafe fn detect_port(&self, port: u8) -> bool {
        if self.ports_implemented & (1 << port) == 0 {
            return false;
        }

        let port_base = self.base + 0x100 + (port as u32) * 0x80;

        // Check if device present
        let status = inl(port_base + PORT_TFD);
        if status & 0xFFFF == 0xFFFF {
            return false;
        }

        true
    }

    pub unsafe fn init_port(&self, port: u8) -> bool {
        if !self.detect_port(port) {
            return false;
        }

        let port_base = self.base + 0x100 + (port as u32) * 0x80;

        // Stop command engine
        let cmd = inl(port_base + PORT_CMD);
        outl(port_base + PORT_CMD, cmd & !CMD_ST);

        // Disable FIS receive
        outl(port_base + PORT_CMD, cmd & !CMD_FRE);

        // Clear pending
        outl(port_base + PORT_IS, 0xFFFFFFFF);

        // Setup command list (simplified - single slot)
        // In real implementation, would allocate proper memory

        // Start command engine
        let cmd = inl(port_base + PORT_CMD);
        outl(port_base + PORT_CMD, cmd | CMD_ST | CMD_FRE);

        true
    }

    pub unsafe fn read_sector(&self, port: u8, lba: u64, buf: &mut [u8; 512]) -> bool {
        if !self.detect_port(port) {
            return false;
        }

        let port_base = self.base + 0x100 + (port as u32) * 0x80;

        // Check if busy
        let tfd = inl(port_base + PORT_TFD);
        if tfd & (TFD_BUSY | TFD_DRQ) != 0 {
            return false;
        }

        // Build FIS
        let mut fis = [0u8; 64];
        fis[0] = FIS_TYPE_H2D;
        fis[1] = 0x80; // Command
        fis[2] = ATA_CMD_READ_DMA_EXT;
        fis[4] = (lba & 0xFF) as u8;
        fis[5] = ((lba >> 8) & 0xFF) as u8;
        fis[6] = ((lba >> 16) & 0xFF) as u8;
        fis[7] = 0x40; // LBA mode
        fis[8] = ((lba >> 24) & 0xFF) as u8;
        fis[12] = ((lba >> 32) & 0xFF) as u8;
        fis[13] = ((lba >> 40) & 0xFF) as u8;
        fis[15] = 1; // Sector count

        // Write to command slot
        // Simplified - would need proper cmd table in real implementation

        // Issue command
        outl(port_base + PORT_CI, 1);

        // Wait for completion
        let mut timeout = 1000000;
        while timeout > 0 {
            let is = inl(port_base + PORT_IS);
            if is & 1 != 0 {
                // D2H FIS received
                outl(port_base + PORT_IS, 1);
                return true;
            }
            if is & 0x08 != 0 {
                // Error
                outl(port_base + PORT_IS, 0x08);
                return false;
            }
            timeout -= 1;
            spin_loop();
        }

        false
    }
}

// Global AHCI instance
pub static AHCI: Mutex<AhciDevice> = Mutex::new(AhciDevice::new());

pub unsafe fn ahci_init() -> bool {
    AhciDevice::detect()
}

pub unsafe fn ahci_read_sector(lba: u64, buf: &mut [u8; 512]) -> bool {
    AHCI.lock().read_sector(0, lba, buf)
}
