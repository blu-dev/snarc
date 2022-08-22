use std::{
    cell::{Ref, RefMut},
    collections::BTreeMap, rc::Rc
};

use binrw::{binread, BinWrite};
use hash40::Hash40;

use crate::{
    engines::{table::*, HashKey},
    INVALID_INDEX, INVALID_INDEX32,
};

use super::SearchWriter;

/// The main container for files in the search filesystem
/// 
/// Folders contain references to other paths, which can also be folders.
/// The search folder is the easiest way to find files which are relative
/// to each other, or relative to the folder itself.
#[binread]
pub struct SearchFolder {
    #[br(temp)]
    path_and_folder_count: HashKey,

    /// The full path of the folder, with no trailing `/`
    #[br(calc = path_and_folder_count.hash())]
    pub full_path: Hash40,

    #[br(temp)]
    parent_and_file_count: HashKey,

    /// The path of the parent of the folder, with no trailing `/`
    #[br(calc = parent_and_file_count.hash())]
    pub parent: Hash40,

    /// The name of the folder
    pub name: Hash40,

    /// The number of files this folder contains
    #[br(calc = parent_and_file_count.index())]
    pub file_count: usize,

    /// The number of child folders this folder contains (non-recursive)
    #[br(calc = path_and_folder_count.index())]
    pub folder_count: usize,

    #[br(align_after = 0x8)]
    #[br(temp)]
    first_child_index: u32,

    /// The children of the folder
    #[br(calc = TableLinkedReference::new(first_child_index as usize))]
    pub children: TableLinkedReference<SearchPath>,
}

multi_reference!(
    optional,
    /// An optional reference to the folder a path represents
    /// 
    /// This isn't actually an unresolved index, as other references
    /// are, but rather a single flag that determines if a path
    /// also has a corresponding [`SearchFolder`].
    /// 
    /// This is to prevent many repeated hash lookups when traversing
    /// the search filesystem.
    enum SearchPathFolderReference : single {
        /// The resolved reference to a folder
        Folder(SearchFolder),
    }
);

multi_reference!(
    optional,
    /// An optional reference to the next child of a folder
    /// 
    /// [`SearchPath`] structures are connected via a singly
    /// linked list, which is used to traverse around.
    /// 
    /// For engine purposes, this is resolved into a contiguous
    /// vector for easy manipulation and serialization.
    pub enum SearchPathNextReference : single {
        /// The resolved reference to the next entry in the list
        Path(SearchPath),
    }
);

/// The main item in the search filesystem
/// 
/// There is a search path for every *path* in the filesystem,
/// including both folders and files. The only exceptions
/// are any files in one of the archive's "mount" paths,
/// which are `prebuilt:` and `stream:`.
/// 
/// Paths are connected to each other via a singly linked list
/// and point to the next path that is the child of their common
/// parent, meaning paths only have one owning [folder](SearchFolder)
#[binread]
pub struct SearchPath {
    #[br(temp)]
    path_and_next_index: HashKey,

    /// The full path of the folder/file.
    /// 
    /// If this is a folder, then there is no trailing `/`
    #[br(calc = path_and_next_index.hash())]
    pub full_path: Hash40,

    #[br(temp)]
    parent_and_is_folder: HashKey,

    /// The path of the parent of this path, with no trailing `/`
    #[br(calc = parent_and_is_folder.hash())]
    pub parent: Hash40,

    /// The name component of this path.
    /// 
    /// If this is a file, it includes the extension.
    pub name: Hash40,

    /// The extension component of this path.
    /// 
    /// If this is a folder, this is blank.
    pub extension: Hash40,

    /// A reference to a [`SearchFolder`] if this path is for a folder
    #[br(calc = { 
        if parent_and_is_folder.index() & 0x0040_0000 != 0 { 
            SearchPathFolderReference::Unresolved(INVALID_INDEX)
        } else { 
            SearchPathFolderReference::None
        }
    })]
    folder: SearchPathFolderReference,

    /// The reference to the next child of this path's parent, if
    /// there is one
    #[br(calc = {
        if path_and_next_index.index() == INVALID_INDEX {
            SearchPathNextReference::None
        } else {
            SearchPathNextReference::Unresolved(path_and_next_index.index())
        }
    })]
    next: SearchPathNextReference,
}

impl SearchPathFolderReference {
    /// Resolves this reference
    /// 
    /// ### Arguments
    /// * `hash` - The hash of the path which this reference belongs to
    /// * `lookup` - The hash -> [`SearchFolder`] lookup to find the parent
    /// 
    /// ### Panicking
    /// * If this reference should be valid but the path cannot be found in the lookup
    pub fn resolve(&mut self, hash: Hash40, lookup: &BTreeMap<Hash40, TableCell<SearchFolder>>) {
        let mut tmp = Self::None;
        std::mem::swap(self, &mut tmp);
        *self = match tmp {
            Self::Unresolved(_) => Self::Folder(lookup.get(&hash).cloned().expect("Lookup should contain hash")),
            other => other,
        }
    }
}

impl SearchPathNextReference {
    /// Resolves this reference
    /// 
    /// ### Arguments
    /// * `paths` - The slice of paths to resolve with
    /// 
    /// ### Panicking
    /// * This reference's unresolved index is OOB of `paths`
    pub fn resolve(&mut self, paths: &[TableCell<SearchPath>]) {
        let mut tmp = Self::None;
        std::mem::swap(self, &mut tmp);
        *self = match tmp {
            Self::Unresolved(index) => Self::Path(paths[index].clone()),
            other => other,
        }
    }
}

impl SearchFolder {
    /// The size (in bytes) of this structure when serialized.
    pub(crate) const REPR_SIZE: usize = 0x20;

    /// Resolves this folder
    /// 
    /// ### Arguments
    /// * `paths` - The slice of [`SearchPath`] to resolve with
    /// 
    /// ### Panicking
    /// * There was an error resolving the underlying [`TableLinkedReference`]
    pub fn resolve(&mut self, paths: &[TableCell<SearchPath>]) {
        self.children.resolve(paths);
    }

    /// Checks if this folder is resolved
    pub fn is_resolved(&self) -> bool {
        self.children.is_resolved()
    }
}

impl SearchPath {
    /// The size (in bytes) of this structure when serialized.
    pub(crate) const REPR_SIZE: usize = 0x20;

    /// Resolves this path
    /// 
    /// ### Arguments
    /// * `paths` - The slice of paths to resolve the next reference with
    /// * `folder_lookup` - The hash -> [`SearchFolder`] lookup to use when resolving
    /// folders
    /// 
    /// ### Panicking
    /// * The next reference is OOB of `paths`
    /// * The folder cannot be found in `folder_lookup` when it is a folder
    pub fn resolve(
        &mut self,
        paths: &[TableCell<SearchPath>],
        folder_lookup: &BTreeMap<Hash40, TableCell<SearchFolder>>,
    ) {
        self.folder.resolve(self.full_path, folder_lookup);
        self.next.resolve(paths);
    }

    /// Checks if this path is resolved
    pub fn is_resolved(&self) -> bool {
        self.folder.is_resolved() && self.next.is_resolved()
    }

    /// Checks if this path represents a [`SearchFolder`]
    pub fn is_folder(&self) -> bool {
        self.folder.is_folder()
    }
}

expose_reference!(SearchPath, folder, SearchFolder, SearchPathFolderReference, Folder);

impl LinkedReference for SearchPath {
    fn next(&self) -> Option<TableCell<Self>> {
        (!self.next.is_none()).then(|| self.next.path().clone())
    }
}

impl BinWrite for SearchFolder {
    type Args = Rc<SearchWriter>;

    fn write_options<W: std::io::Write + std::io::Seek>(
            &self,
            writer: &mut W,
            options: &binrw::WriteOptions,
            args: Self::Args,
    ) -> binrw::BinResult<()> {
        HashKey::new(self.full_path, self.folder_count).write_options(writer, options, ())?;
        HashKey::new(self.parent, self.file_count).write_options(writer, options, ())?;
        self.name.write_options(writer, options, ())?;
        if self.children.is_empty() {
            INVALID_INDEX32.write_options(writer, options, ())?;
        } else {
            args.paths.get_index(&self.children.cells()[0]).write_options(writer, options, ())?;
        }
        [0u8; 4].write_options(writer, options, ())
    }
}

impl BinWrite for SearchPath {
    type Args = Rc<SearchWriter>;

    fn write_options<W: std::io::Write + std::io::Seek>(
            &self,
            writer: &mut W,
            options: &binrw::WriteOptions,
            args: Self::Args,
    ) -> binrw::BinResult<()> {
        let next_index = if self.next.is_none() {
            INVALID_INDEX
        } else {
            args.paths.get_index(self.next.path()) as usize
        };

        HashKey::new(self.full_path, next_index).write_options(writer, options, ())?;
        let is_folder = if self.folder.is_none() {
            0x0
        } else {
            0x0040_0000
        };
        
        HashKey::new(self.full_path, is_folder).write_options(writer, options, ())?;
        self.name.write_options(writer, options, ())?;
        self.extension.write_options(writer, options, ())
    }
}