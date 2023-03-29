use std::{
    cell::{Ref, RefMut},
    collections::BTreeMap,
    io::{Seek, Write},
    path::Path,
    rc::Rc,
};

pub mod types;
use binrw::BinWrite;
use hash40::Hash40;
use types::*;

use crate::Hashable;

use super::{read_table, table::*, HashKey};

/// File engine to access data in the stream filesystem
///
/// The stream filesystem consists of files which cannot be
/// compressed and instead must be read as a contiguous stream
/// of data from code outside of scope of the application.
///
/// For example, BGM and movie files will be found here
pub struct StreamEngine {
    /// The lookup from the hash of a file path to the
    /// [`StreamPath`]
    pub(crate) path_lookup: BTreeMap<Hash40, TableCell<StreamPath>>,

    /// All of the folders in the stream filesystem
    pub folders: Vec<TableCell<StreamFolder>>,

    /// All of the paths in the stream filesystem
    pub paths: Vec<TableCell<StreamPath>>,

    /// All of the stream links in the stream filesystem
    pub links: Vec<TableCell<StreamLink>>,

    /// All of the stream metadatas in the stream filesystem
    pub metadatas: Vec<TableCell<StreamMetadata>>,
}

impl StreamEngine {
    /// Reads the tables from the provided folder and constructs a new engine
    ///
    /// ### Arguments
    /// * `path` - The path of the folder to read from
    ///
    /// ### Returns
    /// * `Ok(Self)` - The engine was successfully created
    /// * `Err(_)` - There was an error in parsing the files
    ///
    /// ### Notes
    /// * [`StreamFolder`]s are read from `"stream_folders.bin"`
    /// * [`StreamPath`]s are read from `"stream_paths.bin"`
    /// * [`StreamLink`]s are read from `"stream_links.bin"`
    /// * [`StreamMetadata`]s are read from `"stream_metadatas.bin"`
    /// * The stream path lookup is not read from this folder, but is generated
    pub fn from_directory(path: impl AsRef<Path>) -> binrw::BinResult<Self> {
        let path = path.as_ref();

        // Read all of the tables
        let folders: Vec<TableCell<StreamFolder>> =
            read_table(&path.join("stream_folders.bin"), StreamFolder::REPR_SIZE)?;
        let paths: Vec<TableCell<StreamPath>> =
            read_table(&path.join("stream_paths.bin"), StreamPath::REPR_SIZE)?;
        let links: Vec<TableCell<StreamLink>> =
            read_table(&path.join("stream_links.bin"), StreamLink::REPR_SIZE)?;
        let metadatas: Vec<TableCell<StreamMetadata>> = read_table(
            &path.join("stream_metadatas.bin"),
            StreamMetadata::REPR_SIZE,
        )?;

        // Generate the lookup
        let path_lookup = paths
            .iter()
            .map(|path| (path.get().full_path, path.clone()))
            .collect();

        Ok(Self {
            path_lookup,
            folders,
            paths,
            links,
            metadatas,
        })
    }

    /// Resolves all of the tables in the filesystem
    ///
    /// ### Panicking
    /// This function can panic if there is unexpected data in the
    /// tables, such as an OOB index.
    pub fn resolve(&self) {
        for mut folder in self.folders.iter().map(TableCell::get_mut) {
            folder.resolve(&self.paths);
        }

        for mut path in self.paths.iter().map(TableCell::get_mut) {
            path.resolve(&self.links);
        }

        for mut link in self.links.iter().map(TableCell::get_mut) {
            link.resolve(&self.metadatas);
        }
    }

    /// Gets an immutable path reference from the provided path
    ///
    /// ### Arguments
    /// * `hash` - The hash of the file you want to access
    ///
    /// ### Returns
    /// * `Some(_)` - The path for the specified hash exists
    /// * `None` - The path for the specified hash does not exist
    pub fn get_path(&self, hash: impl Hashable) -> Option<Ref<'_, StreamPath>> {
        self.path_lookup.get(&hash.to_hash()).map(TableCell::get)
    }

    /// Gets a mutable path reference from the provided path
    ///
    /// ### Arguments
    /// * `hash` - The hash of the file you want to access
    ///
    /// ### Returns
    /// * `Some(_)` - The path for the specified hash exists
    /// * `None` - The path for the specified hash does not exist
    pub fn get_path_mut(&self, hash: impl Hashable) -> Option<RefMut<'_, StreamPath>> {
        self.path_lookup
            .get(&hash.to_hash())
            .map(TableCell::get_mut)
    }

    pub fn reorganize(self) -> Self {
        let writer = StreamWriter::from_engine(self);
        Self {
            path_lookup: writer.path_lookup,
            folders: writer.folders.into_inner(),
            paths: writer.paths.into_inner(),
            links: writer.links.into_inner(),
            metadatas: writer.metadatas.into_inner(),
        }
    }
}

/// Reorganizes the tables from the engine, optionally serializing them to bytes
///
/// After accessing and modifying data via a [`StreamEngine`], use this structure
/// to fix up all the tables and serialize them.
///
/// It is important to understand that an engine taken straight from
/// the vanilla archive is *not* guaranteed to roundtrip byte-for-byte
/// as there might be unreferenced entries.
pub struct StreamWriter {
    /// The lookup from hash to [`StreamPath`]
    path_lookup: BTreeMap<Hash40, TableCell<StreamPath>>,

    /// The new table for folders
    pub(crate) folders: TableMaker<StreamFolder>,

    /// The new table for paths
    pub(crate) paths: TableMaker<StreamPath>,

    /// The new table for links
    pub(crate) links: TableMaker<StreamLink>,

    /// The new table for metadatas
    pub(crate) metadatas: TableMaker<StreamMetadata>,
}

impl StreamWriter {
    /// Pushes a new folder to the tables, along with all owned entries
    ///
    /// ### Arguments
    /// * `folder` - The folder to push
    ///
    /// ### Panicking
    /// This function panics if the folder is already present
    fn push_folder(&mut self, folder: &TableCell<StreamFolder>) {
        self.folders.push(folder.clone());
        for path in folder.get().paths.cells().iter() {
            self.push_path(path);
        }
    }

    /// Pushes a new path to the tables, along with all owned entries
    ///
    /// ### Arguments
    /// * `path` - The path to push
    ///
    /// ### Panicking
    /// This function panics if the path is already present
    fn push_path(&mut self, path: &TableCell<StreamPath>) {
        self.paths.push(path.clone());
        for link in path.get().links.cells().iter() {
            self.push_link(link);
        }
    }

    /// Pushes a new link to the tables, along with all owned entries
    ///
    /// ### Arguments
    /// * `link` - The link to push
    ///
    /// ### Panicking
    /// This function panics if the link is already present
    fn push_link(&mut self, link: &TableCell<StreamLink>) {
        self.links.push(link.clone());
        // Since links can refer to metadata from previous links,
        // we don't push the metadata if it is already present.
        if !self.metadatas.has_cell(link.get().raw_metadata()) {
            self.push_metadata(link.get().raw_metadata());
        }
    }

    /// Pushes a new metadata to the tables
    ///
    /// ### Arguments
    /// * `metadata` - The metadata to push
    ///
    /// ### Panicking
    /// This function panics if the metadata is already present
    fn push_metadata(&mut self, metadata: &TableCell<StreamMetadata>) {
        self.metadatas.push(metadata.clone());
    }

    /// Constructs and reorganizes the tables from the provided engine,
    /// consuming the engine.
    ///
    /// ### Arguments
    /// * `engine` - The engine to reorganize
    ///
    /// ### Returns
    /// The newly constructed tables, ready to be serialized.
    ///
    /// ### Panicking
    /// This function will panic if there is an error in reorganizing
    /// any of the tables
    pub fn from_engine(engine: StreamEngine) -> Self {
        let StreamEngine {
            path_lookup,
            folders,
            ..
        } = engine;

        let mut this = Self {
            path_lookup,
            folders: TableMaker::new(),
            paths: TableMaker::new(),
            links: TableMaker::new(),
            metadatas: TableMaker::new(),
        };

        for folder in folders {
            this.push_folder(&folder);
        }

        this
    }

    /// Writes the tables to the specified directory, consuming the writer
    ///
    /// ### Arguments
    /// * `path` - Path of the directory to serialize to
    ///
    /// ### Returns
    /// * `Ok(())` - There were no errors while writing
    /// * `Err(_)` - There were errors while serializing tables
    ///
    /// ### Panicking
    /// This function can panic if there is an issue with locating table entries
    ///
    /// ### Notes
    /// * [`StreamFolder`]s are serialized to `"stream_folders.bin"`
    /// * [`StreamPath`]s are serialized to `"stream_paths.bin"`
    /// * [`StreamLink`]s are serialized to `"stream_links.bin"`
    /// * [`StreamMetadata`]s are serialized to `"stream_metadatas.bin"`
    /// * The path lookup is serialized to `"stream_path_keys.bin"`
    pub fn to_directory(self, path: impl AsRef<Path>) -> binrw::BinResult<()> {
        let path = path.as_ref();

        let this = Rc::new(self);

        let mut folders = std::io::Cursor::new(vec![]);
        for folder in this.folders.iter() {
            folder.write_with_args(&mut folders, Rc::clone(&this))?;
        }

        let mut paths = std::io::Cursor::new(vec![]);
        for path in this.paths.iter() {
            path.write_with_args(&mut paths, Rc::clone(&this))?;
        }

        let mut links = std::io::Cursor::new(vec![]);
        for link in this.links.iter() {
            link.write_with_args(&mut links, Rc::clone(&this))?;
        }

        let mut metadatas = std::io::Cursor::new(vec![]);
        for metadata in this.metadatas.iter() {
            metadata.write_to(&mut metadatas)?;
        }

        let mut lookup = std::io::Cursor::new(vec![]);
        for (hash, path) in this.path_lookup.iter() {
            HashKey::new(*hash, this.paths.get_index(path) as usize).write_to(&mut lookup)?;
        }

        std::fs::write(path.join("stream_folders.bin"), folders.into_inner())?;
        std::fs::write(path.join("stream_paths.bin"), paths.into_inner())?;
        std::fs::write(path.join("stream_links.bin"), links.into_inner())?;
        std::fs::write(path.join("stream_metadatas.bin"), metadatas.into_inner())?;
        std::fs::write(path.join("stream_path_keys.bin"), lookup.into_inner())?;

        Ok(())
    }

    pub fn to_memory<W: Seek + Write>(self, writer: &mut W) -> binrw::BinResult<()> {
        let this = Rc::new(self);

        for folder in this.folders.iter() {
            folder.write_with_args(writer, Rc::clone(&this))?;
        }

        for (hash, cell) in this.path_lookup.iter() {
            HashKey::new(*hash, this.paths.get_index(cell) as usize).write_to(writer)?;
        }

        for path in this.paths.iter() {
            path.write_with_args(writer, Rc::clone(&this))?;
        }

        for link in this.links.iter() {
            link.write_with_args(writer, Rc::clone(&this))?;
        }

        for metadata in this.metadatas.iter() {
            metadata.write_to(writer)?;
        }

        Ok(())
    }
}
