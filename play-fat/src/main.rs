#![allow(dead_code)]

use std::fs::File;
use std::io::Result;

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
                self.file.seek(SeekFrom::Start(offset));
                self.file.read_exact(dest);
            }
        }
    }
}

pub mod fat {
    use super::block_device::BlockDevice;
    use prim::*;

    pub mod prim {
        use std::convert::TryInto;

        type Range = std::ops::Range<usize>;

        pub const BIOS_PARAMETER_BLOCK_SIZE: usize = 512;

        pub struct CommonBiosParameterBlock;

        impl CommonBiosParameterBlock {
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

            pub fn read_oem(data: &[u8]) -> &[u8] {
                &data[Self::RANGE_OEM]
            }

            pub fn read_sectors_per_fat_16(data: &[u8]) -> u16 {
                let bytes = &data[Self::RANGE_SECTORS_PER_FAT_16];
                u16::from_le_bytes(bytes.try_into().unwrap())
            }

            pub fn read_total_sectors(data: &[u8]) -> u32 {
                match Self::read_total_sectors_16(data) {
                    0 => Self::read_total_sectors_32(data),
                    n => n as u32,
                }
            }

            pub fn read_total_sectors_16(data: &[u8]) -> u16 {
                let bytes = &data[Self::RANGE_TOTAL_SECTORS_16];
                u16::from_le_bytes(bytes.try_into().unwrap())
            }

            pub fn read_total_sectors_32(data: &[u8]) -> u32 {
                let bytes = &data[Self::RANGE_TOTAL_SECTORS_32];
                u32::from_le_bytes(bytes.try_into().unwrap())
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

        pub struct ExtendedFat32BiosParameterBlock;

        impl ExtendedFat32BiosParameterBlock {
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

            pub fn read_sectors_per_fat_32(data: &[u8]) -> u32 {
                let bytes = &data[Self::RANGE_SECTORS_PER_FAT_32];
                u32::from_le_bytes(bytes.try_into().unwrap())
            }
        }

        pub const FAT_DIR_ENTRY_SIZE: usize = 32;

        pub fn root_dir_count_of_sectors(root_entry_count: u16, bytes_per_sector: u16) -> u32 {
            let root_entry_count = root_entry_count as u32;
            let bytes_per_sector = bytes_per_sector as u32;
            let root_entry_bytes = root_entry_count * (FAT_DIR_ENTRY_SIZE as u32);

            (root_entry_bytes + (bytes_per_sector - 1)) / bytes_per_sector
        }

        pub fn sectors_per_fat(data: &[u8]) -> u32 {
            match CommonBiosParameterBlock::read_sectors_per_fat_16(data) {
                0 => ExtendedFat32BiosParameterBlock::read_sectors_per_fat_32(data),
                n => n as u32,
            }
        }

        pub fn data_region_count_of_sectors(
            fat_size: u32,
            total_sectors: u32,
            reserved_sector_count: u16,
            num_fats: u16,
            root_dir_sectors: u32,
        ) -> u32 {
            let reserved_sector_count = reserved_sector_count as u32;

            let num_fats = num_fats as u32;

            total_sectors - (reserved_sector_count + (num_fats * fat_size) + root_dir_sectors)
        }

        pub fn count_of_clusters(data_sectors: u32, sectors_per_cluster: u32) -> u32 {
            data_sectors / sectors_per_cluster
        }
    }

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

            println!(
                "OEM: {}",
                str::from_utf8(CommonBiosParameterBlock::read_oem(read_buffer.as_slice())).unwrap()
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
