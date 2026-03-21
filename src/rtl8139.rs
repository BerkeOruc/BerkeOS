// BerkeOS — rtl8139.rs
// RTL8139 Network Interface Card Driver

use crate::net::{EthHeader, NetInterface, ETH_ALEN, ETH_P_ARP, ETH_P_IP};
use spin::Mutex;

// RTL8139 I/O Port Base (from PCI BAR0)
const RTL8139_IO_BASE: u16 = 0xD000;

// RTL8139 Register Offsets
const RTL8139_MAC0: u16 = 0x00;
const RTL8139_MAC1: u16 = 0x04;
const RTL8139_MAC2: u16 = 0x08;
const RTL8139_MAC3: u16 = 0x0C;
const RTL8139_MAC4: u16 = 0x10;
const RTL8139_CMD: u16 = 0x37;
const RTL8139_IMR: u16 = 0x3C;
const RTL8139_ISR: u16 = 0x3E;
const RTL8139_RCR: u16 = 0x44;
const RTL8139_TCR: u16 = 0x40;
const RTL8139_RXBUF: u16 = 0x30;
const RTL8139_TX0: u16 = 0x20;
const RTL8139_TX1: u16 = 0x24;
const RTL8139_TX2: u16 = 0x28;
const RTL8139_TX3: u16 = 0x2C;

// RTL8139 Command Bits
const CMD_RESET: u8 = 0x10;
const CMD_RX_EN: u8 = 0x08;
const CMD_TX_EN: u8 = 0x04;

// RTL8139 RX Configuration Bits
const RCR_EN_WRAP: u32 = 0x80;
const RCR_ACCEPT_BROADCAST: u32 = 0x04;
const RCR_ACCEPT_MULTICAST: u32 = 0x02;
const RCR_ACCEPT_PHYS_MATCH: u32 = 0x01;
const RCR_ACCEPT_ALL_PHYS: u32 = 0x20;
const RCR_ACCEPT_MAC: u32 = RCR_ACCEPT_BROADCAST | RCR_ACCEPT_MULTICAST | RCR_ACCEPT_PHYS_MATCH;

// RTL8139 Interrupt Status Bits
const ISR_ROK: u16 = 0x0001;
const ISR_RER: u16 = 0x0002;
const ISR_TOK: u16 = 0x0004;
const ISR_TER: u16 = 0x0008;
const ISR_RXOVW: u16 = 0x0010;
const ISR_PUNJ: u16 = 0x0020;
const ISR_FOVW: u16 = 0x0040;
const ISR_CNT: u16 = 0x8000;

// Buffer Sizes
const RX_BUFFER_SIZE: usize = 16384;
const TX_BUFFER_SIZE: usize = 4096;
const NUM_TX_BUFFERS: usize = 4;

// RTL8139 Device
pub struct Rtl8139Device {
    pub io_base: u16,
    pub present: bool,
    pub mac: [u8; ETH_ALEN],
    pub rx_buffer: [u8; RX_BUFFER_SIZE],
    pub tx_buffers: [[u8; TX_BUFFER_SIZE]; NUM_TX_BUFFERS],
    pub tx_dirty: [bool; NUM_TX_BUFFERS],
}

impl Rtl8139Device {
    pub const fn new() -> Self {
        Rtl8139Device {
            io_base: 0,
            present: false,
            mac: [0; ETH_ALEN],
            rx_buffer: [0; RX_BUFFER_SIZE],
            tx_buffers: [[0; TX_BUFFER_SIZE]; NUM_TX_BUFFERS],
            tx_dirty: [false; NUM_TX_BUFFERS],
        }
    }

    unsafe fn read8(&self, port: u16) -> u8 {
        let ptr = (self.io_base + port) as *mut u8;
        ptr.read_volatile()
    }

    unsafe fn read16(&self, port: u16) -> u16 {
        let ptr = (self.io_base + port) as *mut u16;
        ptr.read_volatile()
    }

    unsafe fn read32(&self, port: u16) -> u32 {
        let ptr = (self.io_base + port) as *mut u32;
        ptr.read_volatile()
    }

    unsafe fn write8(&self, port: u16, value: u8) {
        let ptr = (self.io_base + port) as *mut u8;
        ptr.write_volatile(value);
    }

    unsafe fn write16(&self, port: u16, value: u16) {
        let ptr = (self.io_base + port) as *mut u16;
        ptr.write_volatile(value);
    }

    unsafe fn write32(&self, port: u16, value: u32) {
        let ptr = (self.io_base + port) as *mut u32;
        ptr.write_volatile(value);
    }

    pub unsafe fn init(&mut self, io_base: u16) -> bool {
        self.io_base = io_base;

        // Reset the card
        self.write8(RTL8139_CMD, CMD_RESET);

        // Wait for reset to complete
        for _ in 0..100 {
            let cmd = self.read8(RTL8139_CMD);
            if (cmd & CMD_RESET) == 0 {
                break;
            }
        }

        // Check if reset worked
        let cmd = self.read8(RTL8139_CMD);
        if (cmd & CMD_RESET) != 0 {
            return false;
        }

        // Read MAC address
        self.mac[0] = self.read8(RTL8139_MAC0);
        self.mac[1] = self.read8(RTL8139_MAC1);
        self.mac[2] = self.read8(RTL8139_MAC2);
        self.mac[3] = self.read8(RTL8139_MAC3);
        self.mac[4] = self.read8(RTL8139_MAC4);
        let mac5 = self.read8(RTL8139_MAC0 + 4);

        // Check for valid MAC (not all zeros or all 0xFF)
        if self.mac == [0; 6] || self.mac == [0xFF; 6] {
            return false;
        }

        // Set RX buffer
        let rx_phys = self.rx_buffer.as_ptr() as u32;
        self.write32(RTL8139_RXBUF, rx_phys);

        // Configure RX - accept broadcast and our MAC
        let rcr = RCR_ACCEPT_MAC | 0x10000; // AB + AM + WRAP
        self.write32(RTL8139_RCR, rcr);

        // Configure TX
        self.write32(RTL8139_TCR, 0x03000100); // Enable TX

        // Enable RX and TX
        self.write8(RTL8139_CMD, CMD_RX_EN | CMD_TX_EN);

        self.present = true;
        true
    }

    pub unsafe fn receive(&mut self) -> Option<(&[u8], EthHeader)> {
        if !self.present {
            return None;
        }

        // Read ISR to check for RX
        let isr = self.read16(RTL8139_ISR);
        if (isr & ISR_ROK) == 0 {
            return None;
        }

        // Clear RX interrupt
        self.write16(RTL8139_ISR, ISR_ROK);

        // Get RX buffer offset (lower 14 bits of CAPR)
        // In real implementation, need to track head/tail pointers
        // Simplified: just look for packet at fixed offset

        None
    }

    pub unsafe fn send(&mut self, data: &[u8]) -> bool {
        if !self.present {
            return false;
        }

        let len = data.len().min(TX_BUFFER_SIZE - 4);

        // Find a free TX buffer
        let mut tx_idx = 0;
        for i in 0..NUM_TX_BUFFERS {
            if !self.tx_dirty[i] {
                tx_idx = i;
                break;
            }
        }

        // Copy data to TX buffer (with length prefix)
        let buf = &mut self.tx_buffers[tx_idx];
        buf[0] = (len & 0xFF) as u8;
        buf[1] = ((len >> 8) & 0xFF) as u8;
        buf[2] = ((len >> 16) & 0xFF) as u8;
        buf[3] = ((len >> 24) & 0xFF) as u8;
        buf[4..len + 4].copy_from_slice(&data[..len]);

        // Set TX address (physical address)
        let tx_addr = self.tx_buffers[tx_idx].as_ptr() as u32;
        let tx_reg = RTL8139_TX0 + (tx_idx as u16 * 4);
        self.write32(tx_reg, tx_addr);

        // Mark as dirty
        self.tx_dirty[tx_idx] = true;

        // Wait for TX to complete (polling)
        for _ in 0..1000 {
            let cmd = self.read8(RTL8139_CMD);
            if (cmd & 0x04) == 0 {
                // TX complete
                self.tx_dirty[tx_idx] = false;
                return true;
            }
        }

        false
    }

    pub fn get_mac(&self) -> [u8; ETH_ALEN] {
        self.mac
    }

    pub fn is_present(&self) -> bool {
        self.present
    }
}

// Global RTL8139 instance
pub static RTL8139: Mutex<Rtl8139Device> = Mutex::new(Rtl8139Device::new());

pub unsafe fn rtl8139_init(io_base: u16) -> bool {
    RTL8139.lock().init(io_base)
}

pub fn rtl8139_present() -> bool {
    RTL8139.lock().is_present()
}

pub fn rtl8139_get_mac() -> [u8; ETH_ALEN] {
    RTL8139.lock().get_mac()
}

pub fn rtl8139_send(data: &[u8]) -> bool {
    unsafe { RTL8139.lock().send(data) }
}
