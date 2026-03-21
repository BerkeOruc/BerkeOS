// BerkeOS — usb/storage.rs
// USB Mass Storage Class Driver (Bulk-Only Transport)

use crate::usb::{
    ConfigDescriptor, DeviceDescriptor, EndpointDescriptor, InterfaceDescriptor, SetupPacket,
    UsbDevice, SCSI_OP_INQUIRY, SCSI_OP_READ_10, SCSI_OP_READ_CAPACITY, SCSI_OP_REQUEST_SENSE,
    SCSI_OP_TEST_UNIT_READY, SCSI_OP_WRITE_10, USB_CLASS_MASS_STORAGE, USB_DIR_IN, USB_DIR_OUT,
    USB_PROTOCOL_BOT, USB_REQ_GET_DESCRIPTOR, USB_REQ_SET_CONFIGURATION,
};
use spin::Mutex;

// BOT (Bulk-Only Transport) Constants
pub const BOT_CBW_SIGNATURE: u32 = 0x43425355; // "CBWS"
pub const BOT_CSW_SIGNATURE: u32 = 0x53425355; // "CSWS"

// BOT Command Block Wrapper
#[repr(C)]
pub struct BotCbw {
    pub signature: u32,
    pub tag: u32,
    pub data_transfer_length: u32,
    pub flags: u8,
    pub lun: u8,
    pub cb_length: u8,
    pub cb: [u8; 16],
}

impl BotCbw {
    pub fn new() -> Self {
        BotCbw {
            signature: BOT_CBW_SIGNATURE,
            tag: 0,
            data_transfer_length: 0,
            flags: 0,
            lun: 0,
            cb_length: 0,
            cb: [0; 16],
        }
    }

    pub fn setup_read(&mut self, lba: u32, count: u16) {
        self.tag += 1;
        self.data_transfer_length = (count as u32) * 512;
        self.flags = USB_DIR_IN;
        self.lun = 0;
        self.cb_length = 10;
        self.cb[0] = SCSI_OP_READ_10;
        self.cb[1] = 0;
        self.cb[2] = ((lba >> 24) & 0xFF) as u8;
        self.cb[3] = ((lba >> 16) & 0xFF) as u8;
        self.cb[4] = ((lba >> 8) & 0xFF) as u8;
        self.cb[5] = (lba & 0xFF) as u8;
        self.cb[6] = 0;
        self.cb[7] = ((count >> 8) & 0xFF) as u8;
        self.cb[8] = (count & 0xFF) as u8;
        self.cb[9] = 0;
    }

    pub fn setup_write(&mut self, lba: u32, count: u16) {
        self.tag += 1;
        self.data_transfer_length = (count as u32) * 512;
        self.flags = USB_DIR_OUT;
        self.lun = 0;
        self.cb_length = 10;
        self.cb[0] = SCSI_OP_WRITE_10;
        self.cb[1] = 0;
        self.cb[2] = ((lba >> 24) & 0xFF) as u8;
        self.cb[3] = ((lba >> 16) & 0xFF) as u8;
        self.cb[4] = ((lba >> 8) & 0xFF) as u8;
        self.cb[5] = (lba & 0xFF) as u8;
        self.cb[6] = 0;
        self.cb[7] = ((count >> 8) & 0xFF) as u8;
        self.cb[8] = (count & 0xFF) as u8;
        self.cb[9] = 0;
    }

    pub fn setup_inquiry(&mut self) {
        self.tag += 1;
        self.data_transfer_length = 36;
        self.flags = USB_DIR_IN;
        self.lun = 0;
        self.cb_length = 6;
        self.cb[0] = SCSI_OP_INQUIRY;
        self.cb[1] = 0;
        self.cb[2] = 0;
        self.cb[3] = 0;
        self.cb[4] = 36;
        self.cb[5] = 0;
    }

    pub fn setup_read_capacity(&mut self) {
        self.tag += 1;
        self.data_transfer_length = 8;
        self.flags = USB_DIR_IN;
        self.lun = 0;
        self.cb_length = 10;
        self.cb[0] = SCSI_OP_READ_CAPACITY;
        self.cb[1] = 0;
        self.cb[2] = 0;
        self.cb[3] = 0;
        self.cb[4] = 0;
        self.cb[5] = 0;
        self.cb[6] = 0;
        self.cb[7] = 0;
        self.cb[8] = 0;
        self.cb[9] = 0;
    }

    pub fn setup_test_unit_ready(&mut self) {
        self.tag += 1;
        self.data_transfer_length = 0;
        self.flags = USB_DIR_OUT;
        self.lun = 0;
        self.cb_length = 6;
        self.cb[0] = SCSI_OP_TEST_UNIT_READY;
        self.cb[1] = 0;
        self.cb[2] = 0;
        self.cb[3] = 0;
        self.cb[4] = 0;
        self.cb[5] = 0;
    }

    pub fn setup_request_sense(&mut self) {
        self.tag += 1;
        self.data_transfer_length = 18;
        self.flags = USB_DIR_IN;
        self.lun = 0;
        self.cb_length = 6;
        self.cb[0] = SCSI_OP_REQUEST_SENSE;
        self.cb[1] = 0;
        self.cb[2] = 0;
        self.cb[3] = 0;
        self.cb[4] = 18;
        self.cb[5] = 0;
    }
}

// BOT Command Status Wrapper
#[repr(C)]
pub struct BotCsw {
    pub signature: u32,
    pub tag: u32,
    pub data_residue: u32,
    pub status: u8,
}

impl BotCsw {
    pub fn new() -> Self {
        BotCsw {
            signature: BOT_CSW_SIGNATURE,
            tag: 0,
            data_residue: 0,
            status: 0,
        }
    }

    pub fn is_success(&self) -> bool {
        self.signature == BOT_CSW_SIGNATURE && self.status == 0
    }

    pub fn is_failed(&self) -> bool {
        self.signature == BOT_CSW_SIGNATURE && self.status == 1
    }

    pub fn is_phase_error(&self) -> bool {
        self.signature == BOT_CSW_SIGNATURE && self.status == 2
    }
}

// SCSI Inquiry Data
#[repr(C)]
pub struct ScsiInquiry {
    pub peripheral: u8,
    pub device_type: u8,
    pub removable: u8,
    pub version: u8,
    pub response_data_format: u8,
    pub additional_length: u8,
    pub vendor: [u8; 8],
    pub product: [u8; 16],
    pub revision: [u8; 4],
}

impl ScsiInquiry {
    pub fn new() -> Self {
        ScsiInquiry {
            peripheral: 0,
            device_type: 0,
            removable: 0,
            version: 0,
            response_data_format: 0,
            additional_length: 0,
            vendor: [0; 8],
            product: [0; 16],
            revision: [0; 4],
        }
    }

    pub fn is_removable(&self) -> bool {
        (self.removable & 0x80) != 0
    }
}

// SCSI Read Capacity Data
#[repr(C)]
pub struct ScsiReadCapacity {
    pub block_address: u32,
    pub block_length: u32,
}

impl ScsiReadCapacity {
    pub fn new() -> Self {
        ScsiReadCapacity {
            block_address: 0,
            block_length: 0,
        }
    }

    pub fn get_lba_count(&self) -> u32 {
        self.block_address
    }

    pub fn get_block_size(&self) -> u32 {
        self.block_length
    }
}

// USB Mass Storage Device
pub struct UsbStorageDevice {
    pub device: UsbDevice,
    pub bulk_in: u8,
    pub bulk_out: u8,
    pub capacity: u64,
    pub block_size: u32,
    pub present: bool,
}

impl Copy for UsbStorageDevice {}
impl Clone for UsbStorageDevice {
    fn clone(&self) -> Self {
        *self
    }
}

impl UsbStorageDevice {
    pub const fn new() -> Self {
        UsbStorageDevice {
            device: UsbDevice::new(),
            bulk_in: 0,
            bulk_out: 0,
            capacity: 0,
            block_size: 512,
            present: false,
        }
    }

    pub fn read(&mut self, lba: u32, count: u16, buf: &mut [u8]) -> bool {
        if !self.present {
            return false;
        }

        let expected = (count as usize) * self.block_size as usize;
        if buf.len() < expected {
            return false;
        }

        // BOT protocol would send CBW, data, CSW
        // For now, return false - needs actual USB transfer
        false
    }

    pub fn write(&mut self, lba: u32, count: u16, buf: &[u8]) -> bool {
        if !self.present {
            return false;
        }

        let expected = (count as usize) * self.block_size as usize;
        if buf.len() < expected {
            return false;
        }

        // BOT protocol would send CBW, data, CSW
        false
    }

    pub fn get_capacity(&self) -> u64 {
        self.capacity
    }

    pub fn test_ready(&mut self) -> bool {
        if !self.present {
            return false;
        }
        // Would send TEST_UNIT_READY command
        true
    }
}

// USB Storage Controller
pub static USB_STORAGE: Mutex<UsbStorageController> = Mutex::new(UsbStorageController::new());

pub struct UsbStorageController {
    pub present: bool,
    pub devices: [UsbStorageDevice; 4],
}

impl UsbStorageController {
    pub const fn new() -> Self {
        UsbStorageController {
            present: false,
            devices: [UsbStorageDevice::new(); 4],
        }
    }

    pub fn init(&mut self) -> bool {
        // Would scan USB bus for mass storage devices
        // For now, just mark as present with framework ready
        self.present = true;
        true
    }

    pub fn get_device(&mut self, index: usize) -> Option<&mut UsbStorageDevice> {
        if index < self.devices.len() {
            Some(&mut self.devices[index])
        } else {
            None
        }
    }

    pub fn first_present(&mut self) -> Option<&mut UsbStorageDevice> {
        for dev in &mut self.devices {
            if dev.present {
                return Some(dev);
            }
        }
        None
    }
}

pub fn usb_storage_init() -> bool {
    USB_STORAGE.lock().init()
}

pub fn usb_storage_read(lba: u32, count: u16, buf: &mut [u8]) -> bool {
    if let Some(dev) = USB_STORAGE.lock().first_present() {
        dev.read(lba, count, buf)
    } else {
        false
    }
}

pub fn usb_storage_write(lba: u32, count: u16, buf: &[u8]) -> bool {
    if let Some(dev) = USB_STORAGE.lock().first_present() {
        dev.write(lba, count, buf)
    } else {
        false
    }
}

pub fn usb_storage_get_capacity() -> u64 {
    if let Some(dev) = USB_STORAGE.lock().first_present() {
        dev.get_capacity()
    } else {
        0
    }
}

pub fn usb_storage_present() -> bool {
    USB_STORAGE.lock().present
}
