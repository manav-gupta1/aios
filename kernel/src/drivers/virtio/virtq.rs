pub const REG_DEVICE_FEATURES: u16 = 0x00;
pub const REG_GUEST_FEATURES: u16 = 0x04;
pub const REG_QUEUE_ADDRESS: u16 = 0x08;
pub const REG_QUEUE_SIZE: u16 = 0x0C;
pub const REG_QUEUE_SELECT: u16 = 0x0E;
pub const REG_QUEUE_NOTIFY: u16 = 0x10;
pub const REG_DEVICE_STATUS: u16 = 0x12;
pub const REG_ISR_STATUS: u16 = 0x13;
pub const REG_CONFIG: u16 = 0x14;

pub const STATUS_ACKNOWLEDGE: u8 = 1;
pub const STATUS_DRIVER: u8 = 2;
pub const STATUS_DRIVER_OK: u8 = 4;
pub const STATUS_FEATURES_OK: u8 = 8;
pub const STATUS_FAILED: u8 = 128;

pub const VIRTQ_DESC_F_NEXT: u16 = 1;
pub const VIRTQ_DESC_F_WRITE: u16 = 2;

#[repr(C, align(16))]
pub struct VirtqDesc {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

#[repr(C)]
pub struct VirtqAvail {
    pub flags: u16,
    pub idx: u16,
    pub ring: [u16; 256],
}

#[repr(C)]
pub struct VirtqUsedElem {
    pub id: u32,
    pub len: u32,
}

#[repr(C, align(4096))]
pub struct VirtqUsed {
    pub flags: u16,
    pub idx: u16,
    pub ring: [VirtqUsedElem; 256],
}
