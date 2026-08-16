use x86_64::instructions::port::Port;

pub const SECTOR_SIZE: usize = 512;

const ATA_DATA_PORT: u16 = 0x1F0;
const ATA_SECTOR_COUNT_PORT: u16 = 0x1F2;
const ATA_LBA_LOW_PORT: u16 = 0x1F3;
const ATA_LBA_MID_PORT: u16 = 0x1F4;
const ATA_LBA_HIGH_PORT: u16 = 0x1F5;
const ATA_DRIVE_PORT: u16 = 0x1F6;
const ATA_COMMAND_PORT: u16 = 0x1F7;
const ATA_STATUS_PORT: u16 = 0x1F7;

const ATA_CMD_READ_SECTORS: u8 = 0x20;
const ATA_CMD_WRITE_SECTORS: u8 = 0x30;
const ATA_CMD_CACHE_FLUSH: u8 = 0xE7;

const STATUS_BSY: u8 = 0x80;
const STATUS_DRQ: u8 = 0x08;
const STATUS_ERR: u8 = 0x01;
const STATUS_DF: u8 = 0x20;

pub struct AtaPio;

impl AtaPio {
    pub fn is_available() -> bool {
        let mut status_port = Port::<u8>::new(ATA_STATUS_PORT);
        let status = unsafe { status_port.read() };
        status != 0xFF && status != 0x00
    }

    fn wait_bsy() -> Result<(), ()> {
        let mut status_port = Port::<u8>::new(ATA_STATUS_PORT);
        for _ in 0..500_000 {
            let status = unsafe { status_port.read() };
            if (status & STATUS_BSY) == 0 {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(())
    }

    fn wait_drq() -> Result<(), ()> {
        let mut status_port = Port::<u8>::new(ATA_STATUS_PORT);
        for _ in 0..500_000 {
            let status = unsafe { status_port.read() };
            if (status & (STATUS_ERR | STATUS_DF)) != 0 {
                return Err(());
            }
            if (status & STATUS_DRQ) != 0 {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(())
    }

    pub fn read_sector(lba: u32, buffer: &mut [u8; SECTOR_SIZE]) -> Result<(), ()> {
        Self::wait_bsy()?;

        let mut drive_port = Port::<u8>::new(ATA_DRIVE_PORT);
        let mut count_port = Port::<u8>::new(ATA_SECTOR_COUNT_PORT);
        let mut lba_low = Port::<u8>::new(ATA_LBA_LOW_PORT);
        let mut lba_mid = Port::<u8>::new(ATA_LBA_MID_PORT);
        let mut lba_high = Port::<u8>::new(ATA_LBA_HIGH_PORT);
        let mut cmd_port = Port::<u8>::new(ATA_COMMAND_PORT);
        let mut data_port = Port::<u16>::new(ATA_DATA_PORT);

        unsafe {
            drive_port.write(0xE0 | (((lba >> 24) & 0x0F) as u8));
            count_port.write(1);
            lba_low.write((lba & 0xFF) as u8);
            lba_mid.write(((lba >> 8) & 0xFF) as u8);
            lba_high.write(((lba >> 16) & 0xFF) as u8);
            cmd_port.write(ATA_CMD_READ_SECTORS);
        }

        Self::wait_drq()?;

        for i in 0..256 {
            let word = unsafe { data_port.read() };
            buffer[i * 2] = (word & 0xFF) as u8;
            buffer[i * 2 + 1] = ((word >> 8) & 0xFF) as u8;
        }

        Ok(())
    }

    pub fn write_sector(lba: u32, buffer: &[u8; SECTOR_SIZE]) -> Result<(), ()> {
        Self::wait_bsy()?;

        let mut drive_port = Port::<u8>::new(ATA_DRIVE_PORT);
        let mut count_port = Port::<u8>::new(ATA_SECTOR_COUNT_PORT);
        let mut lba_low = Port::<u8>::new(ATA_LBA_LOW_PORT);
        let mut lba_mid = Port::<u8>::new(ATA_LBA_MID_PORT);
        let mut lba_high = Port::<u8>::new(ATA_LBA_HIGH_PORT);
        let mut cmd_port = Port::<u8>::new(ATA_COMMAND_PORT);
        let mut data_port = Port::<u16>::new(ATA_DATA_PORT);

        unsafe {
            drive_port.write(0xE0 | (((lba >> 24) & 0x0F) as u8));
            count_port.write(1);
            lba_low.write((lba & 0xFF) as u8);
            lba_mid.write(((lba >> 8) & 0xFF) as u8);
            lba_high.write(((lba >> 16) & 0xFF) as u8);
            cmd_port.write(ATA_CMD_WRITE_SECTORS);
        }

        Self::wait_drq()?;

        for i in 0..256 {
            let word = (buffer[i * 2] as u16) | ((buffer[i * 2 + 1] as u16) << 8);
            unsafe {
                data_port.write(word);
            }
        }

        unsafe {
            cmd_port.write(ATA_CMD_CACHE_FLUSH);
        }
        Self::wait_bsy()?;

        Ok(())
    }
}
