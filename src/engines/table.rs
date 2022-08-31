use std::{
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    ops::{Deref, DerefMut, Range},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::INVALID_INDEX;

macro_rules! when_resolved {
    ($self:ident, $resolved:ident, $($t:tt)*) => {{
        if let Self::Resolved($resolved) = $self {
            $($t)*
        } else {
            panic!("Table reference is unresolved")
        }
    }}
}

macro_rules! multi_reference {
    (
        $(#[$outer:meta])*
        $vis:vis enum $Reference:ident : single {
            $(
                $(#[$inner:ident $($args:tt)*])*
                $Variant:ident($T:ty),
            )*
        }
    ) => {
        $(#[$outer])*
        $vis enum $Reference {
            $(
                $(#[$inner $($args)*])*
                $Variant(TableCell<$T>),
            )*
            Unresolved(usize),
        }

        paste::paste! {
            impl $Reference {
                #[doc = "Returns an unresolved, invalid reference\n"]
                pub fn invalid() -> Self {
                    Self::Unresolved(INVALID_INDEX)
                }

                #[doc =
                    "Checks if this reference is resolved\n"
                    "### Returns\n"
                    "Whether or not this reference is resolved."
                ]
                pub fn is_resolved(&self) -> bool {
                    !matches!(self, Self::Unresolved(_))
                }

                $(
                    #[doc =
                        "Checks if this reference is a `" $Variant "` reference.\n"
                        "### Returns\n"
                        "Whether or not this reference is a `" $Variant "` reference.\n"
                        "### Note\n"
                        "The result of this function isn't guaranteed if the reference [is not resolved](" $Reference "::is_resolved)!"
                    ]
                    pub fn [<is_ $Variant:snake>](&self) -> bool {
                        matches!(self, Self::$Variant(_))
                    }
                )*

                $(
                    #[doc =
                        "Gets the underlying `" $Variant "` cell from this reference.\n"
                        "### Returns\n"
                        "The underlying reference to a `" $Variant "`.\n"
                        "### Panicking\n"
                        "* The reference is unresolved\n"
                        "* This is not a reference to a `" $Variant "`"
                    ]
                    pub fn [<$Variant:snake>](&self) -> &TableCell<$T> {
                        match self {
                            Self::$Variant(cell) => cell,
                            Self::Unresolved(_) => panic!("Table reference is unresolved"),
                            _ => panic!("Table reference is not a {}", stringify!($Variant))
                        }
                    }
                )*
            }
        }
    };
    (
        optional,
        $(#[$outer:meta])*
        $vis:vis enum $Reference:ident : single {
            $(
                $(#[$inner:ident $($args:tt)*])*
                $Variant:ident($T:ty),
            )*
        }
    ) => {
        $(#[$outer])*
        $vis enum $Reference {
            None,
            $(
                $(#[$inner $($args)*])*
                $Variant(TableCell<$T>),
            )*
            Unresolved(usize),
        }

        paste::paste! {
            impl $Reference {
                #[doc =
                    "Checks if this reference is resolved\n"
                    "### Returns\n"
                    "Whether or not this reference is resolved."
                ]
                pub fn is_resolved(&self) -> bool {
                    !matches!(self, Self::Unresolved(_))
                }

                #[doc =
                    "Checks if this reference exists\n"
                    "### Returns\n"
                    "Whether or not this reference exists."
                ]
                pub fn is_none(&self) -> bool {
                    matches!(self, Self::None)
                }

                $(
                    #[doc =
                        "Checks if this reference is a `" $Variant "` reference.\n"
                        "### Returns\n"
                        "Whether or not this reference is a `" $Variant "` reference.\n"
                        "### Note\n"
                        "The result of this function isn't guaranteed if the reference [is not resolved](" $Reference "::is_resolved)!"
                    ]
                    pub fn [<is_ $Variant:snake>](&self) -> bool {
                        matches!(self, Self::$Variant(_))
                    }
                )*

                $(
                    #[doc =
                        "Gets the underlying `" $Variant "` cell from this reference.\n"
                        "### Returns\n"
                        "The underlying reference to a `" $Variant "`.\n"
                        "### Panicking\n"
                        "* The reference is unresolved\n"
                        "* This is not a reference to a `" $Variant "`"
                    ]
                    pub fn [<$Variant:snake>](&self) -> &TableCell<$T> {
                        match self {
                            Self::$Variant(cell) => cell,
                            Self::Unresolved(_) => panic!("Table reference is unresolved"),
                            _ => panic!("Table reference is not a {}", stringify!($Variant))
                        }
                    }
                )*
            }
        }
    };
    (
        $(#[$outer:meta])*
        $vis:vis enum $Reference:ident : set {
            $(
                $(#[$inner:ident $($args:tt)*])*
                $Variant:ident($T:ty),
            )*
        }
    ) => {
        $(#[$outer])*
        $vis enum $Reference {
            $(
                $(#[$inner $($args)*])*
                $Variant(TableContiguousReference<$T>),
            )*
            Unresolved(Range<usize>),
        }

        paste::paste! {
            impl $Reference {
                #[doc =
                    "Checks if this reference is resolved\n"
                    "### Returns\n"
                    "Whether or not this reference is resolved."
                ]
                pub fn is_resolved(&self) -> bool {
                    !matches!(self, Self::Unresolved(_))
                }

                #[doc = "Returns an unresolved, invalid reference\n"]
                pub fn invalid() -> Self {
                    Self::Unresolved(INVALID_INDEX..INVALID_INDEX)
                }

                $(
                    #[doc =
                        "Checks if this reference is a `" $Variant "` reference.\n"
                        "### Returns\n"
                        "Whether or not this reference is a `" $Variant "` reference.\n"
                        "### Note\n"
                        "The result of this function isn't guaranteed if the reference [is not resolved](" $Reference "::is_resolved)!"
                    ]
                    pub fn [<is_ $Variant:snake>](&self) -> bool {
                        matches!(self, Self::$Variant(_))
                    }
                )*

                $(
                    #[doc =
                        "Gets the underlying `" $Variant "` set from this reference.\n"
                        "### Returns\n"
                        "The underlying reference to a `" $Variant "`.\n"
                        "### Panicking\n"
                        "* The reference is unresolved\n"
                        "* This is not a reference to a `" $Variant "`"
                    ]
                    pub fn [<$Variant:snake>](&self) -> &TableContiguousReference<$T> {
                        match self {
                            Self::$Variant(set) => set,
                            Self::Unresolved(_) => panic!("Table reference is unresolved"),
                            _ => panic!("Table reference is not a {}", stringify!($Variant))
                        }
                    }

                    #[doc =
                        "Gets the mutable reference to the underlying `" $Variant "` set from this reference\n"
                        "### Panicking\n"
                        "* The reference is unresolved\n"
                        "* There is already another mutable reference active\n"
                        "* This is not a reference to a `" $Variant "`"
                    ]
                    pub fn [<$Variant:snake _mut>](&mut self) -> &mut TableContiguousReference<$T> {
                        match self {
                            Self::$Variant(set) => set,
                            Self::Unresolved(_) => panic!("Table reference is unresolved"),
                            _ => panic!("Table reference is not a {}", stringify!($Variant))
                        }
                    }
                )*
            }
        }
    };
    (
        optional,
        $(#[$outer:meta])*
        $vis:vis enum $Reference:ident : set {
            $(
                $(#[$inner:ident $($args:tt)*])*
                $Variant:ident($T:ty),
            )*
        }
    ) => {
        $(#[$outer])*
        $vis enum $Reference {
            None,
            $(
                $(#[$inner $($args)*])*
                $Variant(TableContiguousReference<$T, Range<usize>>),
            )*
            Unresolved(Range<usize>),
        }

        paste::paste! {
            impl $Reference {
                #[doc =
                    "Checks if this reference is resolved\n"
                    "### Returns\n"
                    "Whether or not this reference is resolved."
                ]
                pub fn is_resolved(&self) -> bool {
                    !matches!(self, Self::Unresolved(_))
                }

                #[doc =
                    "Checks if this reference exists\n"
                    "### Returns\n"
                    "Whether or not this reference exists."
                ]
                pub fn is_none(&self) -> bool {
                    matches!(self, Self::None)
                }

                $(
                    #[doc =
                        "Checks if this reference is a `" $Variant "` reference.\n"
                        "### Returns\n"
                        "Whether or not this reference is a `" $Variant "` reference.\n"
                        "### Note\n"
                        "The result of this function isn't guaranteed if the reference [is not resolved](" $Reference "::is_resolved)!"
                    ]
                    pub fn [<is_ $Variant:snake>](&self) -> bool {
                        matches!(self, Self::$Variant(_))
                    }
                )*

                $(
                    #[doc =
                        "Gets the underlying `" $Variant "` cell from this reference.\n"
                        "### Returns\n"
                        "The underlying reference to a `" $Variant "`.\n"
                        "### Panicking\n"
                        "* The reference is unresolved\n"
                        "* This is not a reference to a `" $Variant "`"
                    ]
                    pub fn [<$Variant:snake>](&self) -> &TableContiguousReference<$T> {
                        match self {
                            Self::$Variant(set) => set,
                            Self::Unresolved(_) => panic!("Table reference is unresolved"),
                            _ => panic!("Table reference is not a {}", stringify!($Variant))
                        }
                    }
                )*
            }
        }
    };

}

use binrw::{BinRead, BinWrite};
pub(crate) use multi_reference;

macro_rules! expose_reference {
    ($Structure:ident, $RefField:ident, $RefType:ty) => {
        paste::paste! {
            impl $Structure {
                #[doc =
                    "Gets an immutable reference to the " $RefField " for this `" $Structure "`\n"
                    "### Returns\n"
                    "The immutable reference to the " $RefField " for this `" $Structure "`\n"
                    "### Panicking\n"
                    "This function panics if the [table reference](TableReference) to the " $RefField
                    " is not [resolved](TableReference::resolve)."
                ]
                pub fn $RefField(&self) -> Ref<'_, $RefType> {
                    self.$RefField.cell().get()
                }

                #[doc =
                    "Gets the mutable reference to the " $RefField " for this `" $Structure "`\n"
                    "### Returns\n"
                    "The mutable reference to the " $RefField " for this `" $Structure "`\n"
                    "### Panicking\n"
                    "This function panics if the [table reference](TableReference) to the " $RefField
                    " is not [resolved](TableReference::resolve)."
                ]
                pub fn [<$RefField _mut>](&self) -> RefMut<'_, $RefType> {
                    self.$RefField.cell().get_mut()
                }

                #[doc =
                    "Sets the reference for this `" $Structure "`'s " $RefField " to the specified cell\n"
                    "### Arguments\n"
                    "* `cell` - The cell to set the reference to\n"
                ]
                pub fn [<set_ $RefField>](&mut self, cell: TableCell<$RefType>) {
                    self.$RefField = TableReference::Resolved(cell);
                }

                #[doc =
                    "Gets an immutable reference to the underlying cell for this `" $Structure "`'s " $RefField "\n"
                    "### Returns\n"
                    "An immutable reference to the underlying cell"
                    "### Panicking\n"
                    "This function panics if the [table reference](TableReference) to the " $RefField
                    " is not [resolved](TableReference::resolve)."
                ]
                pub fn [<raw_ $RefField>](&self) -> &TableCell<$RefType> {
                    self.$RefField.cell()
                }
            }
        }
    };
    ($Structure:ident, $RefField:ident, $RefType:ty, $RefName:ident, $RefVariant:ident) => {
        paste::paste! {
            impl $Structure {
                #[doc =
                    "Gets an immutable reference to the " $RefVariant " for this `" $Structure "`\n"
                    "### Returns\n"
                    "The immutable reference to the " $RefVariant " for this `" $Structure "`\n"
                    "### Panicking\n"
                    "This function panics if the [table reference](TableReference) to the " $RefVariant
                    " is not [resolved](TableReference::resolve)."
                ]
                pub fn [<$RefVariant:snake>](&self) -> Ref<'_, $RefType> {
                    self.$RefField.[<$RefVariant:snake>]().get()
                }

                #[doc =
                    "Gets the mutable reference to the " $RefVariant " for this `" $Structure "`\n"
                    "### Returns\n"
                    "The mutable reference to the " $RefVariant " for this `" $Structure "`\n"
                    "### Panicking\n"
                    "This function panics if the [table reference](TableReference) to the " $RefVariant
                    " is not [resolved](TableReference::resolve)."
                ]
                pub fn [<$RefVariant:snake _mut>](&self) -> RefMut<'_, $RefType> {
                    self.$RefField.[<$RefVariant:snake>]().get_mut()
                }

                #[doc =
                    "Sets the reference for this `" $Structure "`'s " $RefVariant " to the specified cell\n"
                    "### Arguments\n"
                    "* `cell` - The cell to set the reference to\n"
                ]
                pub fn [<set_ $RefVariant:snake>](&mut self, cell: TableCell<$RefType>) {
                    self.$RefField = $RefName::$RefVariant(cell);
                }

                #[doc =
                    "Gets an immutable reference to the underlying cell for this `" $Structure "`'s " $RefVariant "\n"
                    "### Returns\n"
                    "An immutable reference to the underlying cell"
                    "### Panicking\n"
                    "This function panics if the [table reference](TableReference) to the " $RefVariant
                    " is not [resolved](TableReference::resolve)."
                ]
                pub fn [<raw_ $RefVariant:snake>](&self) -> &TableCell<$RefType> {
                    self.$RefField.[<$RefVariant:snake>]()
                }

                #[doc =
                    "Checks if the " $RefField " of this `" $Structure "` is a " $RefVariant "\n"
                ]
                pub fn [<is_ $RefField _ $RefVariant:snake>](&self) -> bool {
                    self.$RefField.[<is_ $RefVariant:snake>]()
                }
            }
        }
    };
    (optional, $Structure:ident, $RefField:ident, $RefType:ty, $RefName:ident, $RefVariant:ident) => {
        paste::paste! {
            impl $Structure {
                #[doc =
                    "Gets an immutable reference to the " $RefVariant " for this `" $Structure "`\n"
                    "### Returns\n"
                    "The immutable reference to the " $RefVariant " for this `" $Structure "`\n"
                    "### Panicking\n"
                    "This function panics if the [table reference](TableReference) to the " $RefVariant
                    " is not [resolved](TableReference::resolve)."
                ]
                pub fn [<$RefVariant:snake>](&self) -> Ref<'_, $RefType> {
                    self.$RefField.[<$RefVariant:snake>]().get()
                }

                #[doc =
                    "Gets the mutable reference to the " $RefVariant " for this `" $Structure "`\n"
                    "### Returns\n"
                    "The mutable reference to the " $RefVariant " for this `" $Structure "`\n"
                    "### Panicking\n"
                    "This function panics if the [table reference](TableReference) to the " $RefVariant
                    " is not [resolved](TableReference::resolve)."
                ]
                pub fn [<$RefVariant:snake _mut>](&self) -> RefMut<'_, $RefType> {
                    self.$RefField.[<$RefVariant:snake>]().get_mut()
                }

                #[doc =
                    "Sets the reference for this `" $Structure "`'s " $RefVariant " to the specified cell\n"
                    "### Arguments\n"
                    "* `cell` - The cell to set the reference to\n"
                ]
                pub fn [<set_ $RefVariant:snake>](&mut self, cell: TableCell<$RefType>) {
                    self.$RefField = $RefName::$RefVariant(cell);
                }

                #[doc =
                    "Gets an immutable reference to the underlying cell for this `" $Structure "`'s " $RefVariant "\n"
                    "### Returns\n"
                    "An immutable reference to the underlying cell"
                    "### Panicking\n"
                    "This function panics if the [table reference](TableReference) to the " $RefVariant
                    " is not [resolved](TableReference::resolve)."
                ]
                pub fn [<raw_ $RefVariant:snake>](&self) -> &TableCell<$RefType> {
                    self.$RefField.[<$RefVariant:snake>]()
                }

                #[doc =
                    "Checks if the reference for this `" $Structure "`'s " $RefVariant " is valid\n"
                    "### Returns\n"
                    "Whether or not this `" $Structure "` holds a reference to a " $RefVariant "\n"
                ]
                pub fn [<has_ $RefVariant:snake>](&self) -> bool {
                    self.$RefField.[<is_ $RefVariant:snake>]()
                }
            }
        }
    }
}

pub(crate) use expose_reference;

/// A reference counting, globally unique cell of data in the archive
///
/// The archive format has many tables which cross-reference to each other, most
/// notably in the packaged filesystem. Using reference-counted cells with interior
/// mutability makes accessing and modifying the data significantly easier,
/// with the slight overhead of runtime borrow-checking.
///
/// Each cell comes with a global identifier, which is unique from every other cell
/// active in any engine. Since these cells are only active for the lifetime of the
/// archive, there is no need to use actual UUIDs and instead a static, atomic
/// counter is used to speed up creation of cells.
pub struct TableCell<T: Sized> {
    cell: Rc<RefCell<T>>,
    guid: u64,
}

impl<T: Sized> TableCell<T> {
    /// Constructs a new table cell from the provided data
    ///
    /// ### Arguments
    /// * `data` - The data to fill the cell with
    ///
    /// ### Returns
    /// The constructed cell
    pub fn new(data: T) -> Self {
        // Static atomic for a counter without unsafe
        static GUID: AtomicU64 = AtomicU64::new(0);

        Self {
            cell: Rc::new(RefCell::new(data)),
            guid: GUID.fetch_add(1, Ordering::SeqCst),
        }
    }

    /// Retrieves the GUID of this cell
    ///
    /// ### Returns
    /// The GUID of the cell
    pub fn guid(&self) -> u64 {
        self.guid
    }

    /// Gets the number of users which are referencing this cell
    ///
    /// ### Returns
    /// The number of users referencing this cell
    ///
    /// ### Note
    /// This only takes into account the [strong count](Rc::strong_count) of
    /// the internal cell
    pub fn rc(&self) -> usize {
        Rc::strong_count(&self.cell)
    }

    /// Gets an immutable reference to the underlying data of the cell
    ///
    /// ### Returns
    /// An immutable reference to the underlying data of the cell
    ///
    /// ### Panicking
    /// This method panics if there is currently a mutable reference of the data
    pub fn get(&self) -> Ref<'_, T> {
        self.cell.borrow()
    }

    /// Gets a mutable reference to the underlying data of the cell
    ///
    /// ### Returns
    /// A mutable reference to the underlying data of the cell
    ///
    /// ### Panicking
    /// This method panics if there is currently a mutable reference of the data
    pub fn get_mut(&self) -> RefMut<'_, T> {
        self.cell.borrow_mut()
    }
}

impl<T: Sized> Clone for TableCell<T> {
    fn clone(&self) -> Self {
        Self {
            cell: Rc::clone(&self.cell),
            guid: self.guid,
        }
    }
}

impl<T: Sized + BinRead> BinRead for TableCell<T> {
    type Args = T::Args;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        options: &binrw::ReadOptions,
        args: Self::Args,
    ) -> binrw::BinResult<Self> {
        T::read_options(reader, options, args).map(TableCell::new)
    }
}

impl<T: Sized + BinWrite> BinWrite for TableCell<T> {
    type Args = T::Args;
    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        options: &binrw::WriteOptions,
        args: Self::Args,
    ) -> binrw::BinResult<()> {
        self.get().write_options(writer, options, args)
    }
}

/// A potentially unresolved reference to an archive structure
///
/// Table references are used when parsing archive structures
/// as not everything has been loaded yet. This enables those elements
/// to be loaded into whole tables and, upon resolving, will make
/// accessing the structures related to one another much easier.
pub enum TableReference<T: Sized> {
    /// A resolved reference, an element of one of the archive's tables.
    Resolved(TableCell<T>),

    /// An unresolved reference, which is used to index into one of the
    /// archive's tables.
    Unresolved(usize),
}

impl<T: Sized> TableReference<T> {
    pub fn invalid() -> Self {
        Self::Unresolved(INVALID_INDEX)
    }

    /// Resolves the reference if it is currently unresolved
    ///
    /// ### Arguments
    /// * `table` - An array of [`TableCell`] from which this reference,
    /// if unresolved, can safely index into.
    ///
    /// ### Panicking
    /// This method panics if the unresolved index is out-of-bounds of
    /// the provided slice.
    pub fn resolve(&mut self, table: &[TableCell<T>]) {
        // Create a temporary value which we can swap with `self`
        // so that we can match and assign contents which cannot copy
        // (and so that we don't clone when unnecessary)
        let mut tmp = Self::Unresolved(INVALID_INDEX);
        std::mem::swap(self, &mut tmp);
        *self = match tmp {
            Self::Unresolved(index) => Self::Resolved(table[index].clone()),
            other => other,
        }
    }

    /// Resolves the reference if it is currently unresolved, adding
    /// the specified offset to the unresolved index
    ///
    /// ### Arguments
    /// * `table` - An Array of [`TableCell`] from which this reference,
    /// if unresolved, can safely index into.
    /// * `offset` - The offset to add to the unresolved index
    ///
    /// ### Panicking
    /// This method panics if the sum of the unresolved index and the offset
    /// is out-of-bounds of the provided slice.
    ///
    /// ### Usages
    /// Sometimes information about where the reference is pointing to is
    /// not available until after the whole table has been loaded into memory.
    /// Use this method over [`resolve`](TableReference::resolve) when that
    /// is the case.
    pub fn resolve_with_offset(&mut self, table: &[TableCell<T>], offset: usize) {
        // Create a temporary value which we can swap with `self`
        // so that we can match and assign contents which cannot copy
        // (and so that we don't clone when unnecessary)
        let mut tmp = Self::Unresolved(INVALID_INDEX);
        std::mem::swap(self, &mut tmp);
        *self = match tmp {
            Self::Unresolved(index) => Self::Resolved(table[index + offset].clone()),
            other => other,
        }
    }

    /// Checks if the reference is already resolved
    ///
    /// ### Returns
    /// Whether or not the reference is already resolved.
    pub fn is_resolved(&self) -> bool {
        matches!(self, Self::Resolved(_))
    }

    /// Gets the resolved cell from this reference, panicking if the reference
    /// is not resolved
    ///
    /// ### Returns
    /// An immutable reference to the resolved [`TableCell`].
    ///
    /// ### Panicking
    /// This function panics if the reference is not yet resolved.
    pub fn cell(&self) -> &TableCell<T> {
        when_resolved!(self, cell, cell)
    }
}

/// A set of structures referenced by a single item
///
/// Some times a structure could be considered a "container",
/// or that it references more than one of an item at a time
/// via the same, unresolved index.
///
/// This is a generic implementation to be expanded upon
/// by any set-like container/reference in the tables.
pub enum TableReferenceSet<T: Sized, U: Sized> {
    Resolved(Vec<TableCell<T>>),
    Unresolved(U),
}

impl<T: Sized, U: Sized> TableReferenceSet<T, U> {
    /// Checks if the reference is already resolved
    ///
    /// ### Returns
    /// Whether or not the reference is already resolved.
    pub fn is_resolved(&self) -> bool {
        matches!(self, Self::Resolved(_))
    }

    /// Gets the number of cells in this reference set
    ///
    /// ### Returns
    /// The number of cells in this reference set
    ///
    /// ### Panicking
    /// This function will panic if the reference set is not yet resolved
    pub fn len(&self) -> usize {
        when_resolved!(self, set, set.len())
    }

    /// Checks if the number of cells in this reference set is `0`
    ///
    /// ### Returns
    /// Whether or not there are no cells in this reference set
    ///
    /// ### Panicking
    /// This function will panic if the reference set is not yet resolved
    pub fn is_empty(&self) -> bool {
        when_resolved!(self, set, set.is_empty())
    }

    /// Adds a new cell to this reference set
    ///
    /// ### Arguments
    /// * `cell` - The cell to add to the set
    ///
    /// ### Panicking
    /// This function will panic if the reference set is not yet resolved
    pub fn push(&mut self, cell: TableCell<T>) {
        when_resolved!(self, set, set.push(cell))
    }

    /// Inserts a new cell to this reference set at an
    /// arbitrary index
    ///
    /// ### Arguments
    /// * `index` - The index where `cell` should be inserted
    /// * `cell` - The cell to insert into the set
    ///
    /// ### Panicking
    /// * Panics if `index` > `len()`
    /// * Panics if the reference set is not yet resolved
    pub fn insert(&mut self, index: usize, cell: TableCell<T>) {
        when_resolved!(self, set, set.insert(index, cell))
    }

    /// Removes all references in this set
    ///
    /// ### Panicking
    /// This function will panic if the reference set is not yet resolved
    pub fn clear(&mut self) {
        when_resolved!(self, set, set.clear())
    }

    /// Creates an iterator of immutable references to every cell
    /// in this reference set
    ///
    /// ### Returns
    /// An iterator over each cell, returning an immutable reference
    /// to the data contained within
    ///
    /// ### Panicking
    /// This function will panic if the reference set is not yet resolved
    pub fn iter(&self) -> impl Iterator<Item = Ref<'_, T>> {
        when_resolved!(self, set, set.iter().map(TableCell::get))
    }

    /// Creates an iterator of mutable references to every cell
    /// in this reference set
    ///
    /// ### Returns
    /// An iterator over each cell, returning a mutable reference
    /// to the data contained within
    ///
    /// ### Panicking
    /// This function will panic if the reference set is not yet resolved
    pub fn iter_mut(&self) -> impl Iterator<Item = RefMut<'_, T>> {
        when_resolved!(self, set, set.iter().map(TableCell::get_mut))
    }

    /// Gets the underlying cells of this reference set
    ///
    /// ### Returns
    /// The slice of cells for this reference set
    ///
    /// ### Panicking
    /// This function will panic if the reference set is not yet resolved
    pub fn cells(&self) -> &[TableCell<T>] {
        when_resolved!(self, set, set.as_slice())
    }

    pub fn cells_mut(&mut self) -> &mut [TableCell<T>] {
        when_resolved!(self, set, set.as_mut_slice())
    }

    pub fn replace(&mut self, set: Vec<TableCell<T>>) {
        *self = TableReferenceSet::Resolved(set);
    }

    pub fn get(&self, index: usize) -> Ref<'_, T> {
        when_resolved!(self, set, set[index].get())
    }

    pub fn get_mut(&self, index: usize) -> RefMut<'_, T> {
        when_resolved!(self, set, set[index].get_mut())
    }
}

/// A reference to a set of archive structures which are consecutive
///
/// Lots of collections in the archive format refer to consecutive,
/// contiguous entries. This collections aids in resolving those
/// collections.
///
/// It implements [`Deref`] and [`DerefMut`] on [`TableReferenceSet`] for
/// simple access to all of the cells.
#[repr(transparent)]
pub struct TableContiguousReference<T: Sized>(pub(crate) TableReferenceSet<T, Range<usize>>);

impl<T: Sized> TableContiguousReference<T> {
    pub fn invalid() -> Self {
        Self(TableReferenceSet::Unresolved(INVALID_INDEX..INVALID_INDEX))
    }

    /// Constructs a new, unresolved reference set from the start index and the number of elements
    ///
    /// ### Arguments
    /// * `start` - The starting index of the references
    /// * `count` - The number of elements in the set
    ///
    /// ### Returns
    /// The unresolved reference
    pub fn new_from_count(start: usize, count: usize) -> Self {
        Self(TableReferenceSet::Unresolved(start..(start + count)))
    }

    /// Resolves the reference if it is currently unresolved
    ///
    /// ### Arguments
    /// * `table` - An array of [`TableCell`] from which this reference,
    /// if unresolved, can safely index into.
    ///
    /// ### Panicking
    /// This method panics if the unresolved range is out-of-bounds of
    /// the provided slice.
    pub fn resolve(&mut self, table: &[TableCell<T>]) {
        // Create a temporary value which we can swap with `self`
        // so that we can match and assign contents which cannot copy
        // (and so that we don't clone when unnecessary)
        let mut tmp = Self(TableReferenceSet::Unresolved(INVALID_INDEX..INVALID_INDEX));
        std::mem::swap(self, &mut tmp);
        *self = match tmp {
            Self(TableReferenceSet::Unresolved(range)) => Self(TableReferenceSet::Resolved(
                table[range].iter().map(TableCell::clone).collect(),
            )),
            other => other,
        }
    }

    /// Resolves the reference if it is currently unresolved
    ///
    /// ### Arguments
    /// * `table` - An array of [`TableCell`] from which this reference,
    /// if unresolved, can safely index into.
    /// * `offset` - The offset to add to the unresolved range
    ///
    /// ### Panicking
    /// This method panics if the sum of the unresolved range and the offset
    /// is out-of-bounds of the provided slice.
    ///
    /// ### Usages
    /// Sometimes information about where the reference is pointing to is
    /// not available until after the whole table has been loaded into memory.
    /// Use this method over [`resolve`](TableContiguousReference::resolve) when that
    /// is the case.
    pub fn resolve_with_offset(&mut self, table: &[TableCell<T>], offset: usize) {
        // Create a temporary value which we can swap with `self`
        // so that we can match and assign contents which cannot copy
        // (and so that we don't clone when unnecessary)
        let mut tmp = Self(TableReferenceSet::Unresolved(INVALID_INDEX..INVALID_INDEX));
        std::mem::swap(self, &mut tmp);
        *self = match tmp {
            Self(TableReferenceSet::Unresolved(range)) => Self(TableReferenceSet::Resolved(
                table[(range.start + offset)..(range.end + offset)]
                    .iter()
                    .map(TableCell::clone)
                    .collect(),
            )),
            other => other,
        }
    }
}

impl<T: Sized> Deref for TableContiguousReference<T> {
    type Target = TableReferenceSet<T, Range<usize>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Sized> DerefMut for TableContiguousReference<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Trait for locating the next cell in a linked-list reference set
///
/// Some sets of data in the archive format are linked together via
/// a linked-list like method, which requires one element to point
/// to the next one, and if there is no next element the set is over.
///
/// This trait is required to be implemented on any structure which
/// is an element of one of these sets.
pub trait LinkedReference: Sized {
    fn next(&self) -> Option<TableCell<Self>>;
}

/// A reference to a set of archive structures which are joined via a linked-list
///
/// Some collections in the archive format iterate through their children via
/// a linked-list like method, which requires one element to point to the next one.
/// If there is no next element, the set is over.
///
/// This reference type serves to more easily collect and represent those elements,
/// and it has the added benefit of not requiring the user to iterate over the linked
/// list each time.
///
/// Unlike it's [contiguous counterpart](TableContiguousReference), this
/// reference set only requires the start index of the set in order to resolve.
///
/// Note that any type which is referenced by this type must implement [`LinkedReference`]
#[repr(transparent)]
pub struct TableLinkedReference<T: LinkedReference>(TableReferenceSet<T, usize>);

impl<T: LinkedReference> TableLinkedReference<T> {
    pub fn invalid() -> Self {
        Self::new(INVALID_INDEX)
    }

    /// Helper method for creating a new unresolved reference set
    ///
    /// ### Arguments
    /// * `start` - The starting unresolved index
    ///
    /// ### Returns
    /// The unresolved reference set
    pub fn new(start: usize) -> Self {
        Self(TableReferenceSet::Unresolved(start))
    }

    /// Resolves the reference if it is currently unresolved
    ///
    /// ### Arguments
    /// * `table` - An array of [`TableCell`] from which this reference,
    /// if unresolved, can safely index into.
    ///
    /// ### Panicking
    /// This method panics if the unresolved index is out-of-bounds of
    /// the provided slice.
    ///
    /// ### Notes
    /// * This function makes no checks on the data received from `table` to
    /// see if the referenced cell(s) are already resolved.
    /// * There is no option for resolving with an index, as that doesn't make
    /// sense for a linked-list based reference set.
    pub fn resolve(&mut self, table: &[TableCell<T>]) {
        // Create a temporary value which we can swap with `self`
        // so that we can match and assign contents which cannot copy
        // (and so that we don't clone when unnecessary)
        let mut tmp = Self(TableReferenceSet::Unresolved(INVALID_INDEX));
        std::mem::swap(self, &mut tmp);
        *self = match tmp {
            Self(TableReferenceSet::Unresolved(start_index)) => {
                // Get the start of our references, this is the only one
                // that we directly index
                let mut current = table[start_index].clone();
                let mut set = vec![current.clone()];

                loop {
                    // Continuously attempt to get the next element
                    // in the set, breaking out if there is not one
                    let next = if let Some(next) = current.get().next() {
                        next
                    } else {
                        break;
                    };

                    // Push the element and then change our current element
                    set.push(next.clone());
                    current = next;
                }
                Self(TableReferenceSet::Resolved(set))
            }
            other => other,
        }
    }
}

impl<T: LinkedReference> Deref for TableLinkedReference<T> {
    type Target = TableReferenceSet<T, usize>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: LinkedReference> DerefMut for TableLinkedReference<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Helper structure for reconstructing tables
///
/// When reconstructing tables as intended for the archive,
/// it is important to make sure the elements are properly
/// set up in the proper reference chains. This structure
/// provides an encapsulated way of building new tables using
/// only the cells and also maintaining a lookup of their GUID
/// to the table index for use in serializing out the structure
/// to their raw form.
pub struct TableMaker<T: Sized> {
    table: Vec<TableCell<T>>,
    guid_to_index: HashMap<u64, u32>,
}

impl<T: Sized> TableMaker<T> {
    /// Constructs a new [`TableMaker`] with empty containers
    ///
    /// ### Returns
    /// The constructed [`TableMaker`]
    pub fn new() -> Self {
        Self {
            table: Vec::new(),
            guid_to_index: HashMap::new(),
        }
    }

    /// Pushes a cell to the new table
    ///
    /// ### Arguments
    /// * `cell` - The cell to add to the table
    ///
    /// ### Panicking
    /// This function panics if the cell is already present in the table
    pub fn push(&mut self, cell: TableCell<T>) {
        let index = self.table.len() as u32;
        if self.guid_to_index.insert(cell.guid(), index).is_some() {
            panic!("Overlapping GUIDs in index lookup!");
        }
        self.table.push(cell);
    }

    /// Checks if the table contains the cell already
    ///
    /// ### Arguments
    /// * `cell` - A reference to the cell to check for
    ///
    /// ### Returns
    /// Whether or not the cell is already in the table
    pub fn has_cell(&self, cell: &TableCell<T>) -> bool {
        self.guid_to_index.contains_key(&cell.guid())
    }

    /// Gets the index of the cell in the table
    ///
    /// ### Arguments
    /// * `cell` - A reference to the cell to get the index for
    ///
    /// ### Returns
    /// The index of the cell in the table
    ///
    /// ### Panicking
    /// This function panics if the cell is not in the table
    pub fn get_index(&self, cell: &TableCell<T>) -> u32 {
        self.guid_to_index
            .get(&cell.guid())
            .copied()
            .expect("Index lookup should have index for GUID")
    }

    /// Gets an iterator over each cell in the table
    ///
    /// ### Returns
    /// The iterator over the cells
    pub fn iter(&self) -> impl Iterator<Item = &TableCell<T>> {
        self.table.iter()
    }

    pub fn into_inner(self) -> Vec<TableCell<T>> {
        self.table
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }
}

impl<T: Sized> Default for TableMaker<T> {
    fn default() -> Self {
        Self::new()
    }
}
