use std::{fmt, path::Path};

use hash40::Hash40;

use crate::HASH_MASK;

pub mod table;

pub mod stream;

use binrw::{BinRead, BinWrite, VecArgs};

#[derive(Debug, Copy, Clone, PartialEq, Eq, BinRead, BinWrite)]
pub struct HashKey(u64);

impl HashKey {
    pub fn new(hash: Hash40, index: usize) -> Self {
        Self(hash.0 | (index as u64) << 40)
    }

    pub fn hash(self) -> Hash40 {
        Hash40(self.0 & HASH_MASK)
    }

    pub fn index(self) -> usize {
        (self.0 & !HASH_MASK) as usize >> 40
    }

    pub fn set_hash(&mut self, hash: Hash40) {
        self.0 = (self.0 & !HASH_MASK) | hash.0;
    }

    pub fn set_index(&mut self, index: usize) {
        self.0 = (self.0 & HASH_MASK) | (index as u64) << 40;
    }
}

impl fmt::Display for HashKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#08x} | {}", self.index(), self.hash())
    }
}

fn read_table<T: BinRead>(path: &Path, item_size: usize) -> binrw::BinResult<Vec<T>>
where
    <T as BinRead>::Args: Default,
{
    let bytes = std::fs::read(path)?;
    if bytes.len() % item_size != 0 {
        return Err(binrw::Error::Custom {
            pos: 0,
            err: Box::new(
                "Table alignment error: byte length of table is not a multiple of item_size",
            ),
        });
    }
    let count = bytes.len() / item_size;
    Vec::read_args(
        &mut std::io::Cursor::new(bytes),
        VecArgs::builder().count(count).finalize(),
    )
}
