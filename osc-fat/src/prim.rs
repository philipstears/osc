use crate::math::DivCeiling;
use crate::util::*;
use crate::*;

pub const BIOS_PARAMETER_BLOCK_SIZE: usize = 512;

pub struct CommonBiosParameterBlock<'a>(&'a [u8]);

impl<'a> CommonBiosParameterBlock<'a> {
    pub const SIZE: usize = 36;

    const RANGE_JUMP: ByteRange = 0..3;
    const RANGE_OEM: ByteRange = 3..11;
    const RANGE_BYTES_PER_SECTOR: ByteRange = 11..13;
    const RANGE_SECTORS_PER_CLUSTER: ByteRange = 13..14;
    const RANGE_RESERVED_SECTOR_COUNT: ByteRange = 14..16;
    const RANGE_NUM_FATS: ByteRange = 16..17;
    // NOTE: zero for FAT32
    const RANGE_ROOT_ENTRY_COUNT: ByteRange = 17..19;
    const RANGE_TOTAL_SECTORS_16: ByteRange = 19..21;
    const RANGE_MEDIA: ByteRange = 21..22;
    // NOTE: zero for FAT32
    const RANGE_SECTORS_PER_FAT_16: ByteRange = 22..24;
    const RANGE_SECTORS_PER_TRACK: ByteRange = 24..26;
    const RANGE_NUM_HEADS: ByteRange = 26..28;
    const RANGE_HIDDEN_SECTORS: ByteRange = 28..32;
    const RANGE_TOTAL_SECTORS_32: ByteRange = 32..36;

    pub fn oem(&self) -> &[u8] {
        self.0.range(Self::RANGE_OEM)
    }

    pub fn bytes_per_sector(&self) -> u16 {
        self.0.u16(Self::RANGE_BYTES_PER_SECTOR)
    }

    pub fn sectors_per_cluster(&self) -> u8 {
        self.0.u8(Self::RANGE_SECTORS_PER_CLUSTER)
    }

    pub fn reserved_sector_count(&self) -> u16 {
        self.0.u16(Self::RANGE_RESERVED_SECTOR_COUNT)
    }

    pub fn fat_count(&self) -> u8 {
        self.0.u8(Self::RANGE_NUM_FATS)
    }

    pub fn root_entry_count(&self) -> u16 {
        self.0.u16(Self::RANGE_ROOT_ENTRY_COUNT)
    }

    pub fn sectors_per_fat_16(&self) -> u16 {
        self.0.u16(Self::RANGE_SECTORS_PER_FAT_16)
    }

    pub fn total_sectors_16(&self) -> u16 {
        self.0.u16(Self::RANGE_TOTAL_SECTORS_16)
    }

    pub fn total_sectors_32(&self) -> u32 {
        self.0.u32(Self::RANGE_TOTAL_SECTORS_32)
    }

    pub fn total_sectors(&self) -> u32 {
        match self.total_sectors_16() {
            0 => self.total_sectors_32(),
            n => n as u32,
        }
    }
}

impl<'a> From<&'a [u8]> for CommonBiosParameterBlock<'a> {
    fn from(other: &'a [u8]) -> Self {
        Self(other)
    }
}

pub struct ExtendedBiosParameterBlock;

impl ExtendedBiosParameterBlock {
    const RANGE_DRIVE_NUM: ByteRange = 36..37;
    const RANGE_RESV1: ByteRange = 37..38;
    const RANGE_BOOT_SIG: ByteRange = 38..39;
    const RANGE_VOL_ID: ByteRange = 39..43;
    const RANGE_VOL_LAB: ByteRange = 43..54;
    const RANGE_FS_TYPE: ByteRange = 54..62;
    const RANGE_BOOT: ByteRange = 62..510;
    const RANGE_SIG_WORD: ByteRange = 510..512;
}

pub struct ExtendedFat32BiosParameterBlock<'a>(&'a [u8]);

impl<'a> ExtendedFat32BiosParameterBlock<'a> {
    const RANGE_SECTORS_PER_FAT_32: ByteRange = 36..40;
    const RANGE_EXT_FLAGS: ByteRange = 40..42;
    const RANGE_FS_VER: ByteRange = 42..44;
    const RANGE_ROOT_CLUSTER: ByteRange = 44..48;
    const RANGE_FS_INFO_SECTOR: ByteRange = 48..50;
    const RANGE_BACKUP_BOOT_SECTOR: ByteRange = 50..52;
    const RANGE_RESERVED: ByteRange = 52..64;
    const RANGE_DRIVE_NUM: ByteRange = 64..65;
    const RANGE_RESERVED1: ByteRange = 65..66;
    const RANGE_BOOT_SIG: ByteRange = 66..67;
    const RANGE_VOL_ID: ByteRange = 67..71;
    const RANGE_VOL_LAB: ByteRange = 71..82;
    const RANGE_FS_TYPE: ByteRange = 82..90;
    const RANGE_BOOT: ByteRange = 90..510;
    const RANGE_SIG_WORD: ByteRange = 510..512;

    pub fn sectors_per_fat_32(&self) -> u32 {
        self.0.u32(Self::RANGE_SECTORS_PER_FAT_32)
    }

    pub fn root_cluster(&self) -> u32 {
        self.0.u32(Self::RANGE_ROOT_CLUSTER)
    }
}

impl<'a> From<&'a [u8]> for ExtendedFat32BiosParameterBlock<'a> {
    fn from(other: &'a [u8]) -> Self {
        Self(other)
    }
}

pub fn root_dir_sector_count(root_entry_count: u32, bytes_per_sector: u16) -> u32 {
    let root_entry_bytes = root_entry_count * (DirectoryEntry::SIZE as u32);
    root_entry_bytes.div_ceiling(u32::from(bytes_per_sector))
}

pub fn sectors_per_fat(data: &[u8]) -> u32 {
    match CommonBiosParameterBlock::from(data).sectors_per_fat_16() {
        0 => ExtendedFat32BiosParameterBlock::from(data).sectors_per_fat_32(),
        n => n as u32,
    }
}

pub fn meta_sector_count(
    reserved_sector_count: u16,
    sectors_per_fat: u32,
    fat_count: u8,
    root_dir_sectors: u32,
) -> u32 {
    let reserved_sector_count = reserved_sector_count as u32;
    let fat_count = fat_count as u32;
    reserved_sector_count + (sectors_per_fat * fat_count) + root_dir_sectors
}

pub fn data_region_sector_count(total_sectors: u32, meta_sector_count: u32) -> u32 {
    total_sectors - meta_sector_count
}

pub fn first_sector_of_cluster(
    cluster: u32,
    sectors_per_cluster: u8,
    first_data_sector: u32,
) -> u32 {
    ((cluster - 2) * u32::from(sectors_per_cluster)) + first_data_sector
}

pub struct FileAllocationTable32<'a>(&'a [u8]);

impl<'a> FileAllocationTable32<'a> {
    pub fn get_entry(&self, entry_byte_offset: u32) -> FileAllocationTable32Result {
        let start = entry_byte_offset as usize;
        let end = start + 4;

        // Need to mask off the top 4 bits, according to the spec
        // only 28-bits are used, and the others must be ignored
        // on read, and left alone on write
        (self.0.u32(start..end) & 0x0FFFFFFF).into()
    }
}

impl<'a> From<&'a [u8]> for FileAllocationTable32<'a> {
    fn from(other: &'a [u8]) -> Self {
        Self(other)
    }
}

pub enum FileAllocationTable32Result {
    NextClusterIndex(u32),
    BadCluster,
    EndOfChain,
}

impl From<u32> for FileAllocationTable32Result {
    fn from(other: u32) -> Self {
        if other >= 0x0FFFFFF8 {
            Self::EndOfChain
        } else if other == 0x0FFFFFF7 {
            Self::BadCluster
        } else {
            Self::NextClusterIndex(other)
        }
    }
}
