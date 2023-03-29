use binrw::binrw;
use hash40::Hash40;

bitflags::bitflags! {
    pub struct PackageFlags : u32 {
        const IS_LOCALIZED = 1 << 24;
        const IS_REGIONAL = 1 << 25;
        const HAS_SUB_PACKAGE = 1 << 26;
        const SYM_LINK_IS_REGIONAL = 1 << 27;
        const IS_SYM_LINK = 1 << 28;
    }

    pub struct InfoFlags : u32 {
        const IS_REGULAR_FILE = 1 << 4;
        const IS_GRAPHICS_ARCHIVE = 1 << 12;
        const IS_LOCALIZED = 1 << 15;
        const IS_REGIONAL = 1 << 16;
        const IS_SHARED = 1 << 20;
        const UNKNOWN = 1 << 21;
    }

    pub struct MetadataFlags : u32 {
        const IS_REGULAR_ZSTD = 1 << 0;
        const IS_COMPRESSED = 1 << 1;
        const IS_VERSIONED_REGIONAL_DATA = 1 << 2;
        const IS_VERSIONED_LOCALIZED_DATA = 1 << 3;
    }
}

pub struct RawHashKey {
    hash: u32,
    len_and_index: u32,
}

impl RawHashKey {
    pub fn hash40(self) -> Hash40 {
        Hash40(self.len_and_index as u64 | ((self.len_and_index & 0x0000_00FF) as u64) << 32)
    }

    pub fn index(self) -> usize {
        (self.len_and_index >> 8) as usize
    }
}

pub struct RawPackage {
    pub path_and_group_index: RawHashKey,
    pub name: RawHashKey,
    pub parent: RawHashKey,
    pub lifetime: RawHashKey,
    pub info_start: u32,
    pub info_count: u32,
    pub child_start: u32,
    pub child_count: u32,
    pub flags: PackageFlags,
}

pub struct RawGroup {
    archive_offset_1: u32,
    archive_offset_2: u32,
    pub decompressed_size: u32,
    pub compressed_size: u32,
    pub range_start: u32,
    pub range_count: u32,
    pub sub_package: u32,
}

impl RawGroup {
    pub fn archive_offset(self) -> usize {
        self.archive_offset_1 as usize | (self.archive_offset_2 as usize) << 32
    }
}

pub struct RawPath {
    pub path_and_link_index: RawHashKey,
    pub extension_and_versioned_file_index: RawHashKey,
    pub parent: Hash40,
    pub file_name: Hash40,
}

pub struct RawLink {
    pub owner: u32,
    pub info: u32,
}

pub struct RawInfo {
    pub path: u32,
    pub link: u32,
    pub descriptor: u32,
    pub flags: InfoFlags,
}

pub struct RawDescriptor {
    pub group: u32,
    pub metadata: u32,
    pub load_args: u32,
}

pub struct RawMetadata {
    pub group_offset: u32,
    pub compressed_size: u32,
    pub decompressed_size: u32,
    pub flags: MetadataFlags,
}

pub struct RawSearchFolder {
    pub path_and_folder_count: RawHashKey,
    pub parent_and_file_count: RawHashKey,
    pub name: Hash40,
    pub first_child_index: u32,
}

pub struct RawSearchPath {
    pub path_and_next_index: RawHashKey,
    pub parent_and_is_folder: RawHashKey,
    pub name: RawHashKey,
    pub extension: RawHashKey,
}
