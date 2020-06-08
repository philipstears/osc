#![allow(dead_code)]

use std::fs::File;
use std::io::Result;

pub mod math {
    pub trait DivCeiling {
        type Value;

        fn div_ceiling(self, divisor: Self::Value) -> Self::Value;
    }

    impl DivCeiling for u32 {
        type Value = Self;

        fn div_ceiling(self, divisor: Self::Value) -> Self::Value {
            (self + (divisor - 1)) / divisor
        }
    }
}

pub mod block_device {
    pub trait BlockDevice {
        fn block_size(&self) -> u16;
        fn read_block(&mut self, block: u64, destination: &mut [u8]);
    }

    pub mod virt {
        use super::*;
        use std::fs::File;
        use std::io::{Read, Seek, SeekFrom};

        pub struct FileBlockDevice {
            file: File,
            offset: u64,
        }

        impl FileBlockDevice {
            pub fn new(file: File, offset: u64) -> Self {
                Self { file, offset }
            }
        }

        impl BlockDevice for FileBlockDevice {
            fn block_size(&self) -> u16 {
                512
            }

            fn read_block(&mut self, block: u64, dest: &mut [u8]) {
                let block_size = self.block_size() as u64;

                if dest.is_empty() {
                    panic!("The destination must be at least one block in size");
                }

                if dest.len() % (block_size as usize) > 0 {
                    panic!("The destination must be a multiple of the block size");
                }

                let offset = self.offset + (block * block_size);
                self.file.seek(SeekFrom::Start(offset)).unwrap();
                self.file.read_exact(dest).unwrap();
            }
        }
    }
}

pub mod fat {
    use super::block_device::BlockDevice;
    use prim::*;

    pub mod prim {
        use super::super::math::DivCeiling;
        use std::convert::TryInto;

        type Range = std::ops::Range<usize>;

        pub const BIOS_PARAMETER_BLOCK_SIZE: usize = 512;

        pub struct CommonBiosParameterBlock<'a>(&'a [u8]);

        impl<'a> CommonBiosParameterBlock<'a> {
            pub const SIZE: usize = 36;

            const RANGE_JUMP: Range = 0..3;
            const RANGE_OEM: Range = 3..11;
            const RANGE_BYTES_PER_SECTOR: Range = 11..13;
            const RANGE_SECTORS_PER_CLUSTER: Range = 13..14;
            const RANGE_RESERVED_SECTOR_COUNT: Range = 14..16;
            const RANGE_NUM_FATS: Range = 16..17;
            // NOTE: zero for FAT32
            const RANGE_ROOT_ENTRY_COUNT: Range = 17..19;
            const RANGE_TOTAL_SECTORS_16: Range = 19..21;
            const RANGE_MEDIA: Range = 21..22;
            // NOTE: zero for FAT32
            const RANGE_SECTORS_PER_FAT_16: Range = 22..24;
            const RANGE_SECTORS_PER_TRACK: Range = 24..26;
            const RANGE_NUM_HEADS: Range = 26..28;
            const RANGE_HIDDEN_SECTORS: Range = 28..32;
            const RANGE_TOTAL_SECTORS_32: Range = 32..36;

            pub fn oem(&self) -> &[u8] {
                self.range(Self::RANGE_OEM)
            }

            pub fn bytes_per_sector(&self) -> u16 {
                self.u16(Self::RANGE_BYTES_PER_SECTOR)
            }

            pub fn sectors_per_cluster(&self) -> u8 {
                self.u8(Self::RANGE_SECTORS_PER_CLUSTER)
            }

            pub fn reserved_sector_count(&self) -> u16 {
                self.u16(Self::RANGE_RESERVED_SECTOR_COUNT)
            }

            pub fn fat_count(&self) -> u8 {
                self.u8(Self::RANGE_NUM_FATS)
            }

            pub fn root_entry_count(&self) -> u16 {
                self.u16(Self::RANGE_ROOT_ENTRY_COUNT)
            }

            pub fn sectors_per_fat_16(&self) -> u16 {
                self.u16(Self::RANGE_SECTORS_PER_FAT_16)
            }

            pub fn total_sectors_16(&self) -> u16 {
                self.u16(Self::RANGE_TOTAL_SECTORS_16)
            }

            pub fn total_sectors_32(&self) -> u32 {
                self.u32(Self::RANGE_TOTAL_SECTORS_32)
            }

            pub fn total_sectors(&self) -> u32 {
                match self.total_sectors_16() {
                    0 => self.total_sectors_32(),
                    n => n as u32,
                }
            }

            fn range(&self, range: Range) -> &[u8] {
                &self.0[range]
            }

            fn u8(&self, range: Range) -> u8 {
                self.0[range][0]
            }

            fn u16(&self, range: Range) -> u16 {
                let bytes = self.range(range);
                u16::from_le_bytes(bytes.try_into().unwrap())
            }

            fn u32(&self, range: Range) -> u32 {
                let bytes = self.range(range);
                u32::from_le_bytes(bytes.try_into().unwrap())
            }
        }

        impl<'a> From<&'a [u8]> for CommonBiosParameterBlock<'a> {
            fn from(other: &'a [u8]) -> Self {
                Self(other)
            }
        }

        pub struct ExtendedBiosParameterBlock;

        impl ExtendedBiosParameterBlock {
            const RANGE_DRIVE_NUM: Range = 36..37;
            const RANGE_RESV1: Range = 37..38;
            const RANGE_BOOT_SIG: Range = 38..39;
            const RANGE_VOL_ID: Range = 39..43;
            const RANGE_VOL_LAB: Range = 43..54;
            const RANGE_FS_TYPE: Range = 54..62;
            const RANGE_BOOT: Range = 62..510;
            const RANGE_SIG_WORD: Range = 510..512;
        }

        pub struct ExtendedFat32BiosParameterBlock<'a>(&'a [u8]);

        impl<'a> ExtendedFat32BiosParameterBlock<'a> {
            const RANGE_SECTORS_PER_FAT_32: Range = 36..40;
            const RANGE_EXT_FLAGS: Range = 40..42;
            const RANGE_FS_VER: Range = 42..44;
            const RANGE_ROOT_CLUSTERS: Range = 44..48;
            const RANGE_FS_INFO_SECTOR: Range = 48..50;
            const RANGE_BACKUP_BOOT_SECTOR: Range = 50..52;
            const RANGE_RESERVED: Range = 52..64;
            const RANGE_DRIVE_NUM: Range = 64..65;
            const RANGE_RESERVED1: Range = 65..66;
            const RANGE_BOOT_SIG: Range = 66..67;
            const RANGE_VOL_ID: Range = 67..71;
            const RANGE_VOL_LAB: Range = 71..82;
            const RANGE_FS_TYPE: Range = 82..90;
            const RANGE_BOOT: Range = 90..510;
            const RANGE_SIG_WORD: Range = 510..512;

            pub fn sectors_per_fat_32(&self) -> u32 {
                self.u32(Self::RANGE_SECTORS_PER_FAT_32)
            }

            fn range(&self, range: Range) -> &[u8] {
                &self.0[range]
            }

            fn u16(&self, range: Range) -> u16 {
                let bytes = self.range(range);
                u16::from_le_bytes(bytes.try_into().unwrap())
            }

            fn u32(&self, range: Range) -> u32 {
                let bytes = self.range(range);
                u32::from_le_bytes(bytes.try_into().unwrap())
            }
        }

        impl<'a> From<&'a [u8]> for ExtendedFat32BiosParameterBlock<'a> {
            fn from(other: &'a [u8]) -> Self {
                Self(other)
            }
        }

        pub const FAT_DIR_ENTRY_SIZE: usize = 32;

        pub fn root_dir_sector_count(root_entry_count: u16, bytes_per_sector: u16) -> u32 {
            let root_entry_bytes = (root_entry_count as u32) * (FAT_DIR_ENTRY_SIZE as u32);
            root_entry_bytes.div_ceiling(bytes_per_sector as u32)
        }

        pub fn sectors_per_fat(data: &[u8]) -> u32 {
            match CommonBiosParameterBlock::from(data).sectors_per_fat_16() {
                0 => ExtendedFat32BiosParameterBlock::from(data).sectors_per_fat_32(),
                n => n as u32,
            }
        }

        pub fn data_region_sector_count(
            total_sectors: u32,
            reserved_sector_count: u16,
            sectors_per_fat: u32,
            fat_count: u8,
            root_dir_sectors: u32,
        ) -> u32 {
            let reserved_sector_count = reserved_sector_count as u32;
            let fat_count = fat_count as u32;

            let meta_sectors =
                reserved_sector_count + (sectors_per_fat * fat_count) + root_dir_sectors;

            total_sectors - meta_sectors
        }
    }

    #[derive(Debug)]
    pub enum Type {
        Fat12,
        Fat16,
        Fat32,
    }

    pub fn determine_type(count_of_clusters: u32) -> Type {
        use Type::*;

        if count_of_clusters < 4085 {
            Fat12
        } else if count_of_clusters < 65525 {
            Fat16
        } else {
            Fat32
        }
    }

    pub struct FATFileSystem {
        device: Box<dyn BlockDevice>,
        read_buffer: Vec<u8>,
    }

    impl FATFileSystem {
        pub fn open(mut device: Box<dyn BlockDevice>) -> Self {
            use std::str;

            // Read the BPB
            let mut read_buffer = vec![0u8; device.block_size() as usize];
            device.read_block(0, read_buffer.as_mut_slice());

            // Right, what version of FAT are we dealing with?
            let bpb: CommonBiosParameterBlock = read_buffer.as_slice().into();

            let root_dir_sector_count =
                root_dir_sector_count(bpb.root_entry_count(), bpb.bytes_per_sector());

            let sectors_per_fat = sectors_per_fat(read_buffer.as_slice());

            let sectors_per_cluster = bpb.sectors_per_cluster() as u32;

            let data_sectors = data_region_sector_count(
                bpb.total_sectors(),
                bpb.reserved_sector_count(),
                sectors_per_fat,
                bpb.fat_count(),
                root_dir_sector_count,
            );

            let count_of_clusters = data_sectors / sectors_per_cluster;

            let fat_type = determine_type(count_of_clusters);

            println!(
                "Type: {:?}, OEM: {}",
                fat_type,
                str::from_utf8(bpb.oem()).unwrap()
            );

            Self {
                device,
                read_buffer,
            }
        }
    }
}

fn main() -> Result<()> {
    use block_device::virt::*;
    use fat::*;

    let image = "/home/stears/data/simon/nox-rust/target/x86-nox/release/nox-rust.img";
    let offset = 1048576;

    let file = File::open(image)?;
    let device = Box::new(FileBlockDevice::new(file, offset));

    let _fs = FATFileSystem::open(device);

    Ok(())
}
