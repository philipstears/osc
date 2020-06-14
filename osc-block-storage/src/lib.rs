pub trait BlockDevice {
    fn block_size(&self) -> u16;
    fn read_blocks(&mut self, start_block: u64, destination: &mut [u8]) -> u64;
}

pub mod virt {
    use super::*;
    use std::{
        cmp,
        fs::File,
        io::{Read, Seek, SeekFrom},
    };

    pub struct FileBlockDevice {
        file: File,
        offset: u64,
        len: u64,
    }

    impl FileBlockDevice {
        pub fn new(mut file: File, offset: u64) -> Self {
            let len = file.seek(SeekFrom::End(0)).unwrap();
            Self { file, offset, len }
        }
    }

    impl BlockDevice for FileBlockDevice {
        fn block_size(&self) -> u16 {
            512
        }

        fn read_blocks(&mut self, start_block: u64, dest: &mut [u8]) -> u64 {
            let block_size = self.block_size() as u64;

            if dest.is_empty() {
                panic!("The destination must be at least one block in size");
            }

            if dest.len() % (block_size as usize) > 0 {
                panic!("The destination must be a multiple of the block size");
            }

            let offset = self.offset + (start_block * block_size);
            self.file.seek(SeekFrom::Start(offset)).unwrap();

            let available_bytes = self.len - offset;
            let available_blocks = available_bytes / block_size;

            let dest_blocks = dest.len() as u64 / block_size;

            let read_blocks = cmp::min(available_blocks, dest_blocks);
            let read_bytes = read_blocks * block_size;

            let dest = &mut dest[0..(read_bytes as usize)];

            self.file.read_exact(dest).unwrap();

            read_blocks
        }
    }
}
