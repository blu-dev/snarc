use crate::{
    engines::{table::*, HashKey},
    Hashable, INVALID_INDEX, INVALID_INDEX32,
};
use binrw::{binread, binrw, BinRead, BinWrite};
use camino::Utf8Path;
use hash40::Hash40;
use semver::Version;
use std::{
    cell::{Ref, RefMut},
    num::NonZeroUsize,
    rc::Rc,
};
use std::{ops::Range, str::FromStr};
use thiserror::Error;

use super::{bucket_map::BucketMap, PackagedWriter};

#[binread]
pub struct Package {
    #[br(temp)]
    path_and_group_index: HashKey,

    #[br(calc = path_and_group_index.hash())]
    pub full_path: Hash40,

    pub name: Hash40,
    pub parent: Hash40,
    pub lifetime: Hash40,

    #[br(temp)]
    info_start: u32,

    #[br(temp)]
    info_count: u32,

    #[br(temp)]
    child_package_start: u32,

    #[br(temp)]
    child_package_count: u32,

    #[br(temp)]
    flags: u32,

    #[br(calc = {
        if flags & (1 << 24) != 0 {
            TableContiguousReference::new_from_count(path_and_group_index.index(), 15)
        } else if flags & (1 << 25) != 0 {
            TableContiguousReference::new_from_count(path_and_group_index.index(), 6)
        } else {
            TableContiguousReference::new_from_count(path_and_group_index.index(), 1)
        }
    })]
    pub groups: TableContiguousReference<Group>,

    #[br(calc = TableContiguousReference::new_from_count(info_start as usize, info_count as usize))]
    pub infos: TableContiguousReference<Info>,

    #[br(calc = TableContiguousReference::new_from_count(child_package_start as usize, child_package_count as usize))]
    pub child_packages: TableContiguousReference<ChildPackage>,

    #[br(calc = flags & (1 << 24) != 0)]
    pub is_localized: bool,

    #[br(calc = flags & (1 << 25) != 0)]
    pub is_regional: bool,

    #[br(calc = flags & (1 << 26) != 0)]
    pub has_sub_package: bool,

    #[br(calc = flags & (1 << 27) != 0)]
    pub sym_link_is_regional: bool,

    #[br(calc = flags & (1 << 28) != 0)]
    pub is_sym_link: bool,
}

#[binread]
pub struct ChildPackage {
    #[br(temp)]
    key: HashKey,

    #[br(calc = key.hash())]
    pub full_path: Hash40,

    #[br(calc = TableReference::Unresolved(key.index()))]
    package: TableReference<Package>,
}

multi_reference!(
    optional,
    enum GroupSubPackageReference : single {
        Package(Package),
        Group(Group),
    }
);

multi_reference!(
    enum GroupFileReference : set {
        Metadata(Metadata),
        Info(Info),
    }
);

#[binread]
pub struct Group {
    #[br(map = |size: u64| size as usize)]
    pub archive_offset: usize,

    #[br(map = |size: u32| size as usize)]
    pub decompressed_size: usize,

    #[br(map = |size: u32| size as usize)]
    pub compressed_size: usize,

    #[br(temp)]
    range_start: u32,

    #[br(temp)]
    range_count: u32,

    #[br(calc = GroupFileReference::Unresolved((range_start as usize)..((range_start + range_count) as usize)))]
    files: GroupFileReference,

    #[br(map = |index: u32| {
        if index == INVALID_INDEX32 {
            GroupSubPackageReference::None
        } else {
            GroupSubPackageReference::Unresolved(index as usize)
        }
    })]
    sub_package: GroupSubPackageReference,
}

multi_reference!(
    optional,
    enum PathVersionedFileReference : single {
        VersionedFile(VersionedFile),
    }
);

#[binread]
pub struct Path {
    #[br(temp)]
    path_and_link_index: HashKey,

    #[br(temp)]
    extension_and_versioned_file_index: HashKey,

    #[br(calc = path_and_link_index.hash())]
    pub full_path: Hash40,

    #[br(calc = extension_and_versioned_file_index.hash())]
    pub extension: Hash40,

    pub parent: Hash40,

    pub file_name: Hash40,

    #[br(calc = TableReference::Unresolved(path_and_link_index.index()))]
    link: TableReference<Link>,

    #[br(calc = {
        if extension_and_versioned_file_index.index() == INVALID_INDEX {
            PathVersionedFileReference::None
        } else {
            PathVersionedFileReference::Unresolved(extension_and_versioned_file_index.index())
        }
    })]
    versioned_file: PathVersionedFileReference,
}

multi_reference!(
    enum LinkOwnerReference : single {
        Package(Package),
        Group(Group),
    }
);

#[binread]
pub struct Link {
    #[br(map = |index: u32| LinkOwnerReference::Unresolved(index as usize))]
    owner: LinkOwnerReference,

    #[br(map = |index: u32| TableReference::Unresolved(index as usize))]
    info: TableReference<Info>,
}

#[binread]
pub struct Info {
    #[br(map = |index: u32| TableReference::Unresolved(index as usize))]
    path: TableReference<Path>,

    #[br(map = |index: u32| TableReference::Unresolved(index as usize))]
    link: TableReference<Link>,

    #[br(temp)]
    descriptor_index: u32,

    #[br(temp)]
    flags: u32,

    #[br(calc = {
        if flags & (1 << 15) != 0 {
            TableContiguousReference::new_from_count(descriptor_index as usize, 15)
        } else if flags & (1 << 16) != 0 {
            TableContiguousReference::new_from_count(descriptor_index as usize, 6)
        } else {
            TableContiguousReference::new_from_count(descriptor_index as usize, 1)
        }
    })]
    pub descriptors: TableContiguousReference<Descriptor>,

    #[br(calc = flags & (1 << 4) != 0)]
    pub is_regular_file: bool,

    #[br(calc = flags & (1 << 12) != 0)]
    pub is_graphics_archive: bool,

    #[br(calc = flags & (1 << 15) != 0)]
    pub is_localized: bool,

    #[br(calc = flags & (1 << 16) != 0)]
    pub is_regional: bool,

    #[br(calc = flags & (1 << 20) != 0)]
    pub is_shared: bool,

    #[br(calc = flags & (1 << 21) != 0)]
    pub is_unknown_flag: bool,
}

multi_reference!(
    optional,
    pub enum DescriptorLoadArgumentsPatchReference : single {
        Patch(Patch),
    }
);

pub enum DescriptorLoadArguments {
    Unowned {
        link: TableReference<Link>,
    },
    Owned {
        patch: DescriptorLoadArgumentsPatchReference,
    },
    PackageSkip {
        info: TableReference<Info>,
    },
    Unknown,
    SharedButOwned {
        link: TableReference<Link>,
    },
    UnsupportedRegion {
        region_locale: i32,
    },
}

multi_reference!(
    optional,
    enum DescriptorMetadataReference : single {
        Metadata(Metadata),
    }
);

#[binread]
pub struct Descriptor {
    #[br(map = |index: u32| TableReference::Unresolved(index as usize))]
    group: TableReference<Group>,

    #[br(map = |index: u32| {
        if index == INVALID_INDEX32 {
            DescriptorMetadataReference::None
        } else {
            DescriptorMetadataReference::Unresolved(index as usize)
        }
    })]
    metadata: DescriptorMetadataReference,

    pub load_args: DescriptorLoadArguments,
}

#[binrw]
pub struct Metadata {
    #[br(map = |offset: u32| offset as usize)]
    #[bw(map = |offset: &usize| *offset as u32)]
    pub group_offset: usize,

    #[br(map = |size: u32| size as usize)]
    #[bw(map = |size: &usize| *size as u32)]
    pub compressed_size: usize,

    #[br(map = |size: u32| size as usize)]
    #[bw(map = |size: &usize| *size as u32)]
    pub decompressed_size: usize,

    #[br(temp)]
    #[bw(calc = {
        let mut val: u32 = 0;
        if *is_standard_zstd { val |= 0x0000_0001 };
        if *is_compressed { val |= 0x0000_0002 };
        if *is_regional_versioned_data { val |= 0x0000_0004 };
        if *is_localized_versioned_data { val |= 0x0000_0008 };
        val
    })]
    flags: u32,

    #[br(calc = flags & 0x0000_0001 != 0)]
    #[bw(ignore)]
    pub is_standard_zstd: bool,

    #[br(calc = flags & 0x0000_0002 != 0)]
    #[bw(ignore)]
    pub is_compressed: bool,

    #[br(calc = flags & 0x0000_0004 != 0)]
    #[bw(ignore)]
    pub is_regional_versioned_data: bool,

    #[br(calc = flags & 0x0000_0008 != 0)]
    #[bw(ignore)]
    pub is_localized_versioned_data: bool,
}

#[binread]
pub struct Patch {
    #[br(map = |version: (u8, u8, u16)| Version::new(version.2 as u64, version.1 as u64, version.0 as u64))]
    pub version: Version,

    pub(crate) file_count: u32,

    #[br(map = |index: u32| TableReference::Unresolved(index as usize))]
    group: TableReference<Group>,

    #[br(temp)]
    file_start: u32,

    pub(crate) lookup_size_in_bytes: u32,

    #[br(temp)]
    _info_count: u32,

    #[br(temp)]
    _descriptor_count: u32,

    #[br(calc = TableContiguousReference::new_from_count(file_start as usize, file_count as usize))]
    pub versioned_files: TableContiguousReference<VersionedFile>,

    #[br(calc = TableContiguousReference::new_from_count(file_start as usize, file_count as usize))]
    pub infos: TableContiguousReference<Info>,

    #[br(map = |num: u32| num as usize)]
    pub num_changed_this_patch: usize,
}

#[binread]
pub struct VersionedFile {
    #[br(temp)]
    path_and_changed: HashKey,

    #[br(calc = path_and_changed.hash())]
    pub path: Hash40,

    #[br(map = |index: u32| TableReference::Unresolved(index as usize))]
    info: TableReference<Info>,

    #[br(temp)]
    _group_index_start: u32,

    #[br(map = |index: u32| TableReference::Unresolved(index as usize))]
    link: TableReference<Link>,

    #[br(calc = path_and_changed.index() == 0)]
    pub changed_this_patch: bool,
}

expose_reference!(ChildPackage, package, Package);
expose_reference!(
    optional,
    Group,
    sub_package,
    Package,
    GroupSubPackageReference,
    Package
);
expose_reference!(
    optional,
    Group,
    sub_package,
    Group,
    GroupSubPackageReference,
    Group
);

expose_reference!(Path, link, Link);
expose_reference!(
    optional,
    Path,
    versioned_file,
    VersionedFile,
    PathVersionedFileReference,
    VersionedFile
);
expose_reference!(Link, owner, Package, LinkOwnerReference, Package);
expose_reference!(Link, owner, Group, LinkOwnerReference, Group);
expose_reference!(Link, info, Info);
expose_reference!(Info, path, Path);
expose_reference!(Info, link, Link);
expose_reference!(Descriptor, group, Group);
expose_reference!(
    optional,
    Descriptor,
    metadata,
    Metadata,
    DescriptorMetadataReference,
    Metadata
);

expose_reference!(Patch, group, Group);
expose_reference!(VersionedFile, info, Info);
expose_reference!(VersionedFile, link, Link);

impl BinRead for DescriptorLoadArguments {
    type Args = ();

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        options: &binrw::ReadOptions,
        _args: Self::Args,
    ) -> binrw::BinResult<Self> {
        let bitfield = u32::read_options(reader, options, ())?;
        match (bitfield & 0xFF00_0000) >> 24 {
            0x00 => Ok(Self::Unowned {
                link: TableReference::Unresolved((bitfield & 0x00FF_FFFF) as usize),
            }),
            0x01 => Ok(Self::Owned {
                patch: DescriptorLoadArgumentsPatchReference::Unresolved(
                    (bitfield & 0x00FF_FFFF) as usize,
                ),
            }),
            0x03 => Ok(Self::PackageSkip {
                info: TableReference::Unresolved((bitfield & 0x00FF_FFFF) as usize),
            }),
            0x05 => Ok(Self::Unknown),
            0x09 => Ok(Self::SharedButOwned {
                link: TableReference::Unresolved((bitfield & 0x00FF_FFFF) as usize),
            }),
            0x10 => Ok(Self::UnsupportedRegion {
                region_locale: (bitfield & 0x00FF_FFFF) as i32,
            }),
            _ => Err(binrw::Error::Custom {
                pos: options.offset(),
                err: Box::new(format!("unsupported bitfield: {:#x}", bitfield)),
            }),
        }
    }
}

impl Package {
    pub(crate) const REPR_SIZE: usize = 0x34;

    pub fn resolve(
        &mut self,
        groups: &[TableCell<Group>],
        infos: &[TableCell<Info>],
        child_packages: &[TableCell<ChildPackage>],
    ) {
        self.groups.resolve(groups);
        self.infos.resolve(infos);
        self.child_packages.resolve(child_packages);
    }

    pub fn is_resolved(&self) -> bool {
        self.groups.is_resolved() && self.infos.is_resolved() && self.child_packages.is_resolved()
    }
}

impl ChildPackage {
    pub(crate) const REPR_SIZE: usize = 0x8;

    pub fn resolve(&mut self, packages: &[TableCell<Package>]) {
        self.package.resolve(packages);
    }

    pub fn is_resolved(&self) -> bool {
        self.package.is_resolved()
    }
}

impl GroupSubPackageReference {
    pub fn resolve(
        &mut self,
        packages: &[TableCell<Package>],
        groups: &[TableCell<Group>],
    ) -> Option<usize> {
        let mut tmp = Self::None;
        std::mem::swap(self, &mut tmp);
        let mut sub_index = None;
        *self = match tmp {
            Self::Unresolved(index) => {
                sub_index = Some(index);
                if index == 0 {
                    Self::None
                } else if index < packages.len() {
                    Self::Package(packages[index].clone())
                } else {
                    Self::Group(groups[index].clone())
                }
            }
            other => other,
        };
        sub_index
    }
}

impl GroupFileReference {
    pub fn resolve(
        &mut self,
        infos: &[TableCell<Info>],
        metadatas: &[TableCell<Metadata>],
        is_info_group: bool,
    ) {
        let mut tmp = Self::Unresolved(INVALID_INDEX..INVALID_INDEX);
        std::mem::swap(self, &mut tmp);
        *self = match tmp {
            Self::Unresolved(range) => {
                if is_info_group {
                    let mut set = TableContiguousReference(TableReferenceSet::Unresolved(range));
                    set.resolve(infos);
                    Self::Info(set)
                } else {
                    let mut set = TableContiguousReference(TableReferenceSet::Unresolved(range));
                    set.resolve(metadatas);
                    Self::Metadata(set)
                }
            }
            other => other,
        }
    }
}

impl Group {
    pub(crate) const REPR_SIZE: usize = 0x1C;

    pub fn resolve(
        &mut self,
        packages: &[TableCell<Package>],
        groups: &[TableCell<Group>],
        infos: &[TableCell<Info>],
        metadatas: &[TableCell<Metadata>],
        self_index: usize,
    ) {
        let is_info_group = if let Some(sub_index) = self.sub_package.resolve(packages, groups) {
            sub_index == 0 || sub_index == self_index
        } else {
            false
        };

        self.files.resolve(infos, metadatas, is_info_group);
    }

    pub fn is_resolved(&self) -> bool {
        self.files.is_resolved() && self.sub_package.is_resolved()
    }

    pub fn is_info_group(&self) -> bool {
        self.files.is_info()
    }

    pub fn is_metadata_group(&self) -> bool {
        self.files.is_metadata()
    }

    pub fn is_version_group(&self) -> bool {
        self.files.is_info() && self.sub_package.is_none()
    }

    pub fn infos(&self) -> &TableContiguousReference<Info> {
        self.files.info()
    }

    pub fn infos_mut(&mut self) -> &mut TableContiguousReference<Info> {
        self.files.info_mut()
    }

    pub fn metadatas(&self) -> &TableContiguousReference<Metadata> {
        self.files.metadata()
    }

    pub fn metadatas_mut(&mut self) -> &mut TableContiguousReference<Metadata> {
        self.files.metadata_mut()
    }
}

impl PathVersionedFileReference {
    pub fn resolve(
        &mut self,
        versioned_files: &[TableCell<VersionedFile>],
        versioned_file_offset: usize,
    ) {
        let mut tmp = Self::None;
        std::mem::swap(self, &mut tmp);
        *self = match tmp {
            Self::Unresolved(index) => {
                Self::VersionedFile(versioned_files[index + versioned_file_offset].clone())
            }
            other => other,
        }
    }
}

impl Path {
    pub(crate) const REPR_SIZE: usize = 0x20;

    pub fn resolve(
        &mut self,
        links: &[TableCell<Link>],
        versioned_files: &[TableCell<VersionedFile>],
        versioned_file_offset: usize,
    ) {
        self.link.resolve(links);
        self.versioned_file
            .resolve(versioned_files, versioned_file_offset);
    }

    pub fn is_resolved(&self) -> bool {
        self.link.is_resolved() && self.versioned_file.is_resolved()
    }

    pub fn has_graphics_archive_extension(&self) -> bool {
        static GRAPHICS_ARCHIVE_EXTENSIONS: &[Hash40] = &[
            Hash40::new("nutexb"),
            Hash40::new("arc"),
            Hash40::new("bntx"),
            Hash40::new("eff"),
        ];

        GRAPHICS_ARCHIVE_EXTENSIONS
            .iter()
            .any(|ext| self.extension == *ext)
    }
}

#[derive(Debug, Error)]
pub enum PathFromStrError {
    #[error("Path should have a file-name")]
    MissingFileName,

    #[error("Path should have an extension")]
    MissingExtension,

    #[error("Path should have a valid parent")]
    MissingParent,
}

impl FromStr for Path {
    type Err = PathFromStrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let path = Utf8Path::new(s);
        let full_path = path.as_str().to_hash();
        let parent = path
            .parent()
            .ok_or(PathFromStrError::MissingParent)?
            .as_str()
            .to_hash();
        let file_name = path
            .file_name()
            .ok_or(PathFromStrError::MissingFileName)?
            .to_hash();
        let extension = path
            .extension()
            .ok_or(PathFromStrError::MissingExtension)?
            .to_hash();
        Ok(Self {
            full_path,
            parent,
            file_name,
            extension,
            link: TableReference::invalid(),
            versioned_file: PathVersionedFileReference::None,
        })
    }
}

impl LinkOwnerReference {
    pub fn resolve(&mut self, packages: &[TableCell<Package>], groups: &[TableCell<Group>]) {
        let mut tmp = Self::Unresolved(INVALID_INDEX);
        std::mem::swap(self, &mut tmp);
        *self = match tmp {
            Self::Unresolved(index) => {
                if index < packages.len() {
                    Self::Package(packages[index].clone())
                } else {
                    Self::Group(groups[index].clone())
                }
            }
            other => other,
        }
    }
}

impl Link {
    pub(crate) const REPR_SIZE: usize = 0x8;

    pub fn new() -> Self {
        Self {
            owner: LinkOwnerReference::invalid(),
            info: TableReference::invalid(),
        }
    }

    pub fn resolve(
        &mut self,
        packages: &[TableCell<Package>],
        groups: &[TableCell<Group>],
        infos: &[TableCell<Info>],
    ) {
        self.owner.resolve(packages, groups);
        self.info.resolve(infos);
    }

    pub fn is_resolved(&self) -> bool {
        self.owner.is_resolved() && self.info.is_resolved()
    }
}

impl Info {
    pub(crate) const REPR_SIZE: usize = 0x10;

    pub fn new() -> Self {
        Self {
            path: TableReference::invalid(),
            link: TableReference::invalid(),
            descriptors: TableContiguousReference::invalid(),
            is_regular_file: true,
            is_graphics_archive: false,
            is_localized: false,
            is_regional: false,
            is_shared: false,
            is_unknown_flag: false,
        }
    }

    pub fn resolve(
        &mut self,
        paths: &[TableCell<Path>],
        links: &[TableCell<Link>],
        descriptors: &[TableCell<Descriptor>],
    ) {
        self.path.resolve(paths);
        self.link.resolve(links);
        self.descriptors.resolve(descriptors);
    }

    pub fn is_resolved(&self) -> bool {
        self.path.is_resolved() && self.link.is_resolved() && self.descriptors.is_resolved()
    }
}

impl DescriptorMetadataReference {
    pub fn resolve(&mut self, metadatas: &[TableCell<Metadata>]) {
        let mut tmp = Self::None;
        std::mem::swap(self, &mut tmp);
        *self = match tmp {
            Self::Unresolved(index) => Self::Metadata(metadatas[index].clone()),
            other => other,
        }
    }
}

impl DescriptorLoadArgumentsPatchReference {
    pub fn resolve(&mut self, patches: &[TableCell<Patch>], is_versioned_descriptor: bool) {
        if !is_versioned_descriptor {
            *self = Self::None;
            return;
        }
        let mut tmp = Self::None;
        std::mem::swap(self, &mut tmp);
        *self = match tmp {
            Self::Unresolved(index) => Self::Patch(patches[index].clone()),
            other => other,
        }
    }
}

impl DescriptorLoadArguments {
    pub fn new() -> Self {
        Self::Owned {
            patch: DescriptorLoadArgumentsPatchReference::None,
        }
    }

    pub fn resolve(
        &mut self,
        links: &[TableCell<Link>],
        infos: &[TableCell<Info>],
        patches: &[TableCell<Patch>],
        info_offset: usize,
        is_versioned_descriptor: bool,
    ) {
        match self {
            Self::Unowned { link } => link.resolve(links),
            Self::Owned { patch } => patch.resolve(patches, is_versioned_descriptor),
            Self::PackageSkip { info } => info.resolve_with_offset(infos, info_offset),
            Self::SharedButOwned { link } => link.resolve(links),
            _ => {}
        }
    }

    pub fn is_resolved(&self) -> bool {
        match self {
            Self::Unowned { link } => link.is_resolved(),
            Self::Owned { patch } => patch.is_resolved(),
            Self::PackageSkip { info } => info.is_resolved(),
            _ => true,
        }
    }
}

impl Descriptor {
    pub(crate) const REPR_SIZE: usize = 0xC;

    pub fn new() -> Self {
        Self {
            group: TableReference::invalid(),
            metadata: DescriptorMetadataReference::None,
            load_args: DescriptorLoadArguments::new(),
        }
    }

    pub fn resolve(
        &mut self,
        groups: &[TableCell<Group>],
        links: &[TableCell<Link>],
        infos: &[TableCell<Info>],
        metadatas: &[TableCell<Metadata>],
        patches: &[TableCell<Patch>],
        info_offset: usize,
        is_versioned_descriptor: bool,
    ) {
        self.group.resolve(groups);
        self.metadata.resolve(metadatas);
        self.load_args
            .resolve(links, infos, patches, info_offset, is_versioned_descriptor);
    }

    pub fn is_resolved(&self) -> bool {
        self.group.is_resolved() && self.metadata.is_resolved() && self.load_args.is_resolved()
    }
}

impl Metadata {
    pub(crate) const REPR_SIZE: usize = 0x10;

    pub fn new() -> Self {
        Self {
            group_offset: usize::MAX,
            compressed_size: usize::MAX,
            decompressed_size: usize::MAX,
            is_standard_zstd: false,
            is_compressed: false,
            is_regional_versioned_data: false,
            is_localized_versioned_data: false,
        }
    }
}

impl Patch {
    pub fn resolve(
        &mut self,
        files: &[TableCell<VersionedFile>],
        infos: &[TableCell<Info>],
        groups: &[TableCell<Group>],
        patch_index: usize,
        info_offset: usize,
    ) {
        self.versioned_files.resolve(files);
        self.infos.resolve_with_offset(infos, info_offset);
        self.group.resolve_with_offset(groups, patch_index);
    }

    pub fn is_resolved(&self) -> bool {
        self.versioned_files.is_resolved() && self.infos.is_resolved() && self.group.is_resolved()
    }
}

impl VersionedFile {
    pub fn resolve(&mut self, links: &[TableCell<Link>], infos: &[TableCell<Info>]) {
        self.link.resolve(links);
        self.info.resolve(infos);
    }
}

impl BinWrite for Package {
    type Args = Rc<PackagedWriter>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        args: Self::Args,
    ) -> binrw::BinResult<()> {
        HashKey::new(
            self.full_path,
            args.groups.get_index(&self.groups.cells()[0]) as usize,
        )
        .write_options(writer, options, ())?;

        self.name.write_options(writer, options, ())?;
        self.parent.write_options(writer, options, ())?;
        self.lifetime.write_options(writer, options, ())?;

        if self.infos.is_empty() {
            0u32.write_options(writer, options, ())?;
            0u32.write_options(writer, options, ())?;
        } else {
            args.infos
                .get_index(&self.infos.cells()[0])
                .write_options(writer, options, ())?;
            (self.infos.len() as u32).write_options(writer, options, ())?;
        }

        if self.child_packages.is_empty() {
            0u32.write_options(writer, options, ())?;
            0u32.write_options(writer, options, ())?;
        } else {
            args.child_packages
                .get_index(&self.child_packages.cells()[0])
                .write_options(writer, options, ())?;
            (self.child_packages.len() as u32).write_options(writer, options, ())?;
        }

        let flags = {
            let mut var = 0u32;
            if self.is_localized {
                var |= 1 << 24
            }
            if self.is_regional {
                var |= 1 << 25
            }
            if self.has_sub_package {
                var |= 1 << 26
            }
            if self.sym_link_is_regional {
                var |= 1 << 27
            }
            if self.is_sym_link {
                var |= 1 << 28
            }
            var
        };

        flags.write_options(writer, options, ())
    }
}

impl BinWrite for ChildPackage {
    type Args = Rc<PackagedWriter>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        args: Self::Args,
    ) -> binrw::BinResult<()> {
        HashKey::new(
            self.full_path,
            args.packages.get_index(self.package.cell()) as usize,
        )
        .write_options(writer, options, ())
    }
}

impl BinWrite for Group {
    type Args = Rc<PackagedWriter>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        args: Self::Args,
    ) -> binrw::BinResult<()> {
        (self.archive_offset as u64).write_options(writer, options, ())?;
        (self.decompressed_size as u32).write_options(writer, options, ())?;
        (self.compressed_size as u32).write_options(writer, options, ())?;

        if self.is_info_group() {
            if self.infos().is_empty() {
                0u32.write_options(writer, options, ())?;
                0u32.write_options(writer, options, ())?;
            } else {
                args.infos
                    .get_index(&self.infos().cells()[0])
                    .write_options(writer, options, ())?;
                (self.infos().len() as u32).write_options(writer, options, ())?;
            }

            if self.is_version_group() {
                0u32.write_options(writer, options, ())?;
            } else {
                args.groups
                    .get_index(self.raw_group())
                    .write_options(writer, options, ())?;
            }
        } else {
            if self.metadatas().is_empty() {
                0u32.write_options(writer, options, ())?;
                0u32.write_options(writer, options, ())?;
            } else {
                args.metadatas
                    .get_index(&self.metadatas().cells()[0])
                    .write_options(writer, options, ())?;
                (self.metadatas().len() as u32).write_options(writer, options, ())?;
            }

            if self.has_group() {
                args.groups
                    .get_index(self.raw_group())
                    .write_options(writer, options, ())?;
            } else if self.has_package() {
                args.packages
                    .get_index(self.raw_package())
                    .write_options(writer, options, ())?;
            } else {
                INVALID_INDEX32.write_options(writer, options, ())?;
            }
        }
        Ok(())
    }
}

impl BinWrite for Path {
    type Args = Rc<PackagedWriter>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        args: Self::Args,
    ) -> binrw::BinResult<()> {
        HashKey::new(
            self.full_path,
            args.links.get_index(self.raw_link()) as usize,
        )
        .write_options(writer, options, ())?;

        let versioned_index = if self.has_versioned_file() {
            args.versioned_files.get_index(self.raw_versioned_file()) as usize
                - args.last_patch_files_start
        } else {
            INVALID_INDEX
        };
        HashKey::new(self.extension, versioned_index).write_options(writer, options, ())?;

        self.parent.write_options(writer, options, ())?;
        self.file_name.write_options(writer, options, ())
    }
}

impl BinWrite for Link {
    type Args = Rc<PackagedWriter>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        args: Self::Args,
    ) -> binrw::BinResult<()> {
        if self.is_owner_package() {
            args.packages
                .get_index(self.raw_package())
                .write_options(writer, options, ())?;
        } else {
            args.groups
                .get_index(self.raw_group())
                .write_options(writer, options, ())?;
        }

        args.infos
            .get_index(self.raw_info())
            .write_options(writer, options, ())
    }
}

impl BinWrite for Info {
    type Args = Rc<PackagedWriter>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        args: Self::Args,
    ) -> binrw::BinResult<()> {
        args.paths
            .get_index(self.raw_path())
            .write_options(writer, options, ())?;
        args.links
            .get_index(self.raw_link())
            .write_options(writer, options, ())?;
        args.descriptors
            .get_index(&self.descriptors.cells()[0])
            .write_options(writer, options, ())?;
        let flags = {
            let mut var = 0u32;
            if self.is_regular_file {
                var |= 1 << 4
            }
            if self.is_graphics_archive {
                var |= 1 << 12
            }
            if self.is_localized {
                var |= 1 << 15
            }
            if self.is_regional {
                var |= 1 << 16
            }
            if self.is_shared {
                var |= 1 << 20
            }
            if self.is_unknown_flag {
                var |= 1 << 21
            }
            var
        };

        flags.write_options(writer, options, ())
    }
}

impl BinWrite for DescriptorLoadArguments {
    type Args = (Rc<PackagedWriter>, usize);

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        args: Self::Args,
    ) -> binrw::BinResult<()> {
        let (pwriter, info_offset) = args;
        let bitfield = match self {
            Self::Unowned { link } => pwriter.links.get_index(link.cell()) & 0x00FF_FFFF,
            Self::Owned { patch } => {
                let patch_id = if patch.is_patch() {
                    pwriter.patches.get_index(patch.patch()) & 0x00FF_FFFF
                } else {
                    0
                };
                0x0100_0000 | patch_id
            }
            Self::PackageSkip { info } => {
                let index = pwriter.infos.get_index(info.cell()) - info_offset as u32;
                0x0300_0000 | index
            }
            Self::Unknown => 0x0500_0000,
            Self::SharedButOwned { link } => {
                let index = pwriter.links.get_index(link.cell()) & 0x00FF_FFFF;
                index | 0x0900_0000
            }
            Self::UnsupportedRegion { region_locale } => 0x1000_0000 | *region_locale as u32,
        };

        bitfield.write_options(writer, options, ())
    }
}

impl BinWrite for Descriptor {
    type Args = (Rc<PackagedWriter>, usize);

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        args: Self::Args,
    ) -> binrw::BinResult<()> {
        let (pwriter, info_offset) = args;
        pwriter
            .groups
            .get_index(self.raw_group())
            .write_options(writer, options, ())?;
        if self.has_metadata() {
            pwriter
                .metadatas
                .get_index(self.raw_metadata())
                .write_options(writer, options, ())?;
        } else {
            INVALID_INDEX32.write_options(writer, options, ())?;
        }
        self.load_args
            .write_options(writer, options, (pwriter, info_offset))
    }
}

impl Patch {
    fn write_header<W>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        pwriter: Rc<PackagedWriter>,
        patch_id: usize,
    ) -> binrw::BinResult<()>
    where
        W: std::io::Write + std::io::Seek,
    {
        (self.version.patch as u8).write_options(writer, options, ())?;
        (self.version.minor as u8).write_options(writer, options, ())?;
        (self.version.major as u16).write_options(writer, options, ())?;
        (self.versioned_files.len() as u32).write_options(writer, options, ())?;
        (pwriter.groups.get_index(self.raw_group()) - patch_id as u32).write_options(
            writer,
            options,
            (),
        )?;
        pwriter
            .versioned_files
            .get_index(&self.versioned_files.cells()[0])
            .write_options(writer, options, ())?;
        let lookup_size = 8 * (self.versioned_files.len() + 0x401);
        (lookup_size as u32).write_options(writer, options, ())?;

        (self.infos.len() as u32).write_options(writer, options, ())?;
        let descriptor_count: usize = self.infos.iter().map(|info| info.descriptors.len()).sum();
        (descriptor_count as u32).write_options(writer, options, ())?;
        (self.group().infos().len() as u32).write_options(writer, options, ())
    }

    fn write_body<W>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        pwriter: Rc<PackagedWriter>,
    ) -> binrw::BinResult<()>
    where
        W: std::io::Write + std::io::Seek,
    {
        let mut lookup = BucketMap::new(NonZeroUsize::new(0x400).unwrap());
        let group_index = pwriter.groups.get_index(self.raw_group()) as usize;
        for (index, file) in self.versioned_files.iter().enumerate() {
            lookup.insert(file.path, index);
            file.write_options(writer, options, (Rc::clone(&pwriter), group_index))?;
        }

        let lookup = lookup.into_inner();
        let total_count: usize = lookup.iter().map(|map| map.len()).sum();
        (lookup.len() as u32).write_options(writer, options, ())?;
        (total_count as u32).write_options(writer, options, ())?;
        let mut total = 0;
        for bucket in lookup.iter() {
            total.write_options(writer, options, ())?;
            total += bucket.len() as u32;
            total.write_options(writer, options, ())?;
        }

        for (key, index) in lookup.into_iter().flat_map(|bucket| bucket.into_iter()) {
            HashKey::new(key, index).write_options(writer, options, ())?;
        }

        Ok(())
    }
}

impl BinWrite for Patch {
    type Args = (Rc<PackagedWriter>, bool, usize);

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        args: Self::Args,
    ) -> binrw::BinResult<()> {
        let (pwriter, is_header, patch_id) = args;
        if is_header {
            self.write_header(writer, options, pwriter, patch_id)
        } else {
            self.write_body(writer, options, pwriter)
        }
    }
}

impl BinWrite for VersionedFile {
    type Args = (Rc<PackagedWriter>, usize);

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        args: Self::Args,
    ) -> binrw::BinResult<()> {
        let (pwriter, group_index) = args;
        HashKey::new(
            self.path,
            if self.changed_this_patch {
                0x0
            } else {
                1 << 24
            },
        )
        .write_options(writer, options, ())?;

        pwriter
            .infos
            .get_index(self.raw_info())
            .write_options(writer, options, ())?;
        (group_index as u32).write_options(writer, options, ())?;
        pwriter
            .links
            .get_index(self.raw_link())
            .write_options(writer, options, ())
    }
}
