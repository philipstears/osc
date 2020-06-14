use core::{cell::RefCell, ops::Range};
use osc_block_storage::BlockDevice;
use std::rc::Rc;

pub(crate) struct ReadBuffer<'a> {
    device: Rc<RefCell<Box<dyn BlockDevice>>>,
    buffer: &'a mut [u8],
    sector_size_bytes: u16,
    loaded_sectors: Option<Range<u64>>,
}

impl<'a> ReadBuffer<'a> {
    pub fn new(
        device: Rc<RefCell<Box<dyn BlockDevice>>>,
        buffer: &'a mut [u8],
        sector_size_bytes: u16,
    ) -> Self {
        Self {
            device,
            buffer,
            sector_size_bytes,
            loaded_sectors: None,
        }
    }

    pub fn get_sector(&mut self, sector_index: u64) -> &[u8] {
        let sector_range = self.ensure_sector_prime(sector_index);
        &self.buffer[sector_range]
    }

    pub fn get_loaded_sector(&self, sector_index: u64) -> Option<&[u8]> {
        match self.loaded_sectors {
            Some(ref loaded_sectors) if loaded_sectors.contains(&sector_index) => {
                let sector_range = self.sector_range(loaded_sectors, sector_index);
                return Some(&self.buffer[sector_range]);
            }
            Some(_) | None => {
                return None;
            }
        }
    }

    pub fn ensure_sector(&mut self, sector_index: u64) {
        self.ensure_sector_prime(sector_index);
    }

    fn ensure_sector_prime(&mut self, sector_index: u64) -> Range<usize> {
        match self.loaded_sectors {
            Some(ref loaded_sectors) if loaded_sectors.contains(&sector_index) => {
                return self.sector_range(loaded_sectors, sector_index);
            }
            Some(_) | None => {
                return self.read_block_for_sector(sector_index);
            }
        }
    }

    fn sector_range(&self, loaded_sectors: &Range<u64>, sector_index: u64) -> Range<usize> {
        // NOTE: this could technically truncate on a 32-bit system, but in practice it
        // won't because the buffer size can't be big enough that a relative sector
        // index can be big enough to do that
        let relative_sector_index = (sector_index - loaded_sectors.start) as usize;

        let sector_size_bytes = usize::from(self.sector_size_bytes);
        let byte_start = relative_sector_index * sector_size_bytes;
        let byte_end = byte_start + sector_size_bytes;

        byte_start..byte_end
    }

    fn read_block_for_sector(&mut self, desired_sector_index: u64) -> Range<usize> {
        let mut device = self.device.borrow_mut();

        let sector_size_bytes = u64::from(self.sector_size_bytes);
        let block_size_bytes = u64::from(device.block_size());

        // Read the block containing the desired sector
        let block_index = (desired_sector_index * sector_size_bytes) / block_size_bytes;
        let blocks_read = device.read_blocks(block_index, self.buffer);
        let sectors_read = (blocks_read * block_size_bytes) / sector_size_bytes;

        // TODO: this means the sector doesn't exist on disk, we need
        // an error handling strategy for things like that
        assert_ne!(0, sectors_read);

        let first_sector = (block_index * block_size_bytes) / sector_size_bytes;
        let last_sector = first_sector + sectors_read;

        let loaded_sectors = first_sector..last_sector;
        let sector_range = self.sector_range(&loaded_sectors, desired_sector_index);

        self.loaded_sectors = Some(loaded_sectors);
        sector_range
    }
}
