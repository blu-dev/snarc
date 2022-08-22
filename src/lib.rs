#![feature(const_trait_impl)]
use hash40::Hash40;

pub mod engines;

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

#[test]
fn stream_test() {
    use std::path::Path;

    let engine = engines::stream::StreamEngine::from_directory(
        "/Users/blujay/Documents/Arc-Filesystems/filesystem_13.0.1",
    )
    .unwrap();

    engine.resolve();

    engine
        .get_path("stream:/sound/bgm/bgm_crs01_menu.nus3audio")
        .unwrap();

    let writer = engines::stream::StreamWriter::from_engine(engine);
    writer
        .to_directory("/Users/blujay/Documents/Arc-Filesystems/filesystem_13.0.1/roundtrip")
        .unwrap();

    for file in [
        "stream_folders.bin",
        "stream_paths.bin",
        "stream_metadatas.bin",
        "stream_path_keys.bin",
        "stream_links.bin",
    ] {
        if std::fs::read(
            Path::new("/Users/blujay/Documents/Arc-Filesystems/filesystem_13.0.1").join(file),
        )
        .unwrap()
            != std::fs::read(
                Path::new("/Users/blujay/Documents/Arc-Filesystems/filesystem_13.0.1/roundtrip")
                    .join(file),
            )
            .unwrap()
        {
            panic!("{}", file);
        }
    }
}
