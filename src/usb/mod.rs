// BerkeOS — usb/mod.rs
// USB core definitions and constants

pub mod ohci;
pub mod storage;

// USB Descriptor Types
pub const USB_DT_DEVICE: u8 = 0x01;
pub const USB_DT_CONFIG: u8 = 0x02;
pub const USB_DT_STRING: u8 = 0x03;
pub const USB_DT_INTERFACE: u8 = 0x04;
pub const USB_DT_ENDPOINT: u8 = 0x05;
pub const USB_DT_DEVICE_QUALIFIER: u8 = 0x06;
pub const USB_DT_OTHER_SPEED_CONFIG: u8 = 0x07;
pub const USB_DT_INTERFACE_POWER: u8 = 0x08;

// USB Request Types
pub const USB_REQ_GET_STATUS: u8 = 0x00;
pub const USB_REQ_CLEAR_FEATURE: u8 = 0x01;
pub const USB_REQ_SET_FEATURE: u8 = 0x03;
pub const USB_REQ_SET_ADDRESS: u8 = 0x05;
pub const USB_REQ_GET_DESCRIPTOR: u8 = 0x06;
pub const USB_REQ_SET_DESCRIPTOR: u8 = 0x07;
pub const USB_REQ_GET_CONFIGURATION: u8 = 0x08;
pub const USB_REQ_SET_CONFIGURATION: u8 = 0x09;
pub const USB_REQ_GET_INTERFACE: u8 = 0x0A;
pub const USB_REQ_SET_INTERFACE: u8 = 0x0B;
pub const USB_REQ_SYNCH_FRAME: u8 = 0x0C;

// USB Class Codes
pub const USB_CLASS_PER_INTERFACE: u8 = 0x00;
pub const USB_CLASS_AUDIO: u8 = 0x01;
pub const USB_CLASS_COMM: u8 = 0x02;
pub const USB_CLASS_HID: u8 = 0x03;
pub const USB_CLASS_PHYSICAL: u8 = 0x05;
pub const USB_CLASS_IMAGE: u8 = 0x06;
pub const USB_CLASS_PRINTER: u8 = 0x07;
pub const USB_CLASS_MASS_STORAGE: u8 = 0x08;
pub const USB_CLASS_HUB: u8 = 0x09;
pub const USB_CLASS_DATA: u8 = 0x0A;
pub const USB_CLASS_SMART_CARD: u8 = 0x0B;
pub const USB_CLASS_CONTENT_SECURITY: u8 = 0x0D;
pub const USB_CLASS_VIDEO: u8 = 0x0E;
pub const USB_CLASS_PERSONAL_HEALTHCARE: u8 = 0x0F;
pub const USB_CLASS_AUDIO_VIDEO: u8 = 0x10;
pub const USB_CLASS_BILLBOARD: u8 = 0x11;
pub const USB_CLASS_USB_TYPE_C_BRIDGE: u8 = 0x12;
pub const USB_CLASS_DIAGNOSTIC: u8 = 0xDC;
pub const USB_CLASS_WIRELESS_CONTROLLER: u8 = 0xE0;
pub const USB_CLASS_MISC: u8 = 0xEF;
pub const USB_CLASS_VENDOR_SPECIFIC: u8 = 0xFF;

// USB Descriptor Types for HID
pub const USB_DT_HID: u8 = 0x21;
pub const USB_DT_REPORT: u8 = 0x22;
pub const USB_DT_PHYSICAL: u8 = 0x23;

// USB Transfer Types
pub const USB_TRANSFER_TYPE_CONTROL: u8 = 0x00;
pub const USB_TRANSFER_TYPE_ISOCHRONOUS: u8 = 0x01;
pub const USB_TRANSFER_TYPE_BULK: u8 = 0x02;
pub const USB_TRANSFER_TYPE_INTERRUPT: u8 = 0x03;

// USB Directions
pub const USB_DIR_OUT: u8 = 0x00;
pub const USB_DIR_IN: u8 = 0x80;

// USB Endpoint Address
pub fn USB_ENDPOINT_IN(addr: u8) -> bool {
    (addr & 0x80) != 0
}
pub fn USB_ENDPOINT_OUT(addr: u8) -> bool {
    (addr & 0x80) == 0
}
pub fn USB_ENDPOINT_ADDR(num: u8) -> u8 {
    num & 0x0F
}

// USB Speed
pub const USB_SPEED_LOW: u8 = 0;
pub const USB_SPEED_FULL: u8 = 1;
pub const USB_SPEED_HIGH: u8 = 2;

// USB Mass Storage Class
pub const USB_SUBCLASS_SCSI: u8 = 0x06;
pub const USB_PROTOCOL_BOT: u8 = 0x50; // Bulk-Only Transport
pub const USB_PROTOCOL_CBI: u8 = 0x01; // Control/Bulk/Interrupt

// SCSI Commands
pub const SCSI_OP_INQUIRY: u8 = 0x12;
pub const SCSI_OP_READ_CAPACITY: u8 = 0x25;
pub const SCSI_OP_READ_10: u8 = 0x28;
pub const SCSI_OP_WRITE_10: u8 = 0x2A;
pub const SCSI_OP_TEST_UNIT_READY: u8 = 0x00;
pub const SCSI_OP_REQUEST_SENSE: u8 = 0x03;

// USB Device Descriptor
#[repr(C)]
pub struct DeviceDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub bcd_usb: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
    pub max_packet_size0: u8,
    pub id_vendor: u16,
    pub id_product: u16,
    pub bcd_device: u8,
    pub manufacturer: u8,
    pub product: u8,
    pub serial_number: u8,
    pub num_configurations: u8,
}

// USB Configuration Descriptor
#[repr(C)]
pub struct ConfigDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub total_length: u16,
    pub num_interfaces: u8,
    pub configuration_value: u8,
    pub configuration: u8,
    pub attributes: u8,
    pub max_power: u8,
}

// USB Interface Descriptor
#[repr(C)]
pub struct InterfaceDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub interface_number: u8,
    pub alternate_setting: u8,
    pub num_endpoints: u8,
    pub interface_class: u8,
    pub interface_subclass: u8,
    pub interface_protocol: u8,
    pub interface: u8,
}

// USB Endpoint Descriptor
#[repr(C)]
pub struct EndpointDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub endpoint_address: u8,
    pub attributes: u8,
    pub max_packet_size: u16,
    pub interval: u8,
}

// USB Setup Packet (for control transfers)
#[repr(C)]
pub struct SetupPacket {
    pub request_type: u8,
    pub request: u8,
    pub value: u16,
    pub index: u16,
    pub length: u16,
}

impl SetupPacket {
    pub fn new(request_type: u8, request: u8, value: u16, index: u16, length: u16) -> Self {
        SetupPacket {
            request_type,
            request,
            value,
            index,
            length,
        }
    }

    pub fn get_descriptor(_dev_addr: u8, dtype: u8, dindex: u8, len: u16) -> Self {
        SetupPacket {
            request_type: USB_DIR_IN,
            request: USB_REQ_GET_DESCRIPTOR,
            value: ((dtype as u16) << 8) | (dindex as u16),
            index: 0,
            length: len,
        }
    }

    pub fn set_address(addr: u8) -> Self {
        SetupPacket {
            request_type: 0,
            request: USB_REQ_SET_ADDRESS,
            value: addr as u16,
            index: 0,
            length: 0,
        }
    }

    pub fn set_configuration(config: u8) -> Self {
        SetupPacket {
            request_type: 0,
            request: USB_REQ_SET_CONFIGURATION,
            value: config as u16,
            index: 0,
            length: 0,
        }
    }
}

// USB Hub Descriptor
#[repr(C)]
pub struct HubDescriptor {
    pub length: u8,
    pub descriptor_type: u8,
    pub num_ports: u8,
    pub characteristics: u16,
    pub delay: u8,
    pub bitmap: u16,
}

// USB Port Status
#[repr(C)]
pub struct PortStatus {
    pub connect: bool,
    pub enable: bool,
    pub suspend: bool,
    pub over_current: bool,
    pub reset: bool,
    pub power: bool,
    pub low_speed: bool,
    pub high_speed: bool,
    pub test: bool,
    pub indicator: bool,
    pub port_change: u8,
    pub port_enable_change: bool,
    pub port_connect_change: bool,
}

impl PortStatus {
    pub fn from_u32(val: u32) -> Self {
        PortStatus {
            connect: (val & 0x0001) != 0,
            enable: (val & 0x0002) != 0,
            suspend: (val & 0x0004) != 0,
            over_current: (val & 0x0008) != 0,
            reset: (val & 0x0010) != 0,
            power: (val & 0x0100) != 0,
            low_speed: (val & 0x0200) != 0,
            high_speed: (val & 0x0400) != 0,
            test: (val & 0x0800) != 0,
            indicator: (val & 0x1000) != 0,
            port_change: ((val >> 16) & 0xFF) as u8,
            port_enable_change: (val & 0x0002) != 0,
            port_connect_change: (val & 0x0001) != 0,
        }
    }
}

// USB Controller State
#[derive(Copy, Clone, PartialEq)]
pub enum UsbState {
    Detached,
    Attached,
    Powered,
    Default,
    Address,
    Configured,
    Suspended,
}

// USB Device
#[derive(Copy, Clone)]
pub struct UsbDevice {
    pub address: u8,
    pub speed: u8,
    pub max_packet0: u8,
    pub config: u8,
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub state: UsbState,
}

impl UsbDevice {
    pub const fn new() -> Self {
        UsbDevice {
            address: 0,
            speed: USB_SPEED_FULL,
            max_packet0: 8,
            config: 0,
            class: 0,
            subclass: 0,
            protocol: 0,
            vendor_id: 0,
            product_id: 0,
            state: UsbState::Detached,
        }
    }
}

// USB Controller Status
#[derive(Copy, Clone)]
pub struct UsbController {
    pub present: bool,
    pub ohci_base: u32,
    pub num_ports: u8,
    pub devices_connected: u8,
}

impl UsbController {
    pub const fn new() -> Self {
        UsbController {
            present: false,
            ohci_base: 0,
            num_ports: 0,
            devices_connected: 0,
        }
    }
}
