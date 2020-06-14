use crate::prim::{FileAllocationTable32, FileAllocationTable32Result};
use crate::read_buffer::*;
use crate::FATGeometry;

pub(crate) struct ClusterWalker<'a> {
    buffer: ReadBuffer<'a>,
    cluster_index: u32,
    cluster_sector_index: u8,
    geo: FATGeometry,
}

impl<'a> ClusterWalker<'a> {
    pub fn open(buffer: ReadBuffer<'a>, cluster_index: u32, geo: FATGeometry) -> Option<Self> {
        let mut result = Self {
            buffer,
            cluster_index,
            cluster_sector_index: 0,
            geo,
        };

        result.ensure_sector();

        Some(result)
    }

    pub fn current_sector(&self) -> &[u8] {
        self.buffer
            .get_loaded_sector(self.absolute_sector_index())
            .unwrap_or_else(|| unreachable!())
    }

    pub fn next_sector(&mut self) -> bool {
        match self.cluster_sector_index + 1 {
            n if n == self.geo.cluster_size_sectors => false,
            n => {
                self.cluster_sector_index = n;
                self.ensure_sector();
                true
            }
        }
    }

    pub fn next_cluster(mut self) -> Option<Self> {
        let fat_byte_offset = u64::from(self.cluster_index) * 4;

        let fat_sector =
            self.geo.first_fat_sector + (fat_byte_offset / u64::from(self.geo.sector_size_bytes));

        // Sector size bytes has a maximum value of 4096 so 'as' is safe here
        let ent_offset = (fat_byte_offset % u64::from(self.geo.sector_size_bytes)) as u32;

        let fat_sector_data = self.buffer.get_sector(fat_sector);

        match FileAllocationTable32::from(fat_sector_data).get_entry(ent_offset) {
            FileAllocationTable32Result::NextClusterIndex(next_cluster_index) => {
                self.cluster_index = next_cluster_index;
                self.ensure_sector();
                Some(self)
            }
            FileAllocationTable32Result::EndOfChain => None,
            FileAllocationTable32Result::BadCluster => unimplemented!(),
        }
    }

    fn absolute_sector_index(&self) -> u64 {
        let absolute_start_sector_index = u64::from(self.cluster_index - 2)
            * u64::from(self.geo.cluster_size_sectors)
            + self.geo.first_data_sector;

        let absolute_sector_index =
            absolute_start_sector_index + u64::from(self.cluster_sector_index);

        absolute_sector_index
    }

    fn ensure_sector(&mut self) {
        // TODO: this should be fallible
        self.buffer.ensure_sector(self.absolute_sector_index());
    }
}
