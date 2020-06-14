use core::convert::{AsRef, TryInto};
use core::ops::Range;

mod cluster_walker;
pub(crate) use cluster_walker::*;

mod read_buffer;
pub(crate) use read_buffer::*;

pub(crate) type ByteRange = Range<usize>;

pub(crate) trait DataStructure {
    fn range(&self, range: ByteRange) -> &[u8];

    fn u8(&self, range: ByteRange) -> u8 {
        self.range(range)[0]
    }

    fn u16(&self, range: ByteRange) -> u16 {
        let bytes = self.range(range);
        u16::from_le_bytes(bytes.try_into().unwrap())
    }

    fn u32(&self, range: ByteRange) -> u32 {
        let bytes = self.range(range);
        u32::from_le_bytes(bytes.try_into().unwrap())
    }
}

impl<T> DataStructure for T
where
    T: AsRef<[u8]>,
{
    fn range(&self, range: ByteRange) -> &[u8] {
        &self.as_ref()[range]
    }
}
