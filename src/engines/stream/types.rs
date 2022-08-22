use binrw::{binread, binrw, BinWrite};
use hash40::Hash40;

use crate::engines::{table::*, HashKey};

use std::{
    cell::{Ref, RefMut},
    rc::Rc,
};

use super::StreamWriter;

/// The container for [stream paths](StreamPath).
///
/// Stream folders are a very simple form of a folder, being that
/// they only point to the paths that are inside of them.
///
/// In the archive currently, there are only three of these.
#[binread]
#[br(little)]
pub struct StreamFolder {
    #[br(temp)]
    name_and_count: HashKey,

    #[br(temp)]
    start: u32,

    /// The name of the folder, stripped of its parent's path.
    #[br(calc = name_and_count.hash())]
    pub name: Hash40,

    /// The paths which this folder references.
    #[br(calc = TableContiguousReference::new_from_count(start as usize, name_and_count.index()))]
    pub paths: TableContiguousReference<StreamPath>,
}

/// The main information for a file in the stream filesystem.
///
/// Stream files are very simple, so they don't need a lot of
/// information to describe them. Like other files in the archive,
/// they can be both `localized` or `regional`, which will change
/// the number of [links](StreamLink) that they refer to.
#[binread]
#[br(little)]
pub struct StreamPath {
    #[br(temp)]
    path_and_link: HashKey,

    #[br(temp)]
    flags: u32,

    /// The full path of this stream file, including the
    /// `stream:/` prefix.
    #[br(calc = path_and_link.hash())]
    pub full_path: Hash40,

    /// The stream links that this file refers to.
    ///
    /// If both the `is_localized` and `is_regional` flags
    /// are false, then this only refers to one link,
    /// otherwise it refers to 14 or 5, respectively.
    ///
    /// This localized/regional behavior is unlike the packaged
    /// filesystem, as there is no non-regional data to load
    /// and instead the game's implementation will default
    /// to Japanese.
    #[br(calc = match flags {
        0x0 => TableContiguousReference::new_from_count(path_and_link.index(), 1),
        0x1 => TableContiguousReference::new_from_count(path_and_link.index(), 14),
        0x2 => TableContiguousReference::new_from_count(path_and_link.index(), 5),
        _ => unreachable!()
    })]
    pub links: TableContiguousReference<StreamLink>,

    /// If this file has localized variants, mutually exclusive
    /// from `is_regional`
    #[br(calc = flags & 0x0000_0001 != 0)]
    pub is_localized: bool,

    /// If this file has regional variants, mutually exclusive
    /// from `is_localized`
    #[br(calc = flags & 0x0000_0002 != 0)]
    pub is_regional: bool,
}

/// The link between the [stream path](StreamPath) and the
/// [stream metadata](StreamMetadata).
///
/// A very simple structure which only redirects to the metadata
/// for the stream file.
///
/// There can be multiple of these per stream path, depending on if
/// it is localized/regional.
///
/// ## Note
/// Unlike the search filesystem, this small redirection cannot be
/// omitted as some localized/regional stream paths do not support
/// all of the locales/regions and use this to declare fallback
/// locales/regions.
#[binread]
pub struct StreamLink {
    #[br(map = |index: u32| TableReference::Unresolved(index as usize))]
    metadata: TableReference<StreamMetadata>,
}

/// The metadata for a stream file.
///
/// Unlike files in the packaged filesystem, stream files
/// are not allowed to be compressed, so they are instead
/// stored simply as archive offsets and data sizes.
#[binrw]
pub struct StreamMetadata {
    /// The size of the file data, in bytes.
    #[br(map = |size: u64| size as usize)]
    #[bw(map = |size: &usize| *size as u64)]
    pub size: usize,

    /// The offset into the archive where the data exists.
    #[br(map = |offset: u64| offset as usize)]
    #[bw(map = |offset: &usize| *offset as u64)]
    pub offset: usize,
}

impl StreamFolder {
    pub(crate) const REPR_SIZE: usize = 0xC;

    /// Resolves this folder
    ///
    /// ### Arguments
    /// * `paths` - The array of [`StreamPath`] cells in which this
    /// folder can safely index
    ///
    /// ### Panicking
    /// This function panics if the range of paths references by
    /// this folder is out-of-bounds of the provided slice
    pub fn resolve(&mut self, paths: &[TableCell<StreamPath>]) {
        self.paths.resolve(paths);
    }

    /// Checks if this folder is resolved
    ///
    /// ### Returns
    /// Whether this folder is resolved or not
    pub fn is_resolved(&self) -> bool {
        self.paths.is_resolved()
    }
}

impl StreamPath {
    pub(crate) const REPR_SIZE: usize = 0xC;

    /// Resolves this path
    ///
    /// ### Arguments
    /// * `links` - The array of [`StreamLink`] cells in which this
    /// path can safely index.
    ///
    /// ### Panicking
    /// This function panics if the range of links referenced by
    /// this path is out-of-bounds of the provided slice.
    pub fn resolve(&mut self, links: &[TableCell<StreamLink>]) {
        self.links.resolve(links);
    }

    /// Checks if this path is resolved
    ///
    /// ### Returns
    /// Whether this path is resolved or not
    pub fn is_resolved(&self) -> bool {
        self.links.is_resolved()
    }
}

impl StreamLink {
    pub(crate) const REPR_SIZE: usize = 0x4;

    /// Resolves this stream link
    ///
    /// ### Arguments
    /// * `metadatas` - The array of [`StreamMetadata`] in which this
    /// link can safely index
    ///
    /// ### Panicking
    /// This function panics if the unresolved metadata reference is
    /// out-of-bounds of the provided slice,
    pub fn resolve(&mut self, metadatas: &[TableCell<StreamMetadata>]) {
        self.metadata.resolve(metadatas);
    }

    /// Checks if this link is resolved
    ///
    /// ### Returns
    /// Whether this link is resolved or not
    pub fn is_resolved(&self) -> bool {
        self.metadata.is_resolved()
    }
}

impl StreamMetadata {
    pub(crate) const REPR_SIZE: usize = 0x10;
}

expose_reference!(StreamLink, metadata, StreamMetadata);

impl BinWrite for StreamFolder {
    type Args = Rc<StreamWriter>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        args: Self::Args,
    ) -> binrw::BinResult<()> {
        let start = if self.paths.is_empty() {
            0
        } else {
            args.paths.get_index(&self.paths.cells()[0])
        };

        let count = self.paths.len();

        HashKey::new(self.name, count).write_options(writer, options, ())?;

        start.write_options(writer, options, ())
    }
}

impl BinWrite for StreamPath {
    type Args = Rc<StreamWriter>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        args: Self::Args,
    ) -> binrw::BinResult<()> {
        let link = args.links.get_index(&self.links.cells()[0]) as usize;

        let flags = if self.is_localized {
            1u32
        } else if self.is_regional {
            2
        } else {
            0
        };

        HashKey::new(self.full_path, link).write_options(writer, options, ())?;
        flags.write_options(writer, options, ())
    }
}

impl BinWrite for StreamLink {
    type Args = Rc<StreamWriter>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        args: Self::Args,
    ) -> binrw::BinResult<()> {
        args.metadatas
            .get_index(self.raw_metadata())
            .write_options(writer, options, ())
    }
}
