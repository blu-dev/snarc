use std::{
    collections::BTreeMap,
    io::{self, Read, Seek, SeekFrom, Write},
    num::NonZeroUsize,
};

use binrw::{binread, binrw, BinRead, BinWrite, FilePtr64, VecArgs};
use hash40::Hash40;
use semver::Version;

use crate::{
    engines::{
        packaged::{
            bucket_map::BucketMap,
            types::{Info, Patch},
            PackagedEngine, PackagedWriter, ToMemoryResults,
        },
        search::{SearchEngine, SearchWriter},
        stream::{StreamEngine, StreamWriter},
        table::TableCell,
    },
    Hashable,
};

pub struct Archive {
    pub file_section_offset: usize,
    pub packaged_fs: PackagedEngine,
    pub search_fs: SearchEngine,
    pub stream_fs: StreamEngine,
    pub version: Version,
    pub region_lookup_table: Vec<(u32, u32, u32)>,
}

#[binrw]
#[brw(magic = 0x10u32)]
struct ArchiveTablesHeader {
    #[br(map = |size: u32| size as usize)]
    #[bw(map = |size: &usize| *size as u32)]
    pub decompressed_size: usize,

    #[br(map = |size: u32| size as usize)]
    #[bw(map = |size: &usize| *size as u32)]
    pub compressed_size: usize,

    #[br(map = |size: u32| size as usize)]
    #[bw(map = |size: &usize| *size as u32)]
    pub compressed_section_size: usize,
}

impl ArchiveTablesHeader {
    #[allow(clippy::uninit_vec)]
    pub fn read_table<R: Read + Seek>(&self, reader: &mut R, offset: usize) -> io::Result<Vec<u8>> {
        let mut compressed_data = Vec::with_capacity(self.compressed_size);

        unsafe {
            compressed_data.set_len(self.compressed_size);
        }

        reader.read_exact(&mut compressed_data)?;
        let time = std::time::Instant::now();
        let decompressed_data =
            crate::decompress_data_with_size(&compressed_data, self.decompressed_size);
        println!("{}", time.elapsed().as_secs_f32());

        reader.seek(io::SeekFrom::Start(
            (offset + self.compressed_section_size) as u64,
        ))?;

        Ok(decompressed_data)
    }
}

#[binrw]
#[bw(import { write_output: ToMemoryResults })]
#[derive(Debug)]
struct PackagedFsHeader {
    #[br(map = |count: u32| count as usize)]
    #[bw(map = |count: &usize| *count as u32)]
    pub path_count: usize,

    #[br(map = |count: u32| count as usize)]
    #[bw(map = |count: &usize| *count as u32)]
    pub link_count: usize,

    #[br(map = |count: u32| count as usize)]
    #[bw(map = |count: &usize| *count as u32)]
    pub package_count: usize,

    #[br(temp)]
    #[bw(calc = write_output.metadata_group_len as u32)]
    metadata_group_count: u32,

    #[br(map = |count: u32| count as usize)]
    #[bw(map = |count: &usize| *count as u32)]
    pub child_package_count: usize,

    #[br(temp)]
    #[bw(calc = write_output.packaged_info_len as u32)]
    package_info_count: u32,

    #[br(temp)]
    #[bw(calc = write_output.packaged_descriptor_len as u32)]
    package_descriptor_count: u32,

    #[br(temp)]
    #[bw(calc = write_output.packaged_data_len as u32)]
    package_metadata_count: u32,

    #[br(temp)]
    #[bw(calc = write_output.info_group_len as u32)]
    info_group_count: u32,

    #[br(temp)]
    #[brw(pad_after = 0xC)]
    #[bw(calc = write_output.group_info_len as u32)]
    group_info_count: u32,

    #[br(map = |count: u8| count as usize)]
    #[bw(map = |_: &usize| 14u8)]
    pub locale_count: usize,

    #[br(map = |count: u8| count as usize)]
    #[bw(map = |_: &usize| 5u8)]
    #[brw(pad_after = 0x2)]
    pub region_count: usize,

    #[br(map = |raw: (u8, u8, u16)| Version::new(raw.2 as u64, raw.1 as u64, raw.0 as u64))]
    #[bw(map = |version: &Version| (version.patch as u8, version.minor as u8, version.major as u16))]
    pub version: Version,

    #[br(temp)]
    #[bw(calc = write_output.version_group_len as u32)]
    version_group_count: u32,

    #[brw(pad_after = 0x4)]
    #[br(map = |count: u32| count as usize)]
    #[bw(map = |count: &usize| *count as u32)]
    pub versioned_file_count: usize,

    #[br(temp)]
    #[bw(calc = write_output.version_info_len as u32)]
    version_info_count: u32,

    #[br(temp)]
    #[bw(calc = write_output.version_descriptor_len as u32)]
    version_descriptor_count: u32,

    #[br(temp)]
    #[bw(calc = write_output.version_data_len as u32)]
    version_metadata_count: u32,

    #[br(count = locale_count)]
    pub locale_region_hash_to_region: Vec<(u32, u32, u32)>,

    #[br(calc = (metadata_group_count + info_group_count + version_group_count) as usize)]
    #[bw(ignore)]
    pub group_count: usize,

    #[br(calc = (package_info_count + group_info_count + version_info_count) as usize)]
    #[bw(ignore)]
    pub info_count: usize,

    #[br(calc = (package_descriptor_count + group_info_count + version_descriptor_count) as usize)]
    #[bw(ignore)]
    pub descriptor_count: usize,

    #[br(calc = (package_metadata_count + group_info_count + version_metadata_count) as usize)]
    #[bw(ignore)]
    pub metadata_count: usize,
}

#[binrw]
struct StreamFsHeader {
    #[br(map = |count: u32| count as usize)]
    #[bw(map = |count: &usize| *count as u32)]
    pub folder_count: usize,

    #[br(map = |count: u32| count as usize)]
    #[bw(map = |count: &usize| *count as u32)]
    pub path_count: usize,

    #[br(map = |count: u32| count as usize)]
    #[bw(map = |count: &usize| *count as u32)]
    pub link_count: usize,

    #[br(map = |count: u32| count as usize)]
    #[bw(map = |count: &usize| *count as u32)]
    pub metadata_count: usize,
}

struct ArchiveNonUserTables(PackagedEngine, StreamEngine, Version, Vec<(u32, u32, u32)>);

impl BinRead for ArchiveNonUserTables {
    type Args = ();

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        options: &binrw::ReadOptions,
        _args: Self::Args,
    ) -> binrw::BinResult<Self> {
        let header = ArchiveTablesHeader::read_options(reader, options, ())?;
        let offset = reader.stream_position()? as usize;
        let data = header.read_table(reader, offset)?;
        let mut data = io::Cursor::new(data);
        let _filesystem_size = u32::read_options(&mut data, options, ())?;
        let packaged_header = PackagedFsHeader::read_options(&mut data, options, ())?;
        let stream_header = StreamFsHeader::read_options(&mut data, options, ())?;

        let stream_folders = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder()
                .count(stream_header.folder_count)
                .finalize(),
        )?;

        data.seek(io::SeekFrom::Current((stream_header.path_count * 8) as i64))?;

        let stream_paths = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder()
                .count(stream_header.path_count)
                .finalize(),
        )?;

        let stream_links = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder()
                .count(stream_header.link_count)
                .finalize(),
        )?;

        let stream_metadatas = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder()
                .count(stream_header.metadata_count)
                .finalize(),
        )?;

        let mut stream_engine = StreamEngine {
            path_lookup: BTreeMap::new(),
            folders: stream_folders,
            paths: stream_paths,
            links: stream_links,
            metadatas: stream_metadatas,
        };

        stream_engine.path_lookup = stream_engine
            .paths
            .iter()
            .map(|cell| (cell.get().full_path, cell.clone()))
            .collect();

        let path_lookup_count = u32::read_options(&mut data, options, ())? as usize;
        let path_bucket_count = u32::read_options(&mut data, options, ())? as usize;

        data.seek(io::SeekFrom::Current(
            (path_lookup_count * 8 + path_bucket_count * 8) as i64,
        ))?;

        let paths = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder()
                .count(packaged_header.path_count)
                .finalize(),
        )?;

        let links = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder()
                .count(packaged_header.link_count)
                .finalize(),
        )?;

        data.seek(io::SeekFrom::Current(
            (packaged_header.package_count * 8) as i64,
        ))?;

        let packages = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder()
                .count(packaged_header.package_count)
                .finalize(),
        )?;

        let groups = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder()
                .count(packaged_header.group_count)
                .finalize(),
        )?;

        let child_packages = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder()
                .count(packaged_header.child_package_count)
                .finalize(),
        )?;

        let infos = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder()
                .count(packaged_header.info_count)
                .finalize(),
        )?;

        let descriptors = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder()
                .count(packaged_header.descriptor_count)
                .finalize(),
        )?;

        let metadatas = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder()
                .count(packaged_header.metadata_count)
                .finalize(),
        )?;

        let version: (u8, u8, u16) = BinRead::read_options(&mut data, options, ())?;
        let version = Version::new(version.2 as u64, version.1 as u64, version.0 as u64);

        let num_patches = u32::read_options(&mut data, options, ())?;

        let patches: Vec<TableCell<Patch>> = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder().count(num_patches as usize).finalize(),
        )?;

        let mut versioned_files = Vec::with_capacity(packaged_header.versioned_file_count);

        for patch in patches.iter().map(TableCell::get) {
            versioned_files.extend(Vec::read_options(
                &mut data,
                options,
                VecArgs::builder()
                    .count(patch.file_count as usize)
                    .finalize(),
            )?);

            let bucket_count = u32::read_options(&mut data, options, ())?;
            let lookup_count = u32::read_options(&mut data, options, ())?;
            data.seek(io::SeekFrom::Current(
                (bucket_count * 8 + lookup_count * 8) as i64,
            ))?;
        }

        let mut packaged_engine = PackagedEngine {
            version,
            package_lookup: BTreeMap::new(),
            file_lookup: BucketMap::new(
                NonZeroUsize::new(path_bucket_count).expect("Bucket count should be non-zero"),
            ),
            packages,
            child_packages,
            groups,
            paths,
            links,
            infos,
            descriptors,
            metadatas,
            patches,
            versioned_files,
        };

        packaged_engine.package_lookup = packaged_engine
            .packages
            .iter()
            .map(|package| (package.get().full_path, package.clone()))
            .collect();

        for path in packaged_engine.paths.iter() {
            packaged_engine
                .file_lookup
                .insert(path.get().full_path, path.clone());
        }

        Ok(Self(
            packaged_engine,
            stream_engine,
            packaged_header.version,
            packaged_header.locale_region_hash_to_region,
        ))
    }
}

#[binrw]
struct SearchFsHeader {
    #[br(map = |count: u32| count as usize)]
    #[bw(map = |count: &usize| *count as u32)]
    pub folder_count: usize,

    #[br(map = |count: u32| count as usize)]
    #[bw(map = |count: &usize| *count as u32)]
    pub path_link_count: usize,

    #[br(map = |count: u32| count as usize)]
    #[bw(map = |count: &usize| *count as u32)]
    pub path_count: usize,
}

struct ArchiveUserTables(SearchEngine);

impl BinRead for ArchiveUserTables {
    type Args = ();

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        options: &binrw::ReadOptions,
        args: Self::Args,
    ) -> binrw::BinResult<Self> {
        let header = ArchiveTablesHeader::read_options(reader, options, ())?;
        let offset = reader.stream_position()? as usize;
        let data = header.read_table(reader, offset)?;
        let mut data = io::Cursor::new(data);

        let _filesystem_size = u64::read_options(&mut data, options, ())?;

        let search_header = SearchFsHeader::read_options(&mut data, options, ())?;

        data.seek(io::SeekFrom::Current(
            (search_header.folder_count * 0x8) as i64,
        ))?;

        let folders = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder()
                .count(search_header.folder_count)
                .finalize(),
        )?;

        data.seek(io::SeekFrom::Current(
            (search_header.path_link_count * 0xC) as i64,
        ))?;

        let paths = Vec::read_options(
            &mut data,
            options,
            VecArgs::builder()
                .count(search_header.path_count)
                .finalize(),
        )?;

        let mut search_engine = SearchEngine {
            folder_lookup: BTreeMap::new(),
            path_lookup: BTreeMap::new(),
            folders,
            paths,
        };

        search_engine.folder_lookup = search_engine
            .folders
            .iter()
            .map(|folder| (folder.get().full_path, folder.clone()))
            .collect();
        search_engine.path_lookup = search_engine
            .paths
            .iter()
            .filter_map(|path| {
                if path.get().full_path == Hash40::new("") {
                    None
                } else {
                    Some((path.get().full_path, path.clone()))
                }
            })
            .collect();

        Ok(Self(search_engine))
    }
}

impl BinRead for Archive {
    type Args = ();

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        options: &binrw::ReadOptions,
        _args: Self::Args,
    ) -> binrw::BinResult<Self> {
        const MAGIC: u64 = 0xABCDEF9876543210;

        let magic = u64::read_options(reader, options, ())?;
        if magic != MAGIC {
            return Err(binrw::Error::BadMagic {
                pos: options.offset(),
                found: Box::new(format!("{:#x}", magic)),
            });
        }

        let _stream_data_start = u64::read_options(reader, options, ())?;
        let file_data_start = u64::read_options(reader, options, ())?;
        let _shared_file_data_start = u64::read_options(reader, options, ())?;

        let ArchiveNonUserTables(packaged_fs, stream_fs, version, region_lookup_table) =
            FilePtr64::parse(reader, options, ())?;
        let ArchiveUserTables(search_fs) = FilePtr64::parse(reader, options, ())?;

        Ok(Self {
            file_section_offset: file_data_start as usize,
            packaged_fs,
            search_fs,
            stream_fs,
            version,
            region_lookup_table,
        })
    }
}

impl Archive {
    pub fn open(path: impl AsRef<std::path::Path>) -> binrw::BinResult<Self> {
        let mut reader = std::io::BufReader::with_capacity(0x0010_0000, std::fs::File::open(path)?);
        Self::read(&mut reader)
    }

    pub fn resolve(&self) {
        self.packaged_fs.resolve();
        self.search_fs.resolve();
        self.stream_fs.resolve();
    }

    pub fn reorganize(self) -> Self {
        let Self {
            file_section_offset,
            packaged_fs,
            search_fs,
            stream_fs,
            version,
            region_lookup_table,
        } = self;
        Self {
            file_section_offset,
            packaged_fs: packaged_fs.reorganize(),
            search_fs: search_fs.reorganize(),
            stream_fs: stream_fs.reorganize(),
            version,
            region_lookup_table,
        }
    }

    pub fn add_file(&mut self, file: impl AsRef<str>, package: impl AsRef<str>) -> TableCell<Info> {
        let file = file.as_ref();
        let package = package.as_ref();

        self.search_fs.add_file(file);
        self.packaged_fs.add_file(file, package)
    }

    #[cfg(feature = "compression")]
    pub fn write_tables<W: Seek + Write>(self, writer: &mut W) -> binrw::BinResult<(usize, usize)> {
        let Self {
            packaged_fs,
            search_fs,
            stream_fs,
            version,
            region_lookup_table,
            ..
        } = self;

        let packaged_writer = PackagedWriter::from_engine(packaged_fs);
        let search_writer = SearchWriter::from_engine(search_fs);
        let stream_writer = StreamWriter::from_engine(stream_fs);

        let packages = packaged_writer.packages.len();
        let child_packages = packaged_writer.child_packages.len();
        let paths = packaged_writer.paths.len();
        let links = packaged_writer.links.len();
        let versioned_files = packaged_writer.versioned_files.len();

        let stream_folders = stream_writer.folders.len();
        let stream_paths = stream_writer.paths.len();
        let stream_links = stream_writer.links.len();
        let stream_metadatas = stream_writer.metadatas.len();

        let mut data = std::io::Cursor::new(vec![]);
        data.write_all(&[0u8; 0x110])?;
        stream_writer.to_memory(&mut data)?;
        let packaged_info = packaged_writer.to_memory(&mut data)?;

        let data = data.into_inner();
        let len = data.len();

        let mut data = std::io::Cursor::new(data);

        let packaged_header = PackagedFsHeader {
            path_count: paths,
            link_count: links,
            package_count: packages,
            child_package_count: child_packages,
            locale_count: 0,
            region_count: 0,
            version,
            versioned_file_count: versioned_files,
            locale_region_hash_to_region: region_lookup_table,
            group_count: 0,
            info_count: 0,
            descriptor_count: 0,
            metadata_count: 0,
        };

        (len as u32).write_to(&mut data)?;
        packaged_header.write_with_args(
            &mut data,
            PackagedFsHeaderBinWriteArgs::builder()
                .write_output(packaged_info)
                .finalize(),
        )?;
        StreamFsHeader {
            folder_count: stream_folders,
            path_count: stream_paths,
            link_count: stream_links,
            metadata_count: stream_metadatas,
        }
        .write_to(&mut data)?;

        let data = data.into_inner();
        let decompressed_non_user_len = data.len();
        let compressed_non_user_data = crate::compress_data(data);

        let search_folders = search_writer.folders.len();
        let search_paths = search_writer.paths.len();

        let mut data = std::io::Cursor::new(vec![]);
        data.write_all(&vec![0u8; 0x14])?;

        search_writer.to_memory(&mut data)?;
        let data = data.into_inner();
        let len = data.len();
        let mut data = std::io::Cursor::new(data);
        (len as u64).write_to(&mut data)?;

        SearchFsHeader {
            folder_count: search_folders,
            path_count: search_paths,
            path_link_count: search_paths,
        }
        .write_to(&mut data)?;

        let data = data.into_inner();
        let decompressed_user_data_len = data.len();
        let compressed_user_data = crate::compress_data(data);

        let mut compressed_non_user_section_size = compressed_non_user_data.len();
        if compressed_non_user_section_size % 8 != 0 {
            compressed_non_user_section_size += 8 - (compressed_non_user_section_size % 8);
        }

        let mut compressed_user_section_size = compressed_user_data.len();
        if compressed_user_section_size % 8 != 0 {
            compressed_user_section_size += 8 - (compressed_user_section_size % 8);
        }

        let non_user_fs_start = writer.stream_position()?;
        ArchiveTablesHeader {
            decompressed_size: decompressed_non_user_len,
            compressed_size: compressed_non_user_data.len(),
            compressed_section_size: compressed_non_user_section_size,
        }
        .write_to(writer)?;
        writer.write_all(&compressed_non_user_data)?;

        let user_fs_start = writer.stream_position()?;

        ArchiveTablesHeader {
            decompressed_size: decompressed_user_data_len,
            compressed_size: compressed_user_data.len(),
            compressed_section_size: compressed_user_section_size,
        }
        .write_to(writer)?;
        writer.write_all(&compressed_user_data)?;

        Ok((non_user_fs_start as usize, user_fs_start as usize))
    }
}
