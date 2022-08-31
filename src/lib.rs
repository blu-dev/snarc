#![feature(const_trait_impl)]
use std::io::Read;

use hash40::Hash40;

pub mod archive;
pub mod engines;

#[cfg(feature = "raw")]
pub mod raw;

pub trait Decompressor: Sync + Send {
    fn decompress(&self, data: &[u8]) -> std::io::Result<Vec<u8>>;
    fn decompress_with_size(&self, data: &[u8], size: usize) -> std::io::Result<Vec<u8>>;
}

pub struct DefaultDecompressor;

impl Decompressor for DefaultDecompressor {
    fn decompress(&self, data: &[u8]) -> std::io::Result<Vec<u8>> {
        let mut decoder = ruzstd::StreamingDecoder::new(std::io::Cursor::new(data)).unwrap();
        let mut data = Vec::new();
        decoder.read_to_end(&mut data).map(|_| data)
    }

    fn decompress_with_size(&self, data: &[u8], size: usize) -> std::io::Result<Vec<u8>> {
        let mut decoder = ruzstd::StreamingDecoder::new(std::io::Cursor::new(data)).unwrap();
        let mut data = Vec::with_capacity(size);
        decoder.read_to_end(&mut data).map(|_| data)
    }
}

static GLOBAL_DECOMPRESSOR: std::sync::RwLock<&'static dyn Decompressor> =
    std::sync::RwLock::new(&DefaultDecompressor);

/// The invalid index for any archive table
///
/// Most indices found in the archive are 24-bit integers, of which the value
/// `-1` is used as the invalid index. Any index which takes on this value
/// is not used by the game unless the archive format requires that the index
/// be valid/unsigned, in which case no check is performed.
pub const INVALID_INDEX: usize = 0x00FF_FFFF;
const INVALID_INDEX32: u32 = INVALID_INDEX as u32;

const HASH_MASK: u64 = 0x0000_00FF_FFFF_FFFF;

pub trait Hashable {
    fn to_hash(self) -> Hash40;
}

impl const Hashable for &str {
    fn to_hash(self) -> Hash40 {
        Hash40::new(self)
    }
}

impl const Hashable for Hash40 {
    fn to_hash(self) -> Hash40 {
        self
    }
}

pub fn load_labels(labels: impl AsRef<std::path::Path>) {
    let map = Hash40::label_map();
    let mut lock = map.lock().unwrap();
    lock.add_labels_from_path(labels).unwrap();
}

pub fn decompress_data(data: impl AsRef<[u8]>) -> Vec<u8> {
    GLOBAL_DECOMPRESSOR
        .read()
        .unwrap()
        .decompress(data.as_ref())
        .unwrap()
}

pub fn decompress_data_with_size(data: impl AsRef<[u8]>, decompressed_size: usize) -> Vec<u8> {
    GLOBAL_DECOMPRESSOR
        .read()
        .unwrap()
        .decompress_with_size(data.as_ref(), decompressed_size)
        .unwrap()
}

pub fn set_decompressor(decompressor: &'static dyn Decompressor) {
    *GLOBAL_DECOMPRESSOR.write().unwrap() = decompressor;
}

#[cfg(feature = "compression")]
pub fn compress_data(data: impl AsRef<[u8]>) -> Vec<u8> {
    let data = data.as_ref();
    zstd::encode_all(std::io::Cursor::new(data), 0).unwrap()
}
