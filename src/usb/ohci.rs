// BerkeOS — usb/ohci.rs
// OHCI (Open Host Controller Interface) USB driver

use crate::usb::{
    ConfigDescriptor, DeviceDescriptor, EndpointDescriptor, InterfaceDescriptor, PortStatus,
    SetupPacket, UsbController, UsbDevice, UsbState, USB_CLASS_MASS_STORAGE, USB_DIR_IN,
    USB_DIR_OUT, USB_PROTOCOL_BOT, USB_REQ_GET_DESCRIPTOR, USB_REQ_SET_ADDRESS,
    USB_REQ_SET_CONFIGURATION, USB_SPEED_FULL, USB_SPEED_HIGH, USB_SPEED_LOW,
};
use spin::Mutex;

// OHCI Register Offsets
const OHCI_REG_REVISION: u32 = 0x00;
const OHCI_REG_CONTROL: u32 = 0x04;
const OHCI_REG_CMDSTATUS: u32 = 0x08;
const OHCI_REG_INTERRUPT: u32 = 0x0C;
const OHCI_REG_HCCA: u32 = 0x10;
const OHCI_REG_PERIODIC_ED: u32 = 0x14;
const OHCI_REG_CONTROL_ED: u32 = 0x18;
const OHCI_REG_BULK_ED: u32 = 0x1C;
const OHCI_REG_DONE_HEAD: u32 = 0x20;
const OHCI_REG_FM_INTERVAL: u32 = 0x34;
const OHCI_REG_FM_REMAINING: u32 = 0x38;
const OHCI_REG_FM_NUMBER: u32 = 0x3C;
const OHCI_REG_RH_STATUS: u32 = 0x40;
const OHCI_REG_RH_PORT1: u32 = 0x44;

// OHCI Control Register Bits
const OHCI_CTRL_HCFS: u16 = 0x00C0;
const OHCI_CTRL_HC_RESET: u16 = 0x0000;
const OHCI_CTRL_HC_RESUME: u16 = 0x0040;
const OHCI_CTRL_HC_OPERATIONAL: u16 = 0x0080;
const OHCI_CTRL_HC_SUSPEND: u16 = 0x00C0;
const OHCI_CTRL_CLE: u16 = 0x0010;
const OHCI_CTRL_BLE: u16 = 0x0020;
const OHCI_CTRL_IE: u16 = 0x0008;

// OHCI Command Status Register Bits
const OHCI_CMD_HCR: u32 = 0x00000001;
const OHCI_CMD_CLF: u32 = 0x00000002;
const OHCI_CMD_BLF: u32 = 0x00000004;
const OHCI_CMD_OCR: u32 = 0x00000008;

// OHCI Interrupt Register Bits
const OHCI_INT_SO: u32 = 0x80000000;
const OHCI_INT_WDH: u32 = 0x40000000;
const OHCI_INT_SF: u32 = 0x20000000;
const OHCI_INT_RD: u32 = 0x10000000;
const OHCI_INT_UE: u32 = 0x04000000;
const OHCI_INT_FNO: u32 = 0x02000000;
const OHCI_INT_RHSC: u32 = 0x01000000;
const OHCI_INT_MIE: u32 = 0x01000000;

// RH Status Register Bits
const OHCI_RHS_LPS: u32 = 0x00000001;
const OHCI_RHS_OCI: u32 = 0x00000004;
const OHCI_RHS_DRWE: u32 = 0x00008000;
const OHCI_RHS_LPSC: u32 = 0x00010000;
const OHCI_RHS_OCIC: u32 = 0x00040000;
const OHCI_RHS_CRWE: u32 = 0x80000000;

// RH Port Status Register Bits
const OHCI_RHP_CCS: u32 = 0x00000001;
const OHCI_RHP_PES: u32 = 0x00000002;
const OHCI_RHP_PSS: u32 = 0x00000004;
const OHCI_RHP_POCI: u32 = 0x00000008;
const OHCI_RHP_PRS: u32 = 0x00000010;
const OHCI_RHP_LSDA: u32 = 0x00000100;
const OHCI_RHP_CSC: u32 = 0x00010000;
const OHCI_RHP_PESC: u32 = 0x00020000;
const OHCI_RHP_PSSC: u32 = 0x00040000;
const OHCI_RHP_OCIC: u32 = 0x00080000;
const OHCI_RHP_PRSC: u32 = 0x00100000;

// Endpoint Descriptor Flags
const OHCI_ED_DIR_OUT: u32 = 0x00000000;
const OHCI_ED_DIR_IN: u32 = 0x00000100;
const OHCI_ED_DIR_UNDEF: u32 = 0x00000200;
const OHCI_ED_SPEED_FULL: u32 = 0x00002000;
const OHCI_ED_SPEED_LOW: u32 = 0x00004000;
const OHCI_ED_SKIP: u32 = 0x00008000;
const OHCI_ED_FORMAT_GEN: u32 = 0x00000000;
const OHCI_ED_FORMAT_ISO: u32 = 0x00010000;
const OHCI_ED_FORMAT_INT: u32 = 0x00020000;

// Transfer Descriptor Flags
const OHCI_TD_DIR_OUT: u32 = 0x00000000;
const OHCI_TD_DIR_IN: u32 = 0x00100000;
const OHCI_TD_DIR_SETUP: u32 = 0x00200000;
const OHCI_TD_TOGGLE_0: u32 = 0x00400000;
const OHCI_TD_TOGGLE_1: u32 = 0x00800000;
const OHCI_TD_NO_COPY: u32 = 0x01000000;
const OHCI_TD_ACTUAL_LENGTH: u32 = 0x00000FFF;
const OHCI_TD_CC: u32 = 0xF0000000;
const OHCI_TD_CC_NO_ERROR: u32 = 0x00000000;
const OHCI_TD_CC_CRC: u32 = 0x10000000;
const OHCI_TD_CC_BITSTUFF: u32 = 0x20000000;
const OHCI_TD_CC_STALL: u32 = 0x40000000;
const OHCI_TD_CC_TIMEOUT: u32 = 0x50000000;

// OHCI Base Address (default for QEMU)
const DEFAULT_OHCI_BASE: u32 = 0xEDC00000;

// Endpoint Descriptor
#[repr(C)]
pub struct OhciEd {
    pub next_ed: u32,
    pub flags: u32,
    pub tail_td: u32,
    pub head_td: u32,
    pub next_ed_copy: u32,
}

impl OhciEd {
    pub fn new() -> Self {
        OhciEd {
            next_ed: 0,
            flags: 0,
            tail_td: 0,
            head_td: 0,
            next_ed_copy: 0,
        }
    }

    pub fn set_address(&mut self, addr: u8) {
        self.flags = (self.flags & !0x0000007F) | ((addr as u32) & 0x7F);
    }

    pub fn set_ep_number(&mut self, ep: u8) {
        self.flags = (self.flags & !0x00000078) | (((ep as u32) & 0xF) << 3);
    }

    pub fn set_direction(&mut self, dir: u32) {
        self.flags = (self.flags & !0x00000300) | dir;
    }

    pub fn set_speed(&mut self, speed: u32) {
        self.flags = (self.flags & !0x00006000) | speed;
    }

    pub fn set_max_packet(&mut self, max: u16) {
        self.flags = (self.flags & !0x07FF0000) | (((max as u32) & 0x7FF) << 16);
    }

    pub fn set_skip(&mut self, skip: bool) {
        if skip {
            self.flags |= OHCI_ED_SKIP;
        } else {
            self.flags &= !OHCI_ED_SKIP;
        }
    }
}

// Transfer Descriptor
#[repr(C)]
pub struct OhciTd {
    pub next_td: u32,
    pub be: u32,
    pub cbp: u32,
    pub flags: u16,
    pub frame: u16,
    pub data: [u8; 36],
}

impl OhciTd {
    pub fn new() -> Self {
        OhciTd {
            next_td: 0,
            be: 0,
            cbp: 0,
            flags: 0,
            frame: 0,
            data: [0; 36],
        }
    }

    pub fn set_data(&mut self, buf: &[u8]) {
        let len = buf.len().min(36);
        self.data[..len].copy_from_slice(&buf[..len]);
    }

    pub fn set_direction(&mut self, dir: u32) {
        let flags = (self.flags as u32 & !0x00300000) | dir;
        self.flags = flags as u16;
    }

    pub fn get_cc(&self) -> u8 {
        ((self.flags as u32 >> 28) & 0xF) as u8
    }

    pub fn get_actual_length(&self) -> u16 {
        (self.flags as u32 & 0x00000FFF) as u16
    }
}

// Host Controller Communications Area
#[repr(C)]
pub struct HccaStruct {
    pub interrupt_table: [u32; 32],
    pub frame_number: u16,
    pub done_head: u32,
    pub reserved: [u8; 116],
}

impl HccaStruct {
    pub fn new() -> Self {
        HccaStruct {
            interrupt_table: [0; 32],
            frame_number: 0,
            done_head: 0,
            reserved: [0; 116],
        }
    }
}

// OHCI Controller
pub struct OhciController {
    pub base: u32,
    pub present: bool,
    pub hcca_ptr: u32,
    pub control_ed_ptr: u32,
    pub bulk_ed_ptr: u32,
    pub devices: [UsbDevice; 128],
}

impl OhciController {
    pub const fn new() -> Self {
        OhciController {
            base: 0,
            present: false,
            hcca_ptr: 0,
            control_ed_ptr: 0,
            bulk_ed_ptr: 0,
            devices: [UsbDevice::new(); 128],
        }
    }

    pub unsafe fn read_reg(&self, offset: u32) -> u32 {
        let ptr = (self.base + offset) as *mut u32;
        ptr.read_volatile()
    }

    pub unsafe fn write_reg(&self, offset: u32, value: u32) {
        let ptr = (self.base + offset) as *mut u32;
        ptr.write_volatile(value)
    }

    pub unsafe fn init(&mut self, base: u32) -> bool {
        self.base = base;
        self.hcca_ptr = 0x100000;
        self.control_ed_ptr = 0x101000;
        self.bulk_ed_ptr = 0x101800;

        let rev = self.read_reg(OHCI_REG_REVISION);
        if (rev & 0xFF) != 0x10 {
            return false;
        }

        self.present = true;

        let rev = self.read_reg(OHCI_REG_REVISION);
        if (rev & 0xFF) != 0x10 {
            return false;
        }

        self.present = true;

        // Reset controller
        self.write_reg(OHCI_REG_CMDSTATUS, OHCI_CMD_HCR);
        for _ in 0..100 {
            let status = self.read_reg(OHCI_REG_CMDSTATUS);
            if (status & OHCI_CMD_HCR) == 0 {
                break;
            }
        }

        // Disable interrupts
        self.write_reg(OHCI_REG_INTERRUPT, 0);

        // Set HCCA address
        self.write_reg(OHCI_REG_HCCA, self.hcca_ptr);

        // Set control and bulk ED addresses
        self.write_reg(OHCI_REG_CONTROL_ED, self.control_ed_ptr);
        self.write_reg(OHCI_REG_BULK_ED, self.bulk_ed_ptr);

        // Set FmInterval
        self.write_reg(OHCI_REG_FM_INTERVAL, 0x27782EDF);

        // Set HC to operational
        self.write_reg(
            OHCI_REG_CONTROL,
            (OHCI_CTRL_HC_OPERATIONAL as u32)
                | (OHCI_CTRL_IE as u32)
                | (OHCI_CTRL_CLE as u32)
                | (OHCI_CTRL_BLE as u32),
        );

        true
    }

    pub unsafe fn get_roothub_status(&self) -> u32 {
        self.read_reg(OHCI_REG_RH_STATUS)
    }

    pub unsafe fn get_port_status(&self, port: u8) -> u32 {
        self.read_reg(OHCI_REG_RH_PORT1 + ((port as u32) * 4))
    }

    pub unsafe fn set_port_status(&self, port: u8, value: u32) {
        self.write_reg(OHCI_REG_RH_PORT1 + ((port as u32) * 4), value);
    }

    pub unsafe fn reset_port(&mut self, port: u8) -> bool {
        // Set port reset
        self.set_port_status(port, OHCI_RHP_PRS);

        // Wait for reset to complete
        for _ in 0..10000 {
            let status = self.get_port_status(port);
            if (status & OHCI_RHP_PRS) == 0 {
                // Check for connection
                if (status & OHCI_RHP_CCS) != 0 {
                    // Enable port
                    self.set_port_status(port, OHCI_RHP_PES);
                    return true;
                }
                return false;
            }
        }
        false
    }

    pub unsafe fn detect_device(&mut self, port: u8) -> Option<&mut UsbDevice> {
        let status = self.get_port_status(port);

        if (status & OHCI_RHP_CCS) == 0 {
            return None;
        }

        // Get device speed
        let speed = if (status & OHCI_RHP_LSDA) != 0 {
            USB_SPEED_LOW
        } else {
            USB_SPEED_FULL
        };

        // Reset port and get device
        if !self.reset_port(port) {
            return None;
        }

        // Assign address 0 for enumeration
        let dev = &mut self.devices[0];
        dev.address = 0;
        dev.speed = speed;
        dev.state = UsbState::Default;

        Some(dev)
    }

    pub unsafe fn get_descriptor(
        &self,
        dev_addr: u8,
        dtype: u8,
        dindex: u8,
        len: u16,
        buf: &mut [u8],
    ) -> bool {
        let mut setup = SetupPacket::get_descriptor(dev_addr, dtype, dindex, len);

        // Control transfer would happen here
        // For now, return false - actual implementation needs TD/ED setup
        false
    }
}

// PCI IDs for USB Controllers
const PCI_CLASS_USB: u8 = 0x0C;
const PCI_SUBCLASS_OHCI: u8 = 0x10;

// OHCI Controller instance
pub static OHCI: Mutex<OhciController> = Mutex::new(OhciController::new());

// Inb/outb for I/O port access
unsafe fn inl(port: u16) -> u32 {
    let result: u32;
    core::arch::asm!("inl {0}, {1}", in(reg) port, out(reg) result);
    result
}

unsafe fn outl(port: u16, value: u32) {
    core::arch::asm!("outl {1}, {0}", in(reg) port, in(reg) value);
}

// USB Initialization
pub unsafe fn usb_init() -> bool {
    // Try to init OHCI at default base
    OHCI.lock().init(DEFAULT_OHCI_BASE)
}

pub fn usb_get_controller() -> spin::MutexGuard<'static, OhciController> {
    OHCI.lock()
}
