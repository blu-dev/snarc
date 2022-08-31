use std::{
    cell::{Ref, RefMut},
    collections::BTreeMap,
    io::{Seek, SeekFrom, Write},
    num::NonZeroUsize,
    rc::Rc,
    str::FromStr,
};

use crate::{Hashable, INVALID_INDEX};

use self::bucket_map::BucketMap;

pub mod bucket_map;
pub mod types;

use binrw::{BinRead, BinWrite, VecArgs};
use hash40::Hash40;
use semver::Version;
use types::*;

use super::{
    read_table,
    table::{TableCell, TableMaker},
    HashKey,
};

pub struct PackagedEngine {
    pub version: Version,
    pub(crate) package_lookup: BTreeMap<Hash40, TableCell<Package>>,
    pub(crate) file_lookup: BucketMap<TableCell<Path>>,

    pub packages: Vec<TableCell<Package>>,
    pub child_packages: Vec<TableCell<ChildPackage>>,
    pub groups: Vec<TableCell<Group>>,

    pub paths: Vec<TableCell<Path>>,
    pub links: Vec<TableCell<Link>>,
    pub infos: Vec<TableCell<Info>>,
    pub descriptors: Vec<TableCell<Descriptor>>,
    pub metadatas: Vec<TableCell<Metadata>>,

    pub patches: Vec<TableCell<Patch>>,
    pub versioned_files: Vec<TableCell<VersionedFile>>,
}

impl PackagedEngine {
    pub fn from_directory(path: impl AsRef<std::path::Path>) -> binrw::BinResult<Self> {
        let path = path.as_ref();

        // Manually parse the version info section since it's not just a singular table
        // The archive devs suck idk why they would do it this way but they suck and I hate them
        // T____T
        let mut version_data =
            std::fs::read(path.join("version_info.bin")).map(std::io::Cursor::new)?;

        let version: (u8, u8, u16) = BinRead::read(&mut version_data)?;
        let version = Version::new(version.2 as u64, version.1 as u64, version.0 as u64);
        let num_versions = u32::read(&mut version_data)? as usize;

        let patches: Vec<TableCell<Patch>> = BinRead::read_args(
            &mut version_data,
            VecArgs::builder().count(num_versions).finalize(),
        )?;

        let mut versioned_files = vec![];

        for patch in patches.iter() {
            let patch = patch.get();

            versioned_files.extend(Vec::read_args(
                &mut version_data,
                VecArgs::builder()
                    .count(patch.file_count as usize)
                    .finalize(),
            )?);

            version_data.seek(SeekFrom::Current(patch.lookup_size_in_bytes as i64))?;
        }

        let packages: Vec<TableCell<Package>> =
            read_table(&path.join("packages.bin"), Package::REPR_SIZE)?;
        let child_packages: Vec<TableCell<ChildPackage>> =
            read_table(&path.join("child_packages.bin"), ChildPackage::REPR_SIZE)?;
        let groups: Vec<TableCell<Group>> = read_table(&path.join("groups.bin"), Group::REPR_SIZE)?;
        let paths: Vec<TableCell<Path>> = read_table(&path.join("paths.bin"), Path::REPR_SIZE)?;
        let links: Vec<TableCell<Link>> = read_table(&path.join("links.bin"), Link::REPR_SIZE)?;
        let infos: Vec<TableCell<Info>> = read_table(&path.join("infos.bin"), Info::REPR_SIZE)?;
        let descriptors: Vec<TableCell<Descriptor>> =
            read_table(&path.join("descriptors.bin"), Descriptor::REPR_SIZE)?;
        let metadatas: Vec<TableCell<Metadata>> =
            read_table(&path.join("metadatas.bin"), Metadata::REPR_SIZE)?;

        let package_lookup = packages
            .iter()
            .map(|package| (package.get().full_path, package.clone()))
            .collect();

        let bucket_count = (std::fs::metadata(path.join("path_buckets.bin"))?.len() as usize) / 8;

        let mut file_lookup = BucketMap::new(
            NonZeroUsize::new(bucket_count).expect("Bucket count should be non-zero!"),
        );

        for path in paths.iter() {
            file_lookup.insert(path.get().full_path, path.clone());
        }

        Ok(Self {
            version,
            package_lookup,
            file_lookup,

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
        })
    }

    pub fn resolve(&self) {
        for package in self.packages.iter() {
            package
                .get_mut()
                .resolve(&self.groups, &self.infos, &self.child_packages);
        }

        for child_package in self.child_packages.iter() {
            child_package.get_mut().resolve(&self.packages);
        }

        let mut info_group_info_start = 0;
        let mut version_group_info_start = 0;
        for (index, group) in self.groups.iter().enumerate() {
            group.get_mut().resolve(
                &self.packages,
                &self.groups,
                &self.infos,
                &self.metadatas,
                index,
            );

            if group.get().is_info_group() && info_group_info_start == 0 {
                info_group_info_start =
                    (group.get().infos().cells()[0].guid() - self.infos[0].guid()) as usize;
            }

            if group.get().is_version_group() && version_group_info_start == 0 {
                version_group_info_start =
                    (group.get().infos().cells()[0].guid() - self.infos[0].guid()) as usize;
            }
        }

        let latest_patch_file_info_start = (self.groups.last().unwrap().get().infos().cells()[0]
            .guid()
            - self.infos[0].guid()) as usize;
        let latest_patch_file_start = latest_patch_file_info_start - version_group_info_start;
        for path in self.paths.iter() {
            path.get_mut()
                .resolve(&self.links, &self.versioned_files, latest_patch_file_start);
        }

        for link in self.links.iter() {
            link.get_mut()
                .resolve(&self.packages, &self.groups, &self.infos);
        }

        for (index, info) in self.infos.iter().enumerate() {
            info.get_mut()
                .resolve(&self.paths, &self.links, &self.descriptors);
            let info_offset = if index >= info_group_info_start {
                0
            } else {
                info_group_info_start
            };
            for mut descriptor in info.get().descriptors.iter_mut() {
                descriptor.resolve(
                    &self.groups,
                    &self.links,
                    &self.infos,
                    &self.metadatas,
                    &self.patches,
                    info_offset,
                    index >= version_group_info_start,
                );
            }
        }

        for (count, patch) in self.patches.iter().enumerate() {
            patch.get_mut().resolve(
                &self.versioned_files,
                &self.infos,
                &self.groups,
                count,
                version_group_info_start,
            );
        }

        for versioned_file in self.versioned_files.iter() {
            versioned_file.get_mut().resolve(&self.links, &self.infos);
        }
    }

    pub fn get_package(&self, hash: impl Hashable) -> Option<Ref<'_, Package>> {
        self.package_lookup.get(&hash.to_hash()).map(TableCell::get)
    }

    pub fn get_package_mut(&self, hash: impl Hashable) -> Option<RefMut<'_, Package>> {
        self.package_lookup
            .get(&hash.to_hash())
            .map(TableCell::get_mut)
    }

    pub fn get_file(&self, hash: impl Hashable) -> Option<Ref<'_, Path>> {
        self.file_lookup.get(hash.to_hash()).map(TableCell::get)
    }

    pub fn get_file_mut(&self, hash: impl Hashable) -> Option<RefMut<'_, Path>> {
        self.file_lookup.get(hash.to_hash()).map(TableCell::get_mut)
    }

    pub fn reorganize(self) -> Self {
        let writer = PackagedWriter::from_engine(self);
        Self {
            version: writer.version,
            package_lookup: writer.package_lookup,
            file_lookup: writer.file_lookup,

            packages: writer.packages.into_inner(),
            child_packages: writer.child_packages.into_inner(),
            groups: writer.groups.into_inner(),

            paths: writer.paths.into_inner(),
            links: writer.links.into_inner(),
            infos: writer.infos.into_inner(),
            descriptors: writer.descriptors.into_inner(),
            metadatas: writer.metadatas.into_inner(),

            patches: writer.patches.into_inner(),
            versioned_files: writer.versioned_files.into_inner(),
        }
    }

    pub fn has_file(&self, hash: impl Hashable) -> bool {
        self.get_file(hash).is_some()
    }

    pub fn has_package(&self, hash: impl Hashable) -> bool {
        self.get_package(hash).is_some()
    }

    pub fn add_file(&mut self, file: impl AsRef<str>, package: impl Hashable) -> TableCell<Info> {
        let file = file.as_ref();
        if self.has_file(file) {
            panic!("File {} already exists!", file);
        }

        let package = if let Some(cell) = self.package_lookup.get(&package.to_hash()) {
            cell
        } else {
            panic!("Package does not exist!")
        };

        let mut descriptor = Descriptor::new();
        descriptor.set_metadata(TableCell::new(Metadata::new()));
        descriptor.set_group(package.get().groups.cells()[0].clone());

        package.get().groups.cells()[0]
            .get_mut()
            .metadatas_mut()
            .push(descriptor.raw_metadata().clone());

        let mut info = Info::new();

        info.descriptors.replace(vec![TableCell::new(descriptor)]);

        let link = TableCell::new(Link::new());

        link.get_mut().set_package(package.clone());
        info.set_link(link.clone());

        let mut path = Path::from_str(file).unwrap();
        path.set_link(link.clone());

        info.is_graphics_archive = path.has_graphics_archive_extension();
        info.is_regular_file = !info.is_graphics_archive;

        info.set_path(TableCell::new(path));

        self.file_lookup
            .insert(info.path().full_path, info.raw_path().clone());

        let info = TableCell::new(info);

        link.get_mut().set_info(info.clone());
        package.get_mut().infos.push(info.clone());
        info
    }
}

pub struct PackagedWriter {
    version: Version,
    pub(crate) package_lookup: BTreeMap<Hash40, TableCell<Package>>,
    pub(crate) file_lookup: BucketMap<TableCell<Path>>,
    pub(crate) packages: TableMaker<Package>,
    pub(crate) child_packages: TableMaker<ChildPackage>,
    pub(crate) groups: TableMaker<Group>,
    pub(crate) paths: TableMaker<Path>,
    pub(crate) links: TableMaker<Link>,
    pub(crate) infos: TableMaker<Info>,
    pub(crate) descriptors: TableMaker<Descriptor>,
    pub(crate) metadatas: TableMaker<Metadata>,
    pub(crate) patches: TableMaker<Patch>,
    pub(crate) versioned_files: TableMaker<VersionedFile>,

    last_patch_files_start: usize,
    info_group_info_start: usize,
}

impl PackagedWriter {
    fn push_package(&mut self, package: &TableCell<Package>) {
        self.packages.push(package.clone());
        let package = package.get();
        for group in package.groups.cells().iter() {
            self.push_group(group);
        }

        for child_package in package.child_packages.cells().iter() {
            self.push_child_package(child_package);
        }

        for info in package.infos.cells().iter() {
            self.push_info(info, false, false);
        }
    }

    fn push_child_package(&mut self, child_package: &TableCell<ChildPackage>) {
        self.child_packages.push(child_package.clone());
    }

    fn push_group(&mut self, group: &TableCell<Group>) {
        self.groups.push(group.clone());
        let group = group.get();

        if group.is_metadata_group() {
            for metadata in group.metadatas().cells().iter() {
                if !self.metadatas.has_cell(metadata) {
                    self.push_metadata(metadata);
                }
            }
        } else {
            let is_versioned = group.is_version_group();
            for info in group.infos().cells().iter() {
                self.push_info(info, true, is_versioned);
            }
        }
    }

    fn push_path(&mut self, path: &TableCell<Path>) {
        self.paths.push(path.clone());
    }

    fn push_link(&mut self, link: &TableCell<Link>) {
        self.links.push(link.clone());
    }

    fn push_info(
        &mut self,
        info: &TableCell<Info>,
        is_info_group_info: bool,
        is_version_group_info: bool,
    ) {
        self.infos.push(info.clone());
        let info = info.get();
        for descriptor in info.descriptors.cells().iter() {
            self.push_descriptor(descriptor, is_info_group_info, is_version_group_info);
        }

        if is_info_group_info {
            if !info.is_shared && !self.links.has_cell(info.raw_link()) {
                self.push_link(info.raw_link())
            }
        } else {
            if !info.is_shared
                && !matches!(
                    info.descriptors.cells()[0].get().load_args,
                    DescriptorLoadArguments::PackageSkip { .. }
                )
                && !self.links.has_cell(info.raw_link())
            {
                self.push_link(info.raw_link())
            }

            if !self.paths.has_cell(info.raw_path()) {
                self.push_path(info.raw_path());
            }
        }
    }

    fn push_descriptor(
        &mut self,
        descriptor: &TableCell<Descriptor>,
        is_info_group_descriptor: bool,
        is_version_group_descriptor: bool,
    ) {
        self.descriptors.push(descriptor.clone());
        let descriptor = descriptor.get();

        if is_version_group_descriptor {
            match &descriptor.load_args {
                DescriptorLoadArguments::Owned { patch }
                    if self.patches.has_cell(patch.patch())
                        && descriptor.has_metadata()
                        && !self.metadatas.has_cell(descriptor.raw_metadata()) =>
                {
                    self.push_metadata(descriptor.raw_metadata());
                }
                _ => {}
            }
        } else if is_info_group_descriptor {
            self.push_metadata(descriptor.raw_metadata());
        }
    }

    fn push_metadata(&mut self, metadata: &TableCell<Metadata>) {
        self.metadatas.push(metadata.clone());
    }

    fn push_patch(&mut self, patch: &TableCell<Patch>) {
        self.patches.push(patch.clone());
        for file in patch.get().versioned_files.cells().iter() {
            self.push_versioned_file(file);
        }
        self.push_group(patch.get().raw_group());
        for info in patch
            .get()
            .infos
            .cells()
            .iter()
            .skip(patch.get().group().infos().len())
        {
            self.push_info(info, true, true);
        }
    }

    fn push_versioned_file(&mut self, versioned_file: &TableCell<VersionedFile>) {
        self.versioned_files.push(versioned_file.clone());
    }

    pub fn from_engine(engine: PackagedEngine) -> Self {
        let PackagedEngine {
            version,
            package_lookup,
            file_lookup,
            packages,
            groups,
            patches,
            ..
        } = engine;

        let mut this = Self {
            version,
            package_lookup,
            file_lookup,

            packages: TableMaker::new(),
            child_packages: TableMaker::new(),
            groups: TableMaker::new(),
            paths: TableMaker::new(),
            links: TableMaker::new(),
            infos: TableMaker::new(),
            descriptors: TableMaker::new(),
            metadatas: TableMaker::new(),
            patches: TableMaker::new(),
            versioned_files: TableMaker::new(),

            last_patch_files_start: 0,
            info_group_info_start: 0,
        };

        for package in packages {
            this.push_package(&package);
        }

        for group in groups {
            if group.get().is_info_group() && !group.get().is_version_group() {
                this.push_group(&group);
                if this.info_group_info_start == 0 {
                    this.info_group_info_start =
                        this.infos.get_index(&group.get().infos().cells()[0]) as usize;
                }
            }
        }

        for patch in patches.iter() {
            this.push_patch(patch);
        }

        if let Some(last_patch) = patches.last() {
            this.last_patch_files_start = this
                .versioned_files
                .get_index(&last_patch.get().versioned_files.cells()[0])
                as usize;
        }

        this
    }

    pub fn to_directory(self, path: impl AsRef<std::path::Path>) -> binrw::BinResult<()> {
        let path = path.as_ref();
        let this = Rc::new(self);

        let mut packages = std::io::Cursor::new(vec![]);
        let mut child_packages = std::io::Cursor::new(vec![]);
        let mut groups = std::io::Cursor::new(vec![]);

        let mut paths = std::io::Cursor::new(vec![]);
        let mut links = std::io::Cursor::new(vec![]);
        let mut infos = std::io::Cursor::new(vec![]);
        let mut descriptors = std::io::Cursor::new(vec![]);
        let mut metadatas = std::io::Cursor::new(vec![]);

        for package in this.packages.iter() {
            package.write_with_args(&mut packages, Rc::clone(&this))?;
        }

        for child_package in this.child_packages.iter() {
            child_package.write_with_args(&mut child_packages, Rc::clone(&this))?;
        }

        for group in this.groups.iter() {
            group.write_with_args(&mut groups, Rc::clone(&this))?;
        }

        for path in this.paths.iter() {
            path.write_with_args(&mut paths, Rc::clone(&this))?;
        }

        for link in this.links.iter() {
            link.write_with_args(&mut links, Rc::clone(&this))?;
        }

        let mut info_group_descriptor_start = 0;

        for (count, info) in this.infos.iter().enumerate() {
            if count == this.info_group_info_start {
                info_group_descriptor_start =
                    this.descriptors
                        .get_index(&info.get().descriptors.cells()[0]) as usize;
            }
            info.write_with_args(&mut infos, Rc::clone(&this))?;
        }

        for (count, descriptor) in this.descriptors.iter().enumerate() {
            if count >= info_group_descriptor_start {
                descriptor.write_with_args(&mut descriptors, (Rc::clone(&this), 0))?;
            } else {
                descriptor.write_with_args(
                    &mut descriptors,
                    (Rc::clone(&this), this.info_group_info_start),
                )?;
            }
        }

        for metadata in this.metadatas.iter() {
            metadata.write_to(&mut metadatas)?;
        }

        std::fs::write(path.join("packages.bin"), packages.into_inner())?;
        std::fs::write(path.join("child_packages.bin"), child_packages.into_inner())?;
        std::fs::write(path.join("groups.bin"), groups.into_inner())?;

        std::fs::write(path.join("paths.bin"), paths.into_inner())?;
        std::fs::write(path.join("links.bin"), links.into_inner())?;
        std::fs::write(path.join("infos.bin"), infos.into_inner())?;
        std::fs::write(path.join("descriptors.bin"), descriptors.into_inner())?;
        std::fs::write(path.join("metadatas.bin"), metadatas.into_inner())?;

        let mut version_info = std::io::Cursor::new(vec![]);

        (this.version.patch as u8).write_to(&mut version_info)?;
        (this.version.minor as u8).write_to(&mut version_info)?;
        (this.version.major as u16).write_to(&mut version_info)?;
        (this.patches.len() as u32).write_to(&mut version_info)?;

        for (count, patch) in this.patches.iter().enumerate() {
            patch.write_with_args(&mut version_info, (Rc::clone(&this), true, count))?;
        }

        for patch in this.patches.iter() {
            patch.write_with_args(&mut version_info, (Rc::clone(&this), false, INVALID_INDEX))?;
        }

        std::fs::write(path.join("version_info.bin"), version_info.into_inner())?;

        let Self {
            package_lookup,
            file_lookup,
            packages,
            paths,
            ..
        } = if let Ok(this) = Rc::try_unwrap(this) {
            this
        } else {
            unreachable!()
        };

        let mut package_keys = std::io::Cursor::new(vec![]);

        for (hash, package) in package_lookup {
            HashKey::new(hash, packages.get_index(&package) as usize)
                .write_to(&mut package_keys)?;
        }

        let mut path_buckets = std::io::Cursor::new(vec![]);
        let file_lookup = file_lookup.into_inner();
        let mut total = 0;
        for bucket in file_lookup.iter() {
            total.write_to(&mut path_buckets)?;
            total += bucket.len() as u32;
            total.write_to(&mut path_buckets)?;
        }

        let mut path_keys = std::io::Cursor::new(vec![]);

        for (key, path) in file_lookup
            .into_iter()
            .flat_map(|bucket| bucket.into_iter())
        {
            HashKey::new(key, paths.get_index(&path) as usize).write_to(&mut path_keys)?;
        }

        std::fs::write(path.join("package_keys.bin"), package_keys.into_inner())?;
        std::fs::write(path.join("path_buckets.bin"), path_buckets.into_inner())?;
        std::fs::write(path.join("path_keys.bin"), path_keys.into_inner())?;

        Ok(())
    }

    pub fn to_memory<W: Seek + Write>(self, writer: &mut W) -> binrw::BinResult<ToMemoryResults> {
        let this = Rc::new(self);

        let bucket_count = this.file_lookup.bucket_count();
        let total_length = this.file_lookup.len();

        (total_length as u32).write_to(writer)?;
        (bucket_count as u32).write_to(writer)?;

        let mut total = 0;
        for bucket in this.file_lookup.buckets() {
            total.write_to(writer)?;
            total += bucket.len() as u32;

            (bucket.len() as u32).write_to(writer)?;
        }

        for (hash, cell) in this.file_lookup.iter() {
            HashKey::new(*hash, this.paths.get_index(cell) as usize).write_to(writer)?;
        }

        for path in this.paths.iter() {
            path.write_with_args(writer, Rc::clone(&this))?;
        }

        for link in this.links.iter() {
            link.write_with_args(writer, Rc::clone(&this))?;
        }

        for (hash, cell) in this.package_lookup.iter() {
            HashKey::new(*hash, this.packages.get_index(cell) as usize).write_to(writer)?;
        }

        for package in this.packages.iter() {
            package.write_with_args(writer, Rc::clone(&this))?;
        }

        let mut output = ToMemoryResults {
            packaged_info_len: 0,
            group_info_len: 0,
            version_info_len: 0,
            packaged_descriptor_len: 0,
            group_descriptor_len: 0,
            version_descriptor_len: 0,
            packaged_data_len: 0,
            group_data_len: 0,
            version_data_len: 0,
            metadata_group_len: 0,
            info_group_len: 0,
            version_group_len: 0,
        };

        for (count, group) in this.groups.iter().enumerate() {
            if output.packaged_info_len == 0 && group.get().is_info_group() {
                let group_ = group.get();
                let info = &group_.infos().cells()[0];
                let info_ = info.get();
                let desc = &info_.descriptors.cells()[0];
                let desc_ = desc.get();
                let data = desc_.raw_metadata();
                output.packaged_info_len = this.infos.get_index(info) as usize;
                output.packaged_descriptor_len = this.descriptors.get_index(desc) as usize;
                output.packaged_data_len = this.metadatas.get_index(data) as usize;
                output.metadata_group_len = count;
            } else if output.group_info_len == 0 && group.get().is_version_group() {
                let group_ = group.get();
                let info = &group_.infos().cells()[0];
                let info_ = info.get();
                let desc = &info_.descriptors.cells()[0];
                let desc_ = desc.get();
                let data = desc_.raw_metadata();
                output.group_info_len =
                    this.infos.get_index(info) as usize - output.packaged_info_len;
                output.group_descriptor_len =
                    this.descriptors.get_index(desc) as usize - output.packaged_descriptor_len;
                output.group_data_len =
                    this.metadatas.get_index(data) as usize - output.packaged_data_len;
                output.info_group_len = count - output.metadata_group_len;
            }
            group.write_with_args(writer, Rc::clone(&this))?;
        }

        output.version_info_len =
            this.infos.len() - output.group_info_len - output.packaged_info_len;
        output.version_descriptor_len =
            this.descriptors.len() - output.group_descriptor_len - output.packaged_descriptor_len;
        output.version_data_len =
            this.metadatas.len() - output.group_data_len - output.packaged_data_len;
        output.version_group_len =
            this.groups.len() - output.info_group_len - output.metadata_group_len;

        for child_package in this.child_packages.iter() {
            child_package.write_with_args(writer, Rc::clone(&this))?;
        }

        for info in this.infos.iter() {
            info.write_with_args(writer, Rc::clone(&this))?;
        }

        for (count, descriptor) in this.descriptors.iter().enumerate() {
            if count >= output.packaged_descriptor_len {
                descriptor.write_with_args(writer, (Rc::clone(&this), 0))?;
            } else {
                descriptor.write_with_args(writer, (Rc::clone(&this), output.packaged_info_len))?;
            }
        }

        for metadata in this.metadatas.iter() {
            metadata.write_to(writer)?;
        }

        (this.version.patch as u8).write_to(writer)?;
        (this.version.minor as u8).write_to(writer)?;
        (this.version.major as u16).write_to(writer)?;

        (this.patches.len() as u32).write_to(writer)?;

        for (count, patch) in this.patches.iter().enumerate() {
            patch.write_with_args(writer, (Rc::clone(&this), true, count))?;
        }

        for patch in this.patches.iter() {
            patch.write_with_args(writer, (Rc::clone(&this), false, INVALID_INDEX))?;
        }

        Ok(output)
    }
}

#[derive(Copy, Clone)]
pub struct ToMemoryResults {
    pub packaged_info_len: usize,
    pub group_info_len: usize,
    pub version_info_len: usize,
    pub packaged_descriptor_len: usize,
    pub group_descriptor_len: usize,
    pub version_descriptor_len: usize,
    pub packaged_data_len: usize,
    pub group_data_len: usize,
    pub version_data_len: usize,

    pub metadata_group_len: usize,
    pub info_group_len: usize,
    pub version_group_len: usize,
}
