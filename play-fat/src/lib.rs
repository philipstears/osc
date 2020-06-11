#![allow(dead_code)]

// TODO: use https://docs.rs/num-integer? it is probably slower though because
// it is more general
pub mod math {
    pub trait DivCeiling {
        type Value;

        fn div_ceiling(self, divisor: Self::Value) -> Self::Value;
    }

    impl DivCeiling for u32 {
        type Value = Self;

        #[inline]
        fn div_ceiling(self, divisor: Self::Value) -> Self::Value {
            (self + (divisor - 1)) / divisor
        }
    }
}

pub mod block_device {
    pub trait BlockDevice {
        fn block_size(&self) -> u16;
        fn read_blocks(&mut self, start_block: u64, destination: &mut [u8]);
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

            fn read_blocks(&mut self, start_block: u64, dest: &mut [u8]) {
                let block_size = self.block_size() as u64;

                if dest.is_empty() {
                    panic!("The destination must be at least one block in size");
                }

                if dest.len() % (block_size as usize) > 0 {
                    panic!("The destination must be a multiple of the block size");
                }

                let offset = self.offset + (start_block * block_size);
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
            const RANGE_ROOT_CLUSTER: Range = 44..48;
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

            pub fn root_cluster(&self) -> u32 {
                self.u32(Self::RANGE_ROOT_CLUSTER)
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

        pub struct DirectoryEntriesCluster<'a>(&'a [u8]);

        impl<'a> DirectoryEntriesCluster<'a> {
            pub fn occupied_entries(&self) -> DirectoryEntriesIterator<'a> {
                DirectoryEntriesIterator(self.0.chunks_exact(DirectoryEntry::SIZE))
            }
        }

        impl<'a> From<&'a [u8]> for DirectoryEntriesCluster<'a> {
            fn from(other: &'a [u8]) -> Self {
                Self(other)
            }
        }

        pub struct DirectoryEntriesIterator<'a>(std::slice::ChunksExact<'a, u8>);

        impl<'a> Iterator for DirectoryEntriesIterator<'a> {
            type Item = DirectoryEntry<'a>;

            fn next(&mut self) -> Option<Self::Item> {
                loop {
                    let entry = self.0.next()?;

                    match entry[0] {
                        0x00 => {
                            return None;
                        }
                        0xE5 => {
                            continue;
                        }
                        _ => {
                            return Some(entry.into());
                        }
                    }
                }
            }
        }

        pub enum DirectoryEntry<'a> {
            Standard(StandardDirectoryEntry<'a>),
            LongFileName(LongFileNameEntry<'a>),
        }

        impl<'a> DirectoryEntry<'a> {
            pub const SIZE: usize = 32;
        }

        impl<'a> From<&'a [u8]> for DirectoryEntry<'a> {
            fn from(other: &'a [u8]) -> Self {
                if other[11] == 0x0F {
                    Self::LongFileName(LongFileNameEntry(other))
                } else {
                    Self::Standard(StandardDirectoryEntry(other))
                }
            }
        }

        pub struct StandardDirectoryEntry<'a>(&'a [u8]);

        impl<'a> StandardDirectoryEntry<'a> {
            const RANGE_NAME: Range = 0..8;
            const RANGE_EXT: Range = 8..11;
            const RANGE_ATTR: Range = 11..12;
            const RANGE_RESERVED_WINNT: Range = 12..13;
            const RANGE_CREATION_TIME_DECISECS: Range = 13..14;
            const RANGE_CREATION_TIME: Range = 14..16;
            const RANGE_CREATION_DATE: Range = 16..18;
            const RANGE_ACCESS_DATE: Range = 18..20;
            const RANGE_FIRST_CLUSTER_HIGH: Range = 20..22;
            const RANGE_MOD_TIME: Range = 22..24;
            const RANGE_MOD_DATE: Range = 24..26;
            const RANGE_FIRST_CLUSTER_LOW: Range = 26..28;
            const RANGE_SIZE: Range = 28..32;

            pub fn name(&self) -> &[u8] {
                self.range(Self::RANGE_NAME)
            }

            pub fn ext(&self) -> &[u8] {
                self.range(Self::RANGE_EXT)
            }

            pub fn size(&self) -> u32 {
                self.u32(Self::RANGE_SIZE)
            }

            pub fn is_read_only(&self) -> bool {
                self.u8(Self::RANGE_ATTR) & 0x01 != 0
            }

            pub fn is_hidden(&self) -> bool {
                self.u8(Self::RANGE_ATTR) & 0x02 != 0
            }

            pub fn is_system(&self) -> bool {
                self.u8(Self::RANGE_ATTR) & 0x04 != 0
            }

            pub fn is_volume_id(&self) -> bool {
                self.u8(Self::RANGE_ATTR) & 0x08 != 0
            }

            pub fn is_directory(&self) -> bool {
                self.u8(Self::RANGE_ATTR) & 0x10 != 0
            }

            pub fn is_archive(&self) -> bool {
                self.u8(Self::RANGE_ATTR) & 0x20 != 0
            }

            pub fn first_cluster_high(&self) -> u16 {
                self.u16(Self::RANGE_FIRST_CLUSTER_HIGH)
            }

            pub fn first_cluster_low(&self) -> u16 {
                self.u16(Self::RANGE_FIRST_CLUSTER_LOW)
            }

            pub fn first_cluster(&self) -> u32 {
                ((self.first_cluster_high() as u32) << 16) | (self.first_cluster_low() as u32)
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

        pub struct LongFileNameEntry<'a>(&'a [u8]);

        impl<'a> LongFileNameEntry<'a> {
            const RANGE_ORDER: Range = 0..1;
            const RANGE_PORTION1: Range = 1..11;
            const RANGE_ATTR: Range = 11..12;
            const RANGE_LONG_ENTRY_TYPE: Range = 12..13;
            const RANGE_CHECKSUM: Range = 13..14;
            const RANGE_PORTION2: Range = 14..26;
            const RANGE_ZERO: Range = 26..28;
            const RANGE_PORTION3: Range = 28..32;

            pub fn chars(&self) -> LongFileNameCharIterator {
                LongFileNameCharIterator::new(self)
            }

            fn portion1(&self) -> &[u8] {
                self.range(Self::RANGE_PORTION1)
            }

            fn portion2(&self) -> &[u8] {
                self.range(Self::RANGE_PORTION2)
            }

            fn portion3(&self) -> &[u8] {
                self.range(Self::RANGE_PORTION3)
            }

            fn range(&self, range: Range) -> &[u8] {
                &self.0[range]
            }
        }

        pub struct LongFileNameCharIterator<'a> {
            entry: &'a LongFileNameEntry<'a>,
            state: LongFileNameCharIteratorState<'a>,
        }

        impl<'a> LongFileNameCharIterator<'a> {
            fn new(entry: &'a LongFileNameEntry) -> Self {
                LongFileNameCharIterator {
                    entry,
                    state: LongFileNameCharIteratorState::Portion1(U16Iterator(
                        entry.portion1().iter(),
                    )),
                }
            }
        }

        impl<'a> Iterator for LongFileNameCharIterator<'a> {
            type Item = u16;

            fn next(&mut self) -> Option<Self::Item> {
                use LongFileNameCharIteratorState::*;

                loop {
                    match self.state {
                        Portion1(ref mut inner) => match inner.next() {
                            Some(0) => {
                                return None;
                            }
                            Some(v) => {
                                return Some(v);
                            }
                            None => {
                                self.state = Portion2(U16Iterator(self.entry.portion2().iter()));
                            }
                        },
                        Portion2(ref mut inner) => match inner.next() {
                            Some(0) => {
                                return None;
                            }
                            Some(v) => {
                                return Some(v);
                            }
                            None => {
                                self.state = Portion3(U16Iterator(self.entry.portion3().iter()));
                            }
                        },
                        Portion3(ref mut inner) => match inner.next() {
                            Some(0) => {
                                return None;
                            }
                            Some(v) => {
                                return Some(v);
                            }
                            None => {
                                return None;
                            }
                        },
                    }
                }
            }
        }

        enum LongFileNameCharIteratorState<'a> {
            Portion1(U16Iterator<'a>),
            Portion2(U16Iterator<'a>),
            Portion3(U16Iterator<'a>),
        }

        struct U16Iterator<'a>(std::slice::Iter<'a, u8>);

        impl<'a> Iterator for U16Iterator<'a> {
            type Item = u16;

            fn next(&mut self) -> Option<Self::Item> {
                match self.0.next() {
                    None => None,
                    Some(first_byte) => match self.0.next() {
                        None => panic!("Incomplete number of bytes"),
                        Some(second_byte) => {
                            Some((*second_byte as u16) << 8 | (*first_byte as u16))
                        }
                    },
                }
            }
        }

        pub fn root_dir_sector_count(root_entry_count: u32, bytes_per_sector: u32) -> u32 {
            let root_entry_bytes = root_entry_count * (DirectoryEntry::SIZE as u32);
            root_entry_bytes.div_ceiling(bytes_per_sector)
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
            sectors_per_cluster: u32,
            first_data_sector: u32,
        ) -> u32 {
            ((cluster - 2) * sectors_per_cluster) + first_data_sector
        }
    }

    #[derive(Debug, Copy, Clone)]
    pub enum Variant {
        Fat12,
        Fat16,
        Fat32,
    }

    impl Variant {
        pub fn from_cluster_count(cluster_count: u32) -> Self {
            if cluster_count < 4085 {
                Self::Fat12
            } else if cluster_count < 65525 {
                Self::Fat16
            } else {
                Self::Fat32
            }
        }
    }

    pub struct FATFileSystem {
        device: Box<dyn BlockDevice>,

        variant: Variant,
        bytes_per_sector: u32,
        sectors_per_cluster: u32,
        first_fat_sector: u32,
        first_data_sector: u32,
        //
        // TODO: Fat32 only
        root_cluster_start_sector: u32,
    }

    impl FATFileSystem {
        pub fn open(mut device: Box<dyn BlockDevice>) -> Self {
            use std::str;

            // Read the BPB
            let mut read_buffer = [0u8; 512];
            device.read_blocks(0, &mut read_buffer);

            let read_buffer_slice = &read_buffer[..];

            // Right, what version of FAT are we dealing with?
            let bpb: CommonBiosParameterBlock = read_buffer_slice.into();

            let bytes_per_sector = bpb.bytes_per_sector() as u32;
            let root_dir_sector_count =
                root_dir_sector_count(bpb.root_entry_count() as u32, bytes_per_sector);

            let sectors_per_fat = sectors_per_fat(read_buffer_slice);
            let sectors_per_cluster = bpb.sectors_per_cluster().into();
            let reserved_sectors = bpb.reserved_sector_count();

            let meta_sectors = meta_sector_count(
                reserved_sectors,
                sectors_per_fat,
                bpb.fat_count(),
                root_dir_sector_count,
            );

            let first_data_sector = meta_sectors;

            let data_sectors = bpb.total_sectors() - meta_sectors;

            let count_of_clusters = data_sectors / sectors_per_cluster;

            let variant = Variant::from_cluster_count(count_of_clusters);

            let root_cluster_start_sector = match variant {
                Variant::Fat12 | Variant::Fat16 => unimplemented!(),
                Variant::Fat32 => first_sector_of_cluster(
                    ExtendedFat32BiosParameterBlock::from(read_buffer_slice).root_cluster(),
                    sectors_per_cluster,
                    first_data_sector,
                ),
            };

            println!(
                "Variant: {:?}, OEM: {}",
                variant,
                str::from_utf8(bpb.oem()).unwrap()
            );

            Self {
                device,
                variant,
                sectors_per_cluster,
                bytes_per_sector,
                first_fat_sector: reserved_sectors.into(),
                first_data_sector,
                root_cluster_start_sector,
            }
        }

        pub fn cluster_bytes(&self) -> u32 {
            self.bytes_per_sector * self.sectors_per_cluster
        }

        pub fn ls_root<'a>(
            &mut self,
            cluster_buffer: &'a mut [u8],
        ) -> DirectoryEntriesIterator<'a> {
            self.device
                .read_blocks(self.root_cluster_start_sector as u64, cluster_buffer);
            let cluster_buffer: &[u8] = cluster_buffer;
            DirectoryEntriesCluster::from(cluster_buffer).occupied_entries()
        }

        pub fn ls<'a>(
            &mut self,
            directory_first_cluster: u32,
            cluster_buffer: &'a mut [u8],
        ) -> DirectoryEntriesIterator<'a> {
            let first_sector = first_sector_of_cluster(
                directory_first_cluster,
                self.sectors_per_cluster,
                self.first_data_sector,
            ) as u64;
            self.device.read_blocks(first_sector, cluster_buffer);
            let cluster_buffer: &[u8] = cluster_buffer;
            DirectoryEntriesCluster::from(cluster_buffer).occupied_entries()
        }

        pub fn read<'a>(&mut self, file_first_cluster: u32, cluster_buffer: &'a mut [u8]) {
            let first_sector = first_sector_of_cluster(
                file_first_cluster,
                self.sectors_per_cluster,
                self.first_data_sector,
            ) as u64;
            self.device.read_blocks(first_sector, cluster_buffer);
        }
    }
}
