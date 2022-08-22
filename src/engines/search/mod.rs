use std::{
    cell::{Ref, RefMut},
    collections::BTreeMap,
    path::Path,
    rc::Rc,
};

use crate::Hashable;

use super::{
    read_table,
    table::{TableCell, TableMaker},
    HashKey,
};

pub mod types;
use binrw::BinWrite;
use hash40::Hash40;
use types::*;

/// The engine to drive search filesystem accessing/modification
pub struct SearchEngine {
    /// The lookup from hash -> folder
    folder_lookup: BTreeMap<Hash40, TableCell<SearchFolder>>,

    /// The lookup from hash -> path
    path_lookup: BTreeMap<Hash40, TableCell<SearchPath>>,

    /// The table of folders
    pub folders: Vec<TableCell<SearchFolder>>,

    /// The table of paths
    pub paths: Vec<TableCell<SearchPath>>,
}

impl SearchEngine {
    /// Reads the required data to construct an engine from the specified folder
    ///
    /// ### Arguments
    /// * `path` - The path of the directory to read from
    ///
    /// ### Returns
    /// * `Ok(Self)` - All reading/parsing was successful
    /// * `Err(_)` - There was an error reading/parsing the data
    ///
    /// ### Notes
    /// * The folder and path lookups are generated, and not read from files
    /// * [`SearchFolder`]s are read from `"search_folders.bin"`
    /// * [`SearchPath`]s are read from `"search_paths.bin"`
    pub fn from_directory(path: impl AsRef<Path>) -> binrw::BinResult<Self> {
        let path = path.as_ref();

        let folders: Vec<TableCell<SearchFolder>> =
            read_table(&path.join("search_folders.bin"), SearchFolder::REPR_SIZE)?;
        let paths: Vec<TableCell<SearchPath>> =
            read_table(&path.join("search_paths.bin"), SearchPath::REPR_SIZE)?;

        let folder_lookup = folders
            .iter()
            .map(|cell| (cell.get().full_path, cell.clone()))
            .collect();

        let path_lookup = paths
            .iter()
            .filter_map(|cell| {
                if cell.get().full_path == Hash40::new("") {
                    None
                } else {
                    Some((cell.get().full_path, cell.clone()))
                }
            })
            .collect();

        Ok(Self {
            folder_lookup,
            path_lookup,
            folders,
            paths,
        })
    }

    /// Resolves all references in the engine
    ///
    /// ### Panicking
    /// * There is an issue any structures. See the following for more:
    ///     * [`SearchFolder::resolve`]
    ///     * [`SearchPath::resolve`]
    pub fn resolve(&self) {
        for path in self.paths.iter() {
            path.get_mut().resolve(&self.paths, &self.folder_lookup);
        }

        for folder in self.folders.iter() {
            folder.get_mut().resolve(&self.paths);
        }
    }

    /// Gets an immutable reference to a folder by hash, if it exists
    pub fn get_folder(&self, hash: impl Hashable) -> Option<Ref<'_, SearchFolder>> {
        self.folder_lookup.get(&hash.to_hash()).map(TableCell::get)
    }

    /// Gets the mutable reference to a folder by hash, if it exists
    pub fn get_folder_mut(&self, hash: impl Hashable) -> Option<RefMut<'_, SearchFolder>> {
        self.folder_lookup
            .get(&hash.to_hash())
            .map(TableCell::get_mut)
    }

    /// Gets an immutable reference to a path by hash, if it exists
    pub fn get_path(&self, hash: impl Hashable) -> Option<Ref<'_, SearchPath>> {
        self.path_lookup.get(&hash.to_hash()).map(TableCell::get)
    }

    /// Gets the mutable reference to a path by hash, if it exists
    pub fn get_path_mut(&self, hash: impl Hashable) -> Option<RefMut<'_, SearchPath>> {
        self.path_lookup
            .get(&hash.to_hash())
            .map(TableCell::get_mut)
    }
}

/// Re-organizer and serializer for the search filesystem
pub struct SearchWriter {
    /// The hash -> folder lookup
    folder_lookup: BTreeMap<Hash40, TableCell<SearchFolder>>,

    /// The hash -> path lookup
    path_lookup: BTreeMap<Hash40, TableCell<SearchPath>>,

    /// The re-organized table of folders
    folders: TableMaker<SearchFolder>,

    /// The re-organized table of paths
    paths: TableMaker<SearchPath>,
}

impl SearchWriter {
    /// Pushes a folder and all of its paths to the new tables
    ///
    /// ### Panicking
    /// * The folder is already in the table, or any of its paths are
    fn push_folder(&mut self, folder: &TableCell<SearchFolder>) {
        self.folders.push(folder.clone());
        for path in folder.get().children.cells().iter() {
            self.push_path(path);
        }
    }

    /// Pushes a path to the tables
    ///
    /// ### Panicking
    /// * The path is already in the table
    fn push_path(&mut self, path: &TableCell<SearchPath>) {
        self.paths.push(path.clone());
    }

    /// Consumes the engine and creates a writer, re-organizing all of the
    /// tables and preparing for serialization
    ///
    /// ### Panicking
    /// * Any of the tables failed to re-organize correctly. This is not a user-error.
    pub fn from_engine(engine: SearchEngine) -> Self {
        let SearchEngine {
            folder_lookup,
            path_lookup,
            folders,
            ..
        } = engine;

        let mut this = Self {
            folder_lookup,
            path_lookup,

            folders: TableMaker::new(),
            paths: TableMaker::new(),
        };

        for folder in folders {
            this.push_folder(&folder);
        }
        this
    }

    /// Serializes the tables and lookups out to the specified directory
    ///
    /// ### Arguments
    /// * `path` - The path to serialize to
    ///
    /// ### Returns
    /// * `Ok(())` - The serialization was a success
    /// * `Err(_)` - There was an issue
    ///
    /// ### Notes
    /// * Hash -> folder lookup: `"search_folder_keys.bin"`
    /// * Hash -> path lookup: `"search_path_keys.bin"`
    /// * Search links: `"search_links.bin"`
    /// * [`SearchFolder`]s: `"search_folders.bin"
    /// * [`SearchPath`]s: `"search_paths.bin"`
    pub fn to_directory(self, path: impl AsRef<Path>) -> binrw::BinResult<()> {
        let path = path.as_ref();

        let this = Rc::new(self);

        let mut folders = std::io::Cursor::new(vec![]);
        for folder in this.folders.iter() {
            folder.write_with_args(&mut folders, Rc::clone(&this))?;
        }

        let mut links = std::io::Cursor::new(vec![]);
        let mut paths = std::io::Cursor::new(vec![]);
        for path in this.paths.iter() {
            this.paths.get_index(path).write_to(&mut links)?;
            path.write_with_args(&mut paths, Rc::clone(&this))?;
        }

        let mut folder_lookup = std::io::Cursor::new(vec![]);
        for (hash, path) in this.folder_lookup.iter() {
            HashKey::new(*hash, this.folders.get_index(path) as usize)
                .write_to(&mut folder_lookup)?;
        }

        let mut path_lookup = std::io::Cursor::new(vec![]);
        for (hash, path) in this.path_lookup.iter() {
            HashKey::new(*hash, this.paths.get_index(path) as usize).write_to(&mut path_lookup)?;
        }

        std::fs::write(path.join("search_folders.bin"), folders.into_inner())?;
        std::fs::write(path.join("search_paths.bin"), paths.into_inner())?;
        std::fs::write(path.join("search_path_links.bin"), links.into_inner())?;
        std::fs::write(
            path.join("search_folder_keys.bin"),
            folder_lookup.into_inner(),
        )?;
        std::fs::write(path.join("search_path_keys.bin"), path_lookup.into_inner())?;

        Ok(())
    }
}
