//! # Database
//! Local file database utilities.
//!
//! This crate gives you a simple way to manage files and folders inside one database directory.
//! The main type is **`DatabaseManager`**, and items are addressed with **`ItemId`**.
//!
//! ## How `ItemId` works
//! - **`ItemId`** has a `name` and an `index`.
//! - `name` is the shared key (for example `"test_file.txt"`).
//! - `index` picks which path you want when that name exists more than once.
//! - `ItemId::id("name")` always means index `0`.
//! - `ItemId::database_id()` is the root ID for the database itself.
//!
//! This means duplicate names are allowed, and you can still target one exact item by index.
//!
//! # Example: Build `ItemId` values
//! ```
//! use file_database::ItemId;
//!
//! let first = ItemId::id("test_file.txt");
//! let second = ItemId::with_index("test_file.txt", 1);
//! let root = ItemId::database_id();
//!
//! assert_eq!(first.get_name(), "test_file.txt");
//! assert_eq!(first.get_index(), 0);
//! assert_eq!(second.get_index(), 1);
//! assert_eq!(root.get_name(), "");
//! assert_eq!(root.get_index(), 0);
//! ```
//!
//! ## How to use **`GenPath`**
//! **`GenPath`** is used to generate paths from a ruleset. This is primarily used as the root directory for **`DatabaseManager`**.
//! Pick the method based on where your app starts:
//! - `GenPath::from_working_dir(steps)` when your process starts in a useful working directory.
//! - `GenPath::from_exe(steps)` when paths should be anchored to the executable location.
//! - `GenPath::from_closest_match("name")` when you want to find the nearest matching folder
//!   while walking upward from the executable.
//!
//! # Example: Build base paths with `GenPath`
//! ```no_run
//! use file_database::{DatabaseError, GenPath};
//!
//! fn main() -> Result<(), DatabaseError> {
//!     let from_cwd = GenPath::from_working_dir(0)?;
//!     let from_exe = GenPath::from_exe(0)?;
//!     assert!(from_cwd.is_absolute() || from_cwd.is_relative());
//!     assert!(from_exe.is_absolute() || from_exe.is_relative());
//!     Ok(())
//! }
//! ```
//!
//! # Example: Find a folder by name with `GenPath`
//! ```no_run
//! use file_database::{DatabaseError, GenPath};
//!
//! fn main() -> Result<(), DatabaseError> {
//!     let project_root = GenPath::from_closest_match("src")?;
//!     assert!(project_root.ends_with("src"));
//!     Ok(())
//! }
//! ```
//!
//! # Example: Create and overwrite a file
//! ```no_run
//! use file_database::{DatabaseError, DatabaseManager, ItemId};
//!
//! fn main() -> Result<(), DatabaseError> {
//!     let mut manager = DatabaseManager::new(".", "database")?;
//!     manager.write_new(ItemId::id("example.txt"), ItemId::database_id())?;
//!     manager.overwrite_existing(ItemId::id("example.txt"), b"hello")?;
//!     Ok(())
//! }
//! ```
//!
//! # Example: Duplicate names with indexes
//! ```no_run
//! use file_database::{DatabaseError, DatabaseManager, ItemId};
//!
//! fn main() -> Result<(), DatabaseError> {
//!     let mut manager = DatabaseManager::new(".", "database")?;
//!
//!     manager.write_new(ItemId::id("folder_a"), ItemId::database_id())?;
//!     manager.write_new(ItemId::id("folder_b"), ItemId::database_id())?;
//!     manager.write_new(ItemId::id("test.txt"), ItemId::id("folder_a"))?;
//!     manager.write_new(ItemId::id("test.txt"), ItemId::id("folder_b"))?;
//!
//!     // First match for "test.txt"
//!     let first = ItemId::id("test.txt");
//!     // Second match for "test.txt"
//!     let second = ItemId::with_index("test.txt", 1);
//!
//!     let _first_path = manager.locate_absolute(first)?;
//!     let _second_path = manager.locate_absolute(second)?;
//!     Ok(())
//! }
//! ```
//!
//! # Example: Get all IDs for one shared name
//! ```no_run
//! use file_database::{DatabaseError, DatabaseManager, ItemId};
//!
//! fn main() -> Result<(), DatabaseError> {
//!     let mut manager = DatabaseManager::new(".", "database")?;
//!     manager.write_new(ItemId::id("a.txt"), ItemId::database_id())?;
//!     manager.write_new(ItemId::id("folder"), ItemId::database_id())?;
//!     manager.write_new(ItemId::id("a.txt"), ItemId::id("folder"))?;
//!
//!     let ids = manager.get_ids_from_shared_id(ItemId::id("a.txt"))?;
//!     assert_eq!(ids.len(), 2);
//!     assert_eq!(ids[0].get_index(), 0);
//!     assert_eq!(ids[1].get_index(), 1);
//!     Ok(())
//! }
//! ```

use std::{
    collections::{HashMap, HashSet},
    env::{current_dir, current_exe},
    ffi::OsStr,
    fs::{self, File, create_dir, remove_dir, remove_dir_all, remove_file},
    hash::Hash,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

// Constants
const ZERO: u64 = 0;
const THOUSAND: u64 = 1_000;
const MILLION: u64 = 1_000_000;
const BILLION: u64 = 1_000_000_000;
const TRILLION: u64 = 1_000_000_000_000;
const QUADRILLION: u64 = 1_000_000_000_000_000;

// -------- Enums --------
#[derive(Debug, Error)]
/// Errors returned by this library.
pub enum DatabaseError {
    /// Returned when requested path-step trimming exceeds the available path depth.
    #[error("Steps '{0}' greater than path length '{1}'")]
    PathStepOverflow(i32, i32),
    /// Returned when no matching directory name can be found while walking upward.
    #[error("Directory '{0}' not found along path to executable")]
    NoClosestDir(String),
    /// Returned when an `ItemId` name has no tracked entries in the index.
    #[error("ID '{0}' doesn't point to a known path")]
    NoMatchingID(String),
    /// Returned when creating or renaming to an ID that already exists at the target path.
    #[error("ID '{0}' already exists")]
    IdAlreadyExists(String),
    /// Returned when source and destination resolve to the same filesystem path.
    #[error("Source and destination are identical: '{0}'")]
    IdenticalSourceDestination(PathBuf),
    /// Returned when an export destination points inside the managed database root.
    #[error("Export destination is inside the database: '{0}'")]
    ExportDestinationInsideDatabase(PathBuf),
    /// Returned when an import source path points inside the managed database root.
    #[error("Import source is inside the database: '{0}'")]
    ImportSourceInsideDatabase(PathBuf),
    /// Returned when the requested `index` is outside the bounds of the ID match list.
    #[error("Index {index} out of bounds for ID '{id}' (len: {len})")]
    IndexOutOfBounds {
        id: String,
        index: usize,
        len: usize,
    },
    /// Returned when an operation does not allow `ItemId::database_id()` as input.
    #[error("Root database ID cannot be used for this operation")]
    RootIdUnsupported,
    /// Returned when a path was expected to be a directory but is not.
    #[error("Path '{0}' doesn't point to a directory")]
    NotADirectory(PathBuf),
    /// Returned when a path was expected to be a file but is not.
    #[error("Path '{0}' doesn't point to a file")]
    NotAFile(PathBuf),
    /// Returned when converting an OS string/path segment into UTF-8 text fails.
    #[error("Couldn't convert OsString to String")]
    OsStringConversion,
    /// Returned when an item has no parent inside the tracked database tree.
    #[error("ID '{0}' doesn't have a parent")]
    NoParent(String),
    /// Returned when an underlying filesystem I/O operation fails.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Returned when JSON serialization or deserialization fails.
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    /// Returned when bincode serialization or deserialization fails.
    #[error(transparent)]
    Bincode(#[from] bincode::Error),
    /// Returned when converting an absolute path into a database-relative path fails.
    #[error(transparent)]
    PathBufConversion(#[from] std::path::StripPrefixError),
}

#[derive(Debug, PartialEq, Clone, Default)]
/// Controls whether directory deletion is forced.
pub enum ForceDeletion {
    #[default]
    Force,
    NoForce,
}

impl Into<bool> for ForceDeletion {
    /// Converts **`ForceDeletion`** into its boolean form.
    fn into(self) -> bool {
        match self {
            ForceDeletion::Force => true,
            ForceDeletion::NoForce => false,
        }
    }
}

impl From<bool> for ForceDeletion {
    /// Converts a boolean into **`ForceDeletion`**.
    fn from(value: bool) -> Self {
        match value {
            true => ForceDeletion::Force,
            false => ForceDeletion::NoForce,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
/// Controls whether list results are sorted.
pub enum ShouldSort {
    #[default]
    Sort,
    NoSort,
}

impl Into<bool> for ShouldSort {
    /// Converts **`ShouldSort`** into its boolean form.
    fn into(self) -> bool {
        match self {
            ShouldSort::Sort => true,
            ShouldSort::NoSort => false,
        }
    }
}

impl From<bool> for ShouldSort {
    /// Converts a boolean into **`ShouldSort`**.
    fn from(value: bool) -> Self {
        match value {
            true => ShouldSort::Sort,
            false => ShouldSort::NoSort,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
/// Controls whether APIs should serialize values.
pub enum Serialize {
    #[default]
    Serialize,
    NoSerialize,
}

impl Into<bool> for Serialize {
    /// Converts **`Serialize`** into its boolean form.
    fn into(self) -> bool {
        match self {
            Serialize::Serialize => true,
            Serialize::NoSerialize => false,
        }
    }
}

impl From<bool> for Serialize {
    /// Converts a boolean into **`Serialize`**.
    fn from(value: bool) -> Self {
        match value {
            true => Serialize::Serialize,
            false => Serialize::NoSerialize,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
/// Controls whether export copies or moves the source.
pub enum ExportMode {
    #[default]
    Copy,
    Move,
}

#[derive(Debug, PartialEq, Clone, Default)]
/// Controls how `scan_for_changes` handles newly found files.
pub enum ScanPolicy {
    DetectOnly,
    RemoveNew,
    #[default]
    AddNew,
}

#[derive(Debug, Default, PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
/// Units used by **`FileSize`**.
pub enum FileSizeUnit {
    #[default]
    Byte,
    Kilobyte,
    Megabyte,
    Gigabyte,
    Terabyte,
    Petabyte,
}

impl FileSizeUnit {
    /// Internal numeric rank used for unit conversion math.
    fn variant_integer_id(&self) -> u8 {
        match self {
            Self::Byte => 0,
            Self::Kilobyte => 1,
            Self::Megabyte => 2,
            Self::Gigabyte => 3,
            Self::Terabyte => 4,
            Self::Petabyte => 5,
        }
    }
}

// -------- Structs --------
#[derive(PartialEq, Debug, Clone, Default)]
/// Helper for building paths from the current process location.
pub struct GenPath;

impl GenPath {
    /// Returns the working directory, with `steps` parts removed from the end.
    ///
    /// # Parameters
    /// - `steps`: number of path parts at the end to remove.
    ///
    /// # Errors
    /// Returns an error if:
    /// - the current directory cannot be read,
    /// - `steps` is greater than or equal to the number of removable segments.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, GenPath};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let _cwd = GenPath::from_working_dir(0)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn from_working_dir(steps: i32) -> Result<PathBuf, DatabaseError> {
        let working_dir = truncate(current_dir()?, steps)?;

        Ok(working_dir)
    }

    /// Returns the executable directory, with `steps` parts removed from the end.
    ///
    /// # Parameters
    /// - `steps`: number of path parts at the end to remove from the executable directory.
    ///
    /// # Errors
    /// Returns an error if:
    /// - the executable path cannot be read,
    /// - `steps` is too large for the path depth.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, GenPath};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let _exe_dir = GenPath::from_exe(0)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn from_exe(steps: i32) -> Result<PathBuf, DatabaseError> {
        let exe = truncate(current_exe()?, steps + 1)?;

        Ok(exe)
    }

    /// Looks for the nearest matching folder name while walking up from the executable.
    ///
    /// At each level, this checks:
    /// - the folder name itself,
    /// - child folders one level down.
    ///
    /// File entries are ignored.
    ///
    /// # Parameters
    /// - `name`: directory name to search for.
    ///
    /// # Errors
    /// Returns an error if:
    /// - no matching directory is found,
    /// - the provided name cannot be converted from `OsStr` to `String`.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, GenPath};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let _path = GenPath::from_closest_match("src")?;
    ///     Ok(())
    /// }
    /// ```
    pub fn from_closest_match(name: impl AsRef<Path>) -> Result<PathBuf, DatabaseError> {
        let exe = current_exe()?;
        let target = name.as_ref();
        let target_name = target.as_os_str();

        for path in exe.ancestors() {
            if !path.is_dir() {
                continue;
            }

            if path
                .file_name()
                .is_some_and(|dir_name| dir_name == target_name)
            {
                return Ok(path.to_path_buf());
            }

            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(_) => continue,
                    };

                    let child_path = entry.path();
                    if !child_path.is_dir() {
                        continue;
                    }

                    if entry.file_name() == target_name {
                        return Ok(child_path);
                    }
                }
            }
        }

        let name_as_string = match target.to_owned().into_os_string().into_string() {
            Ok(string) => string,
            Err(_) => return Err(DatabaseError::OsStringConversion),
        };

        Err(DatabaseError::NoClosestDir(name_as_string))
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
/// Identifier used to select a tracked item by `name` and `index`.
///
/// Use this when:
/// - you know the shared `name` and want the first match (`ItemId::id("name")`),
/// - or when you need a specific duplicate (`ItemId::with_index("name", i)`).
///
/// `ItemId::database_id()` is special and points to the database root itself.
///
/// # Examples
/// ```
/// use file_database::ItemId;
///
/// let first = ItemId::id("report.txt");
/// let second = ItemId::with_index("report.txt", 1);
/// let root = ItemId::database_id();
///
/// assert_eq!(first.get_name(), "report.txt");
/// assert_eq!(first.get_index(), 0);
/// assert_eq!(second.get_index(), 1);
/// assert_eq!(root.get_name(), "");
/// ```
pub struct ItemId {
    name: String,
    index: usize,
}

impl<T> From<T> for ItemId
where
    T: Into<String>,
{
    /// Creates an **`ItemId`** from a name, defaulting `index` to `0`.
    fn from(value: T) -> Self {
        Self {
            name: value.into(),
            index: 0,
        }
    }
}

impl From<&ItemId> for ItemId {
    /// Clones an **`ItemId`** from a reference.
    fn from(value: &ItemId) -> Self {
        value.clone()
    }
}

impl ItemId {
    /// Returns the `ItemId::database_id()` for the database itself.
    ///
    /// # Examples
    /// ```
    /// use file_database::ItemId;
    ///
    /// let root = ItemId::database_id();
    /// assert_eq!(root.get_name(), "");
    /// assert_eq!(root.get_index(), 0);
    /// ```
    pub fn database_id() -> Self {
        Self {
            name: String::new(),
            index: 0,
        }
    }

    /// Creates an **`ItemId`** with `index` `0`.
    ///
    /// # Parameters
    /// - `id`: shared `name` key stored in the manager.
    ///
    /// # Examples
    /// ```
    /// use file_database::ItemId;
    ///
    /// let id = ItemId::id("file.txt");
    /// assert_eq!(id.get_name(), "file.txt");
    /// assert_eq!(id.get_index(), 0);
    /// ```
    pub fn id(id: impl Into<String>) -> Self {
        Self {
            name: id.into(),
            index: 0,
        }
    }

    /// Creates an **`ItemId`** with an explicit shared-name `index`.
    ///
    /// # Parameters
    /// - `id`: shared `name` key.
    /// - `index`: zero-based `index` within that key's stored path vector.
    ///
    /// # Examples
    /// ```
    /// use file_database::ItemId;
    ///
    /// let id = ItemId::with_index("file.txt", 2);
    /// assert_eq!(id.get_name(), "file.txt");
    /// assert_eq!(id.get_index(), 2);
    /// ```
    pub fn with_index(id: impl Into<String>, index: usize) -> Self {
        Self {
            name: id.into(),
            index,
        }
    }

    /// Returns the shared `name` of this **`ItemId`**.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Returns the zero-based `index` for this shared `name`.
    pub fn get_index(&self) -> usize {
        self.index
    }

    /// Returns the shared `name` as `&str`.
    pub fn as_str(&self) -> &str {
        self.get_name()
    }

    /// Returns an owned `String` containing this **`ItemId`**'s shared `name`.
    pub fn as_string(&self) -> String {
        self.name.clone()
    }
}

#[derive(Debug, Default, PartialEq, PartialOrd, Clone, Copy)]
/// File size value paired with a unit.
pub struct FileSize {
    size: u64,
    unit: FileSizeUnit,
}

impl FileSize {
    /// Returns the stored size value in the current unit.
    pub fn get_size(&self) -> u64 {
        self.size
    }

    /// Returns the stored unit.
    pub fn get_unit(&self) -> FileSizeUnit {
        self.unit
    }

    /// Returns a human-readable unit string, pluralized when needed.
    ///
    /// # Examples
    /// ```
    /// use file_database::FileSize;
    ///
    /// let size = FileSize::default();
    /// assert_eq!(size.unit_as_string(), "Bytes");
    /// ```
    pub fn unit_as_string(&self) -> String {
        let name = match self.unit {
            FileSizeUnit::Byte => "Byte",
            FileSizeUnit::Kilobyte => "Kilobyte",
            FileSizeUnit::Megabyte => "Megabyte",
            FileSizeUnit::Gigabyte => "Gigabyte",
            FileSizeUnit::Terabyte => "Terabyte",
            FileSizeUnit::Petabyte => "Petabyte",
        };

        let mut name_string = String::from(name);

        // Push an s to the end of the string if not 1
        match self.size {
            1 => (),
            _ => name_string.push('s'),
        }

        name_string
    }

    /// Returns a copy of this size converted to another unit.
    ///
    /// Conversion uses powers of 1000 between adjacent units.
    ///
    /// # Parameters
    /// - `unit`: destination unit.
    ///
    /// # Examples
    /// ```
    /// use file_database::{FileSize, FileSizeUnit};
    ///
    /// let bytes = FileSize::default().as_unit(FileSizeUnit::Byte);
    /// assert_eq!(bytes.get_unit(), FileSizeUnit::Byte);
    /// ```
    pub fn as_unit(&self, unit: FileSizeUnit) -> Self {
        let difference = self.unit.variant_integer_id() as i8 - unit.variant_integer_id() as i8;

        let mut size = self.size;

        if difference > 0 {
            let factor = THOUSAND.pow(difference as u32);
            size = size.saturating_mul(factor);
        } else if difference < 0 {
            let factor = THOUSAND.pow((-difference) as u32);
            size /= factor;
        }

        Self { size, unit }
    }

    /// Builds **`FileSize`** from raw bytes using automatic unit selection.
    fn from(bytes: u64) -> Self {
        let (size, unit) = match bytes {
            ZERO..THOUSAND => (bytes, FileSizeUnit::Byte),
            THOUSAND..MILLION => (bytes / THOUSAND, FileSizeUnit::Kilobyte),
            MILLION..BILLION => (bytes / MILLION, FileSizeUnit::Megabyte),
            BILLION..TRILLION => (bytes / BILLION, FileSizeUnit::Gigabyte),
            TRILLION..QUADRILLION => (bytes / TRILLION, FileSizeUnit::Terabyte),
            _ => (bytes / QUADRILLION, FileSizeUnit::Petabyte),
        };

        Self { size, unit }
    }
}

#[derive(Debug, Default, PartialEq, PartialOrd, Clone)]
/// Metadata returned by `get_file_information`.
pub struct FileInformation {
    name: Option<String>,
    extension: Option<String>,
    size: FileSize,
    unix_created: Option<u64>,
    time_since_created: Option<u64>,
    unix_last_opened: Option<u64>,
    time_since_last_opened: Option<u64>,
    unix_last_modified: Option<u64>,
    time_since_last_modified: Option<u64>,
}

impl FileInformation {
    /// Returns file `name` without extension, or directory `name` for directories.
    pub fn get_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns file extension for files, otherwise `None`.
    pub fn get_extension(&self) -> Option<&str> {
        self.extension.as_deref()
    }

    /// Returns normalized file size data.
    pub fn get_size(&self) -> &FileSize {
        &self.size
    }

    /// Returns created-at Unix timestamp (seconds), when available on this platform.
    pub fn get_unix_created(&self) -> Option<&u64> {
        self.unix_created.as_ref()
    }

    /// Returns age since creation in seconds, when available.
    pub fn get_time_since_created(&self) -> Option<&u64> {
        self.time_since_created.as_ref()
    }

    /// Returns last-accessed Unix timestamp (seconds), when available.
    pub fn get_unix_last_opened(&self) -> Option<&u64> {
        self.unix_last_opened.as_ref()
    }

    /// Returns age since last access in seconds, when available.
    pub fn get_time_since_last_opened(&self) -> Option<&u64> {
        self.time_since_last_opened.as_ref()
    }

    /// Returns last-modified Unix timestamp (seconds), when available.
    pub fn get_unix_last_modified(&self) -> Option<&u64> {
        self.unix_last_modified.as_ref()
    }

    /// Returns age since last modification in seconds, when available.
    pub fn get_time_since_last_modified(&self) -> Option<&u64> {
        self.time_since_last_modified.as_ref()
    }
}

#[derive(Debug, PartialEq, Clone)]
/// A file or folder change found by `scan_for_changes`.
pub enum ExternalChange {
    Added { id: ItemId, path: PathBuf },
    Removed { id: ItemId, path: PathBuf },
}

#[derive(Debug, PartialEq, Clone)]
/// Summary returned by `scan_for_changes`.
pub struct ScanReport {
    scanned_from: ItemId,
    recursive: bool,
    added: Vec<ExternalChange>,
    removed: Vec<ExternalChange>,
    unchanged_count: usize,
    total_changed_count: usize,
}

impl ScanReport {
    /// Returns the **`ItemId`** used as the scan root.
    pub fn get_scan_from(&self) -> &ItemId {
        &self.scanned_from
    }

    /// Returns all newly discovered items in the scanned scope.
    pub fn get_added(&self) -> &Vec<ExternalChange> {
        &self.added
    }

    /// Returns tracked **`ItemId`** values that were missing on disk.
    pub fn get_removed(&self) -> &Vec<ExternalChange> {
        &self.removed
    }

    /// Returns how many tracked **`ItemId`** values stayed the same in this scan area.
    pub fn get_unchanged_count(&self) -> usize {
        self.unchanged_count
    }

    /// Returns total number of changed items (`added + removed`).
    pub fn get_total_changed_count(&self) -> usize {
        self.total_changed_count
    }
}

#[derive(Debug, PartialEq)]
/// Main type that manages a database directory and its index.
pub struct DatabaseManager {
    path: PathBuf,
    items: HashMap<String, Vec<PathBuf>>,
}

impl DatabaseManager {
    /// Creates a new database directory and returns a manager for it.
    ///
    /// # Parameters
    /// - `path`: parent directory where the database folder will be created.
    /// - `name`: database directory name appended to `path`.
    ///
    /// # Errors
    /// Returns an error if:
    /// - the destination directory already exists,
    /// - parent directories are missing,
    /// - the process cannot create directories at the destination.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let _manager = DatabaseManager::new(".", "database")?;
    ///     Ok(())
    /// }
    /// ```
    pub fn new(path: impl AsRef<Path>, name: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let mut path: PathBuf = path.as_ref().to_path_buf();

        path.push(name);

        create_dir(&path)?;

        let manager = Self {
            path: path.into(),
            items: HashMap::new(),
        };

        Ok(manager)
    }

    /// Creates a new file or directory under `parent`.
    ///
    /// Name interpretation is extension-based:
    /// - if `id.name` has an extension, a file is created,
    /// - otherwise, a directory is created.
    ///
    /// # Parameters
    /// - `id`: name key for the new item. Root **`ItemId`** is not allowed.
    /// - `parent`: destination parent item. Use `ItemId::database_id()` for database root.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `id` is the `ItemId::database_id()`,
    /// - `parent` cannot be found,
    /// - another item already exists at the target relative path,
    /// - filesystem create operations fail.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("notes.txt"), ItemId::database_id())?;
    ///     Ok(())
    /// }
    /// ```
    pub fn write_new(
        &mut self,
        id: impl Into<ItemId>,
        parent: impl Into<ItemId>,
    ) -> Result<(), DatabaseError> {
        let id = id.into();
        let parent = parent.into();

        if id.get_name().is_empty() {
            return Err(DatabaseError::RootIdUnsupported);
        }

        let absolute_parent_path = self.locate_absolute(&parent)?;
        let relative_path = if parent.get_name().is_empty() {
            PathBuf::from(id.get_name())
        } else {
            let mut path = self.locate_relative(parent)?.to_path_buf();
            path.push(id.get_name());
            path
        };
        let absolute_path = absolute_parent_path.join(id.get_name());

        if self
            .items
            .get(id.get_name())
            .is_some_and(|paths| paths.iter().any(|path| path == &relative_path))
        {
            return Err(DatabaseError::IdAlreadyExists(id.as_string()));
        }

        if relative_path.extension().is_none() {
            create_dir(&absolute_path)?;
        } else {
            File::create_new(&absolute_path)?;
        }

        self.items
            .entry(id.get_name().to_string())
            .or_default()
            .push(relative_path);
        Ok(())
    }

    /// Overwrites an existing file with raw bytes in a safe way.
    ///
    /// It writes to a temp file first, then replaces the target file.
    ///
    /// # Parameters
    /// - `id`: target file **`ItemId`**.
    /// - `data`: raw bytes to write.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `id` cannot be found,
    /// - `id` points to a directory,
    /// - writing, syncing, or renaming fails.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("blob.bin"), ItemId::database_id())?;
    ///     manager.overwrite_existing(ItemId::id("blob.bin"), [1_u8, 2, 3, 4])?;
    ///     Ok(())
    /// }
    /// ```
    pub fn overwrite_existing<T>(&self, id: impl Into<ItemId>, data: T) -> Result<(), DatabaseError>
    where
        T: AsRef<[u8]>,
    {
        let id = id.into();
        let bytes = data.as_ref();

        let path = self.locate_absolute(id)?;

        self.overwrite_path_atomic_with(&path, |file| {
            file.write_all(bytes)?;
            Ok(bytes.len() as u64)
        })?;

        Ok(())
    }

    /// Converts `value` to JSON and overwrites the target file.
    ///
    /// # Parameters
    /// - `id`: target file **`ItemId`**.
    /// - `value`: serializable value.
    ///
    /// # Errors
    /// Returns an error if:
    /// - JSON serialization fails,
    /// - finding `id` or overwriting the file fails.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    /// use serde::Serialize;
    ///
    /// #[derive(Serialize)]
    /// struct Config {
    ///     retries: u8,
    /// }
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("config.json"), ItemId::database_id())?;
    ///     manager.overwrite_existing_json(ItemId::id("config.json"), &Config { retries: 3 })?;
    ///     Ok(())
    /// }
    /// ```
    pub fn overwrite_existing_json<T: serde::Serialize>(
        &self,
        id: impl Into<ItemId>,
        value: &T,
    ) -> Result<(), DatabaseError> {
        let data = serde_json::to_vec(value)?;
        self.overwrite_existing(id, data)
    }

    /// Converts `value` to bincode and overwrites the target file.
    ///
    /// # Parameters
    /// - `id`: target file **`ItemId`**.
    /// - `value`: serializable value.
    ///
    /// # Errors
    /// Returns an error if:
    /// - bincode serialization fails,
    /// - finding `id` or overwriting the file fails.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    /// use serde::Serialize;
    ///
    /// #[derive(Serialize)]
    /// enum State {
    ///     Ready,
    /// }
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("state.bin"), ItemId::database_id())?;
    ///     manager.overwrite_existing_binary(ItemId::id("state.bin"), &State::Ready)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn overwrite_existing_binary<T: serde::Serialize>(
        &self,
        id: impl Into<ItemId>,
        value: &T,
    ) -> Result<(), DatabaseError> {
        let data = bincode::serialize(value)?;
        self.overwrite_existing(id, data)
    }

    /// Streams bytes from `reader` into the target file and returns bytes written.
    ///
    /// This uses chunked I/O and a safe replace step, so it works well for large payloads.
    ///
    /// # Parameters
    /// - `id`: target file **`ItemId`**.
    /// - `reader`: source stream consumed until EOF.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `id` cannot be found,
    /// - target is not a file,
    /// - stream read/write/sync/rename fails.
    ///
    /// # Examples
    /// ```no_run
    /// use std::io::Cursor;
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("stream.bin"), ItemId::database_id())?;
    ///     let mut source = Cursor::new(vec![9_u8; 1024]);
    ///     let _bytes = manager.overwrite_existing_from_reader(ItemId::id("stream.bin"), &mut source)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn overwrite_existing_from_reader<R: io::Read>(
        &self,
        id: impl Into<ItemId>,
        reader: &mut R,
    ) -> Result<u64, DatabaseError> {
        let id = id.into();
        let path = self.locate_absolute(id)?;
        self.overwrite_path_atomic_with(&path, |file| Ok(io::copy(reader, file)?))
    }

    /// Reads a managed file and returns its raw bytes.
    ///
    /// # Parameters
    /// - `id`: target file **`ItemId`**.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `id` cannot be found,
    /// - `id` points to a directory,
    /// - file reading fails.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("data.bin"), ItemId::database_id())?;
    ///     manager.overwrite_existing(ItemId::id("data.bin"), [1_u8, 2, 3])?;
    ///     let _data = manager.read_existing(ItemId::id("data.bin"))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn read_existing(&self, id: impl Into<ItemId>) -> Result<Vec<u8>, DatabaseError> {
        let id = id.into();
        let path = self.locate_absolute(id)?;

        if path.is_dir() {
            return Err(DatabaseError::NotAFile(path));
        }

        Ok(fs::read(path)?)
    }

    /// Reads a managed file and turns JSON into `T`.
    ///
    /// # Parameters
    /// - `id`: target file **`ItemId`**.
    ///
    /// # Errors
    /// Returns an error if:
    /// - finding `id` or reading the file fails,
    /// - JSON deserialization fails.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Serialize, Deserialize)]
    /// struct Config {
    ///     retries: u8,
    /// }
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("config.json"), ItemId::database_id())?;
    ///     manager.overwrite_existing_json(ItemId::id("config.json"), &Config { retries: 3 })?;
    ///     let _loaded: Config = manager.read_existing_json(ItemId::id("config.json"))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn read_existing_json<T: serde::de::DeserializeOwned>(
        &self,
        id: impl Into<ItemId>,
    ) -> Result<T, DatabaseError> {
        let bytes = self.read_existing(id)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Reads a managed file and turns bincode into `T`.
    ///
    /// # Parameters
    /// - `id`: target file **`ItemId`**.
    ///
    /// # Errors
    /// Returns an error if:
    /// - finding `id` or reading the file fails,
    /// - bincode deserialization fails.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Serialize, Deserialize)]
    /// enum State {
    ///     Ready,
    /// }
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("state.bin"), ItemId::database_id())?;
    ///     manager.overwrite_existing_binary(ItemId::id("state.bin"), &State::Ready)?;
    ///     let _loaded: State = manager.read_existing_binary(ItemId::id("state.bin"))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn read_existing_binary<T: serde::de::DeserializeOwned>(
        &self,
        id: impl Into<ItemId>,
    ) -> Result<T, DatabaseError> {
        let bytes = self.read_existing(id)?;
        Ok(bincode::deserialize(&bytes)?)
    }

    /// Returns every tracked item in the database.
    ///
    /// # Parameters
    /// - `sorted`: whether output should be sorted by **`ItemId`** ordering.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("a.txt"), ItemId::database_id())?;
    ///     let _all = manager.get_all(true);
    ///     Ok(())
    /// }
    /// ```
    pub fn get_all(&self, sorted: impl Into<bool>) -> Vec<ItemId> {
        let sorted = sorted.into();

        let mut list: Vec<ItemId> = self
            .items
            .iter()
            .flat_map(|(name, paths)| {
                paths
                    .iter()
                    .enumerate()
                    .map(|(index, _)| ItemId::with_index(name.clone(), index))
            })
            .collect();

        if sorted {
            list.sort();
        }

        list
    }

    /// Returns all tracked items that are direct children of `parent`.
    ///
    /// If `parent` is the `ItemId::database_id()`, this returns all top-level items.
    ///
    /// # Parameters
    /// - `parent`: parent directory item to query.
    /// - `sorted`: whether output should be sorted by **`ItemId`**.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `parent` cannot be found,
    /// - `parent` points to a file instead of a directory.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("folder"), ItemId::database_id())?;
    ///     manager.write_new(ItemId::id("a.txt"), ItemId::id("folder"))?;
    ///     let _children = manager.get_by_parent(ItemId::id("folder"), true)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn get_by_parent(
        &self,
        parent: impl Into<ItemId>,
        sorted: impl Into<bool>,
    ) -> Result<Vec<ItemId>, DatabaseError> {
        let parent = parent.into();
        let sorted = sorted.into();

        let absolute_parent = self.locate_absolute(&parent)?;

        if !absolute_parent.is_dir() {
            return Err(DatabaseError::NotADirectory(absolute_parent));
        }

        let mut list: Vec<ItemId> = if parent.get_name().is_empty() {
            self.items
                .iter()
                .flat_map(|(name, paths)| {
                    paths.iter().enumerate().filter_map(|(index, item_path)| {
                        item_path
                            .parent()
                            .is_some_and(|parent| parent.as_os_str().is_empty())
                            .then_some(ItemId::with_index(name.clone(), index))
                    })
                })
                .collect()
        } else {
            let parent_path = self.locate_relative(parent)?;
            self.items
                .iter()
                .flat_map(|(name, paths)| {
                    paths.iter().enumerate().filter_map(|(index, item_path)| {
                        (item_path.parent() == Some(parent_path.as_path()))
                            .then_some(ItemId::with_index(name.clone(), index))
                    })
                })
                .collect()
        };

        if sorted {
            list.sort();
        }

        Ok(list)
    }

    /// Returns the parent **`ItemId`** for an item.
    ///
    /// Top-level items return [`ItemId::database_id`].
    ///
    /// # Parameters
    /// - `id`: item whose parent should be looked up.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `id` cannot be found,
    /// - parent path data cannot be converted to UTF-8 string.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("folder"), ItemId::database_id())?;
    ///     manager.write_new(ItemId::id("a.txt"), ItemId::id("folder"))?;
    ///     let _parent = manager.get_parent(ItemId::id("a.txt"))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn get_parent(&self, id: impl Into<ItemId>) -> Result<ItemId, DatabaseError> {
        let id = id.into();
        let path = self.locate_relative(&id)?;

        let parent = match path.parent() {
            Some(parent) => parent,
            None => return Ok(ItemId::database_id()),
        };

        if parent.as_os_str().is_empty() {
            return Ok(ItemId::database_id());
        }

        match parent.file_name() {
            Some(name) => Ok(ItemId::id(os_str_to_string(Some(name))?)),
            None => Err(DatabaseError::NoParent(id.as_string())),
        }
    }

    /// Renames the chosen item to `to` in the same parent directory.
    ///
    /// # Parameters
    /// - `id`: source **`ItemId`** to rename.
    /// - `to`: new file or directory name.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `id` is the `ItemId::database_id()`,
    /// - `id` cannot be found,
    /// - `id.index` is out of range for the list of paths under this `name`,
    /// - destination `name` already exists at the same relative `path`,
    /// - underlying filesystem rename fails.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("old.txt"), ItemId::database_id())?;
    ///     manager.rename(ItemId::id("old.txt"), "new.txt")?;
    ///     Ok(())
    /// }
    /// ```
    pub fn rename(
        &mut self,
        id: impl Into<ItemId>,
        to: impl AsRef<str>,
    ) -> Result<(), DatabaseError> {
        let id = id.into();
        let name = to.as_ref().to_owned();

        if id.get_name().is_empty() {
            return Err(DatabaseError::RootIdUnsupported);
        }

        let path = self.locate_absolute(&id)?;
        let mut relative_path = self.locate_relative(&id)?.to_path_buf();

        let renamed_path = path.with_file_name(&name);
        relative_path = match relative_path.pop() {
            true => {
                relative_path.push(&name);
                relative_path
            }
            false => PathBuf::from(&name),
        };

        if self
            .items
            .get(&name)
            .is_some_and(|paths| paths.iter().any(|entry| entry == &relative_path))
        {
            return Err(DatabaseError::IdAlreadyExists(name));
        }

        fs::rename(&path, renamed_path)?;

        let old_name = id.get_name().to_string();
        let old_paths = self
            .items
            .get_mut(&old_name)
            .ok_or_else(|| DatabaseError::NoMatchingID(id.as_string()))?;

        if id.get_index() >= old_paths.len() {
            return Err(DatabaseError::IndexOutOfBounds {
                id: id.as_string(),
                index: id.get_index(),
                len: old_paths.len(),
            });
        }

        old_paths.swap_remove(id.get_index());
        if old_paths.is_empty() {
            self.items.remove(&old_name);
        }

        self.items.entry(name).or_default().push(relative_path);

        Ok(())
    }

    /// Deletes a file, directory, or the whole database root.
    ///
    /// # Parameters
    /// - `id`: item to delete. Use `ItemId::database_id()` to target the database folder itself.
    /// - `force`: when deleting directories, controls recursive vs empty-only behavior.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `id` cannot be found,
    /// - `id.index` is out of range for the list of paths under this `name`,
    /// - directory deletion does not match `force` rules,
    /// - filesystem delete operations fail.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ForceDeletion, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("tmp.txt"), ItemId::database_id())?;
    ///     manager.delete(ItemId::id("tmp.txt"), ForceDeletion::Force)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn delete(
        &mut self,
        id: impl Into<ItemId>,
        force: impl Into<bool>,
    ) -> Result<(), DatabaseError> {
        let id = id.into();

        if id.get_name().is_empty() {
            match delete_directory(&self.locate_absolute(id)?, force) {
                Ok(_) => {
                    self.path = PathBuf::new();
                    self.items.drain();
                    return Ok(());
                }
                Err(error) => return Err(error),
            }
        }

        let path = self.locate_absolute(&id)?;

        if path.is_dir() {
            delete_directory(&path, force)?;
        } else {
            remove_file(path)?;
        }

        let key = id.get_name().to_string();
        let paths = self
            .items
            .get_mut(&key)
            .ok_or_else(|| DatabaseError::NoMatchingID(id.as_string()))?;

        if id.get_index() >= paths.len() {
            return Err(DatabaseError::IndexOutOfBounds {
                id: id.as_string(),
                index: id.get_index(),
                len: paths.len(),
            });
        }

        paths.swap_remove(id.get_index());
        if paths.is_empty() {
            self.items.remove(&key);
        }

        Ok(())
    }

    /// Gets the absolute file path for an **`ItemId`**.
    ///
    /// For the `ItemId::database_id()`, this returns the database directory path.
    ///
    /// # Parameters
    /// - `id`: **`ItemId`** to look up.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `id.name` does not exist,
    /// - `id.index` is out of bounds.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("a.txt"), ItemId::database_id())?;
    ///     let _path = manager.locate_absolute(ItemId::id("a.txt"))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn locate_absolute(&self, id: impl Into<ItemId>) -> Result<PathBuf, DatabaseError> {
        let id = id.into();

        if id.get_name().is_empty() {
            return Ok(self.path.to_path_buf());
        }

        Ok(self.path.join(self.resolve_path_by_id(&id)?))
    }

    /// Gets the stored relative path reference for an **`ItemId`**.
    ///
    /// For the `ItemId::database_id()`, this currently returns a reference to the manager root path.
    ///
    /// # Parameters
    /// - `id`: **`ItemId`** to look up.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `id.name` does not exist,
    /// - `id.index` is out of bounds.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("a.txt"), ItemId::database_id())?;
    ///     let _relative = manager.locate_relative(ItemId::id("a.txt"))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn locate_relative(&self, id: impl Into<ItemId>) -> Result<&PathBuf, DatabaseError> {
        let id = id.into();
        if id.get_name().is_empty() {
            return Ok(&self.path);
        }

        self.resolve_path_by_id(&id)
    }

    /// Returns all stored relative paths for a shared `name`.
    ///
    /// # Parameters
    /// - `id`: shared-name **`ItemId`**. `index` is ignored for lookup.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `id` is the `ItemId::database_id()`,
    /// - no entry exists for `id.name`.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("a.txt"), ItemId::database_id())?;
    ///     let _paths = manager.get_paths_for_id(ItemId::id("a.txt"))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn get_paths_for_id(&self, id: impl Into<ItemId>) -> Result<&Vec<PathBuf>, DatabaseError> {
        let id = id.into();

        if id.get_name().is_empty() {
            return Err(DatabaseError::RootIdUnsupported);
        }

        self.items
            .get(id.get_name())
            .ok_or_else(|| DatabaseError::NoMatchingID(id.as_string()))
    }

    /// Returns all specific **`ItemId`** values for a shared `name`.
    ///
    /// # Parameters
    /// - `id`: shared-name **`ItemId`**. `index` is ignored for lookup.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `ItemId::database_id()` is provided,
    /// - no entry exists for `id.name`.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("a.txt"), ItemId::database_id())?;
    ///     let _ids = manager.get_ids_from_shared_id(ItemId::id("a.txt"))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn get_ids_from_shared_id(
        &self,
        id: impl Into<ItemId>,
    ) -> Result<Vec<ItemId>, DatabaseError> {
        let id = id.into();

        let paths = self.get_paths_for_id(&id)?;

        let ids = paths
            .iter()
            .enumerate()
            .map(|(index, _)| ItemId::with_index(id.get_name().to_string(), index))
            .collect();

        Ok(ids)
    }

    /// Scans files on disk and compares them to entries in this scan area.
    ///
    /// Missing tracked items are always removed from the `items` index kept in memory.
    ///
    /// Policy behavior for newly discovered external items:
    /// - `DetectOnly`: report only.
    /// - `AddNew`: report and add to the `index`.
    /// - `RemoveNew`: report and delete from disk.
    ///
    /// # Parameters
    /// - `scan_from`: root **`ItemId`** to scan from (`ItemId::database_id()` scans the full database).
    /// - `policy`: change handling policy.
    /// - `recursive`: `true` scans full subtree, `false` scans immediate children only.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `scan_from` cannot be found,
    /// - `scan_from` points to a file,
    /// - path-to-string conversion fails for discovered entries,
    /// - filesystem read or delete operations fail.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId, ScanPolicy};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     let _report = manager.scan_for_changes(ItemId::database_id(), ScanPolicy::AddNew, true)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn scan_for_changes(
        &mut self,
        scan_from: impl Into<ItemId>,
        policy: ScanPolicy,
        recursive: bool,
    ) -> Result<ScanReport, DatabaseError> {
        let scan_from = scan_from.into();
        let scan_from_absolute = self.locate_absolute(&scan_from)?;
        if !scan_from_absolute.is_dir() {
            return Err(DatabaseError::NotADirectory(scan_from_absolute));
        }

        let scope_relative = if scan_from.get_name().is_empty() {
            None
        } else {
            Some(self.locate_relative(&scan_from)?.clone())
        };

        let discovered_paths = self.collect_paths_in_scope(&scan_from_absolute, recursive)?;
        let discovered_set: HashSet<PathBuf> = discovered_paths.iter().cloned().collect();

        let mut existing_in_scope_set = HashSet::new();
        let mut removed = Vec::new();
        let mut unchanged_count = 0usize;

        for (name, paths) in &self.items {
            for (index, path) in paths.iter().enumerate() {
                if !is_path_in_scope(path, scope_relative.as_deref(), recursive) {
                    continue;
                }

                existing_in_scope_set.insert(path.clone());

                if discovered_set.contains(path) {
                    unchanged_count += 1;
                } else {
                    removed.push(ExternalChange::Removed {
                        id: ItemId::with_index(name.clone(), index),
                        path: path.clone(),
                    });
                }
            }
        }

        let mut added_paths: Vec<PathBuf> = discovered_paths
            .into_iter()
            .filter(|path| !existing_in_scope_set.contains(path))
            .collect();

        let mut added = Vec::new();
        let mut add_offsets: HashMap<String, usize> = HashMap::new();
        for path in &added_paths {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(DatabaseError::OsStringConversion)?
                .to_string();
            let base_len = self.items.get(&name).map(|paths| paths.len()).unwrap_or(0);
            let offset = add_offsets.entry(name.clone()).or_insert(0);
            let index = base_len + *offset;
            *offset += 1;

            added.push(ExternalChange::Added {
                id: ItemId::with_index(name, index),
                path: path.clone(),
            });
        }

        let mut empty_keys = Vec::new();
        for (name, paths) in self.items.iter_mut() {
            paths.retain(|path| {
                !is_path_in_scope(path, scope_relative.as_deref(), recursive)
                    || discovered_set.contains(path)
            });
            if paths.is_empty() {
                empty_keys.push(name.clone());
            }
        }
        for key in empty_keys {
            self.items.remove(&key);
        }

        match policy {
            ScanPolicy::DetectOnly => (),
            ScanPolicy::AddNew => {
                for path in &added_paths {
                    let name = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .ok_or(DatabaseError::OsStringConversion)?
                        .to_string();
                    self.items.entry(name).or_default().push(path.clone());
                }
            }
            ScanPolicy::RemoveNew => {
                added_paths.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
                for path in added_paths {
                    let absolute = self.path.join(&path);
                    if !absolute.exists() {
                        continue;
                    }

                    if absolute.is_dir() {
                        remove_dir_all(&absolute)?;
                    } else if absolute.is_file() {
                        remove_file(&absolute)?;
                    }
                }
            }
        }

        let total_changed_count = added.len() + removed.len();

        Ok(ScanReport {
            scanned_from: scan_from,
            recursive,
            added,
            removed,
            unchanged_count,
            total_changed_count,
        })
    }

    /// Moves the entire database directory to a new parent directory.
    ///
    /// Existing destination database directory with the same name is removed first.
    ///
    /// # Parameters
    /// - `to`: destination parent directory.
    ///
    /// # Errors
    /// Returns an error if:
    /// - current database path is invalid,
    /// - destination cleanup fails,
    /// - recursive copy or source removal fails.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.migrate_database("./new_parent")?;
    ///     Ok(())
    /// }
    /// ```
    pub fn migrate_database(&mut self, to: impl AsRef<Path>) -> Result<(), DatabaseError> {
        let destination = to.as_ref().to_path_buf();
        let name = self
            .path
            .file_name()
            .ok_or_else(|| DatabaseError::NotADirectory(self.path.clone()))?;
        let destination_database_path = destination.join(name);

        if destination_database_path.exists() {
            remove_dir_all(&destination_database_path)?;
        }

        copy_directory_recursive(&self.path, &destination_database_path)?;
        remove_dir_all(&self.path)?;

        self.path = destination_database_path;

        Ok(())
    }

    /// Moves a managed item to another directory inside the same database.
    ///
    /// # Parameters
    /// - `id`: source item to move.
    /// - `to`: destination directory item (or `ItemId::database_id()`).
    ///
    /// # Errors
    /// Returns an error if:
    /// - `id` is root or cannot be found,
    /// - destination is not a directory,
    /// - source and destination are identical,
    /// - `id.index` is out of bounds for the source `name` vector,
    /// - filesystem move fails.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("folder"), ItemId::database_id())?;
    ///     manager.write_new(ItemId::id("a.txt"), ItemId::database_id())?;
    ///     manager.migrate_item(ItemId::id("a.txt"), ItemId::id("folder"))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn migrate_item(
        &mut self,
        id: impl Into<ItemId>,
        to: impl Into<ItemId>,
    ) -> Result<(), DatabaseError> {
        let id = id.into();
        let to = to.into();

        if id.get_name().is_empty() {
            return Err(DatabaseError::RootIdUnsupported);
        }

        let destination_dir = self.locate_absolute(&to)?;
        if !destination_dir.is_dir() {
            return Err(DatabaseError::NotADirectory(destination_dir));
        }

        let source_absolute = self.locate_absolute(&id)?;
        let source_name = source_absolute
            .file_name()
            .ok_or_else(|| DatabaseError::NoMatchingID(id.as_string()))?;
        let destination_absolute = destination_dir.join(source_name);

        if destination_absolute == source_absolute {
            return Err(DatabaseError::IdenticalSourceDestination(
                destination_absolute,
            ));
        }

        if destination_absolute.exists() {
            if destination_absolute.is_dir() {
                remove_dir_all(&destination_absolute)?;
            } else {
                remove_file(&destination_absolute)?;
            }
        }

        fs::rename(&source_absolute, &destination_absolute)?;

        let old_name = id.get_name().to_string();
        let old_paths = self
            .items
            .get_mut(&old_name)
            .ok_or_else(|| DatabaseError::NoMatchingID(id.as_string()))?;

        if id.get_index() >= old_paths.len() {
            return Err(DatabaseError::IndexOutOfBounds {
                id: id.as_string(),
                index: id.get_index(),
                len: old_paths.len(),
            });
        }

        old_paths.swap_remove(id.get_index());
        if old_paths.is_empty() {
            self.items.remove(&old_name);
        }

        let relative_destination = destination_absolute.strip_prefix(&self.path)?.to_path_buf();
        let new_name = match relative_destination.file_name() {
            Some(name) => os_str_to_string(Some(name))?,
            None => old_name,
        };

        self.items
            .entry(new_name)
            .or_default()
            .push(relative_destination);

        Ok(())
    }

    /// Exports a managed file or directory to an external destination directory.
    ///
    /// `Copy` keeps the item in the `index`. `Move` removes the moved entry from the `index`.
    ///
    /// # Parameters
    /// - `id`: source item to export.
    /// - `to`: external destination directory path.
    /// - `mode`: copy or move behavior.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `id` is root or cannot be found,
    /// - destination is inside the database,
    /// - destination path cannot be created or used as a directory,
    /// - `id.index` is out of bounds when removing moved entries,
    /// - filesystem copy/move operations fail.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ExportMode, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("a.txt"), ItemId::database_id())?;
    ///     manager.export_item(ItemId::id("a.txt"), "./exports", ExportMode::Copy)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn export_item(
        &mut self,
        id: impl Into<ItemId>,
        to: impl AsRef<Path>,
        mode: ExportMode,
    ) -> Result<(), DatabaseError> {
        let id = id.into();
        let destination_dir = {
            let to = to.as_ref();
            if to.is_absolute() {
                to.to_path_buf()
            } else {
                current_dir()?.join(to)
            }
        };

        if id.get_name().is_empty() {
            return Err(DatabaseError::RootIdUnsupported);
        }

        if destination_dir.starts_with(&self.path) {
            return Err(DatabaseError::ExportDestinationInsideDatabase(
                destination_dir,
            ));
        }

        fs::create_dir_all(&destination_dir)?;
        if !destination_dir.is_dir() {
            return Err(DatabaseError::NotADirectory(destination_dir));
        }

        let source_absolute = self.locate_absolute(&id)?;
        let source_name = source_absolute
            .file_name()
            .ok_or_else(|| DatabaseError::NoMatchingID(id.as_string()))?;
        let destination_absolute = destination_dir.join(source_name);

        if destination_absolute == source_absolute {
            return Err(DatabaseError::IdenticalSourceDestination(
                destination_absolute,
            ));
        }

        if destination_absolute.exists() {
            if destination_absolute.is_dir() {
                remove_dir_all(&destination_absolute)?;
            } else {
                remove_file(&destination_absolute)?;
            }
        }

        match mode {
            ExportMode::Copy => {
                if source_absolute.is_dir() {
                    copy_directory_recursive(&source_absolute, &destination_absolute)?;
                } else {
                    fs::copy(&source_absolute, &destination_absolute)?;
                }
            }
            ExportMode::Move => {
                match fs::rename(&source_absolute, &destination_absolute) {
                    Ok(_) => (),
                    Err(_) => {
                        if source_absolute.is_dir() {
                            copy_directory_recursive(&source_absolute, &destination_absolute)?;
                            remove_dir_all(&source_absolute)?;
                        } else {
                            fs::copy(&source_absolute, &destination_absolute)?;
                            remove_file(&source_absolute)?;
                        }
                    }
                }

                let key = id.get_name().to_string();
                let paths = self
                    .items
                    .get_mut(&key)
                    .ok_or_else(|| DatabaseError::NoMatchingID(id.as_string()))?;

                if id.get_index() >= paths.len() {
                    return Err(DatabaseError::IndexOutOfBounds {
                        id: id.as_string(),
                        index: id.get_index(),
                        len: paths.len(),
                    });
                }

                paths.swap_remove(id.get_index());
                if paths.is_empty() {
                    self.items.remove(&key);
                }
            }
        }

        Ok(())
    }

    /// Imports an external file or directory into a database destination directory.
    ///
    /// The imported item keeps its original `name`.
    ///
    /// # Parameters
    /// - `from`: source path outside the database.
    /// - `to`: destination directory item in the database.
    ///
    /// # Errors
    /// Returns an error if:
    /// - source path points inside the database,
    /// - destination is not a directory,
    /// - destination `path`/`name` already exists,
    /// - source does not exist as file or directory,
    /// - filesystem copy operations fail.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("imports"), ItemId::database_id())?;
    ///     manager.import_item("./outside/example.txt", ItemId::id("imports"))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn import_item(
        &mut self,
        from: impl AsRef<Path>,
        to: impl Into<ItemId>,
    ) -> Result<(), DatabaseError> {
        let source_path = {
            let from = from.as_ref();
            if from.is_absolute() {
                from.to_path_buf()
            } else {
                current_dir()?.join(from)
            }
        };
        let to = to.into();

        if source_path.starts_with(&self.path) {
            return Err(DatabaseError::ImportSourceInsideDatabase(source_path));
        }

        let destination_parent = self.locate_absolute(&to)?;
        if !destination_parent.is_dir() {
            return Err(DatabaseError::NotADirectory(destination_parent));
        }

        let item_name = source_path
            .file_name()
            .ok_or_else(|| DatabaseError::NotAFile(source_path.clone()))?
            .to_string_lossy()
            .to_string();

        let destination_absolute = destination_parent.join(&item_name);
        let destination_relative = if to.get_name().is_empty() {
            PathBuf::from(&item_name)
        } else {
            let mut relative = self.locate_relative(&to)?.to_path_buf();
            relative.push(&item_name);
            relative
        };

        if destination_absolute.exists()
            || self
                .items
                .get(&item_name)
                .is_some_and(|paths| paths.iter().any(|path| path == &destination_relative))
        {
            return Err(DatabaseError::IdAlreadyExists(item_name));
        }

        if source_path.is_dir() {
            copy_directory_recursive(&source_path, &destination_absolute)?;
        } else if source_path.is_file() {
            fs::copy(&source_path, &destination_absolute)?;
        } else {
            return Err(DatabaseError::NoMatchingID(
                source_path.display().to_string(),
            ));
        }

        self.items
            .entry(item_name)
            .or_default()
            .push(destination_relative);

        Ok(())
    }

    /// Duplicates a managed item into `parent` using a caller-provided `name`.
    ///
    /// # Parameters
    /// - `id`: source item to duplicate.
    /// - `parent`: destination parent directory item (or `ItemId::database_id()`).
    /// - `name`: new name for the duplicate.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `id` is root or cannot be found,
    /// - destination parent is not a directory,
    /// - destination `name` already exists in the target directory,
    /// - filesystem copy fails.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("a.txt"), ItemId::database_id())?;
    ///     manager.duplicate_item(ItemId::id("a.txt"), ItemId::database_id(), "copy.txt")?;
    ///     Ok(())
    /// }
    /// ```
    pub fn duplicate_item(
        &mut self,
        id: impl Into<ItemId>,
        parent: impl Into<ItemId>,
        name: impl AsRef<str>,
    ) -> Result<(), DatabaseError> {
        let id = id.into();
        let parent = parent.into();
        let name = name.as_ref().to_owned();

        if id.get_name().is_empty() {
            return Err(DatabaseError::RootIdUnsupported);
        }

        let source_absolute = self.locate_absolute(&id)?;
        let parent_absolute = self.locate_absolute(&parent)?;
        if !parent_absolute.is_dir() {
            return Err(DatabaseError::NotADirectory(parent_absolute));
        }

        let destination_absolute = parent_absolute.join(&name);
        let destination_relative = if parent.get_name().is_empty() {
            PathBuf::from(&name)
        } else {
            let mut path = self.locate_relative(&parent)?.to_path_buf();
            path.push(&name);
            path
        };

        if destination_absolute.exists()
            || self
                .items
                .get(&name)
                .is_some_and(|paths| paths.iter().any(|path| path == &destination_relative))
        {
            return Err(DatabaseError::IdAlreadyExists(name));
        }

        if source_absolute.is_dir() {
            copy_directory_recursive(&source_absolute, &destination_absolute)?;
        } else {
            fs::copy(&source_absolute, &destination_absolute)?;
        }

        self.items
            .entry(
                destination_relative
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
            )
            .or_default()
            .push(destination_relative);

        Ok(())
    }

    /// Returns filesystem metadata summary for a managed file or directory.
    ///
    /// Includes:
    /// - `name`/`extension`,
    /// - normalized size,
    /// - Unix timestamps and "time since" timestamps where available.
    ///
    /// # Parameters
    /// - `id`: item to inspect.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `id` cannot be found,
    /// - metadata lookup fails.
    ///
    /// # Examples
    /// ```no_run
    /// use file_database::{DatabaseError, DatabaseManager, ItemId};
    ///
    /// fn main() -> Result<(), DatabaseError> {
    ///     let mut manager = DatabaseManager::new(".", "database")?;
    ///     manager.write_new(ItemId::id("a.txt"), ItemId::database_id())?;
    ///     let _info = manager.get_file_information(ItemId::id("a.txt"))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn get_file_information(
        &self,
        id: impl Into<ItemId>,
    ) -> Result<FileInformation, DatabaseError> {
        let id = id.into();

        let path = self.locate_absolute(id)?;

        let metadata = fs::metadata(&path)?;

        let name = {
            let os = if path.is_dir() {
                path.file_name()
            } else {
                path.file_stem()
            };

            match os_str_to_string(os) {
                Ok(name) => Some(name),
                Err(_) => None,
            }
        };

        let extension = {
            if path.is_dir() {
                None
            } else {
                match os_str_to_string(path.extension()) {
                    Ok(extension) => Some(extension),
                    Err(_) => None,
                }
            }
        };

        let size = FileSize::from(metadata.len());

        let unix_created = sys_time_to_unsigned_int(metadata.created());
        let time_since_created = sys_time_to_time_since(metadata.created());

        let unix_last_opened = sys_time_to_unsigned_int(metadata.accessed());
        let time_since_last_opened = sys_time_to_time_since(metadata.accessed());

        let unix_last_modified = sys_time_to_unsigned_int(metadata.modified());
        let time_since_last_modified = sys_time_to_time_since(metadata.modified());

        Ok(FileInformation {
            name,
            extension,
            size,
            unix_created,
            time_since_created,
            unix_last_opened,
            time_since_last_opened,
            unix_last_modified,
            time_since_last_modified,
        })
    }

    /// Gets one specific path from a shared `name` + `index`.
    ///
    /// # Errors
    /// Returns an error if:
    /// - the shared `name` key does not exist,
    /// - `id.index` is out of bounds.
    fn resolve_path_by_id(&self, id: &ItemId) -> Result<&PathBuf, DatabaseError> {
        let matches = self
            .items
            .get(id.get_name())
            .ok_or_else(|| DatabaseError::NoMatchingID(id.as_string()))?;

        if id.get_index() >= matches.len() {
            return Err(DatabaseError::IndexOutOfBounds {
                id: id.as_string(),
                index: id.get_index(),
                len: matches.len(),
            });
        }

        Ok(&matches[id.get_index()])
    }

    /// Overwrites a file safely by using a temp file and rename.
    ///
    /// `write_fn` is responsible for writing bytes to the temporary file and returning
    /// the number of bytes written.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `path` points to a directory,
    /// - temp create/write/sync/rename fails.
    fn overwrite_path_atomic_with<F>(&self, path: &Path, write_fn: F) -> Result<u64, DatabaseError>
    where
        F: FnOnce(&mut File) -> Result<u64, DatabaseError>,
    {
        if path.is_dir() {
            return Err(DatabaseError::NotAFile(path.to_path_buf()));
        }

        let buffer = path.with_extension("tmp");

        let result = (|| {
            let mut file = File::create(&buffer)?;
            let bytes_written = write_fn(&mut file)?;
            file.sync_all()?;
            fs::rename(&buffer, path)?;
            Ok(bytes_written)
        })();

        if result.is_err() && buffer.exists() {
            let _ = remove_file(&buffer);
        }

        result
    }

    /// Collects relative file and folder paths in the scan area.
    ///
    /// # Parameters
    /// - `scope_absolute`: absolute root directory for collection.
    /// - `recursive`: whether to include descendants recursively.
    ///
    /// # Errors
    /// Returns an error if reading folders fails or converting to a relative prefix fails.
    fn collect_paths_in_scope(
        &self,
        scope_absolute: &Path,
        recursive: bool,
    ) -> Result<Vec<PathBuf>, DatabaseError> {
        let mut collected = Vec::new();

        if recursive {
            let mut stack = vec![scope_absolute.to_path_buf()];
            while let Some(directory) = stack.pop() {
                for entry in fs::read_dir(&directory)? {
                    let entry = entry?;
                    let absolute_path = entry.path();
                    let relative_path = absolute_path.strip_prefix(&self.path)?.to_path_buf();

                    if absolute_path.is_dir() {
                        collected.push(relative_path);
                        stack.push(absolute_path);
                    } else if absolute_path.is_file() {
                        collected.push(relative_path);
                    }
                }
            }
        } else {
            for entry in fs::read_dir(scope_absolute)? {
                let entry = entry?;
                let absolute_path = entry.path();
                let relative_path = absolute_path.strip_prefix(&self.path)?.to_path_buf();

                if absolute_path.is_dir() || absolute_path.is_file() {
                    collected.push(relative_path);
                }
            }
        }

        Ok(collected)
    }
}

// -------- Functions --------
/// Removes `steps` trailing segments from `path`.
///
/// # Errors
/// Returns [`DatabaseError::PathStepOverflow`] when `steps` is too large for `path`.
fn truncate(mut path: PathBuf, steps: i32) -> Result<PathBuf, DatabaseError> {
    let parents = (path.ancestors().count() - 1) as i32;

    if parents <= steps {
        return Err(DatabaseError::PathStepOverflow(steps, parents));
    }

    for _ in 0..steps {
        path.pop();
    }

    Ok(path)
}

/// Converts an optional `OsStr` into an owned `String`.
///
/// # Errors
/// Returns [`DatabaseError::OsStringConversion`] if the value is `None` or invalid UTF-8.
fn os_str_to_string(os_str: Option<&OsStr>) -> Result<String, DatabaseError> {
    let os_str = match os_str {
        Some(os_str) => os_str,
        None => return Err(DatabaseError::OsStringConversion),
    };

    match os_str.to_os_string().into_string() {
        Ok(string) => Ok(string),
        Err(_) => Err(DatabaseError::OsStringConversion),
    }
}

/// Converts `SystemTime` to Unix timestamp seconds.
///
/// Returns `None` for platform or conversion failures.
fn sys_time_to_unsigned_int(time: io::Result<SystemTime>) -> Option<u64> {
    match time {
        Ok(time) => match time.duration_since(UNIX_EPOCH) {
            Ok(duration) => Some(duration.as_secs()),
            Err(_) => None,
        },
        Err(_) => None,
    }
}

/// Converts `SystemTime` to "time since now" represented as Unix-seconds duration.
///
/// Returns `None` for platform or conversion failures.
fn sys_time_to_time_since(time: io::Result<SystemTime>) -> Option<u64> {
    let duration = match time {
        Ok(time) => match SystemTime::now().duration_since(time) {
            Ok(duration) => duration,
            Err(_) => return None,
        },
        Err(_) => return None,
    };

    sys_time_to_unsigned_int(Ok(UNIX_EPOCH + duration))
}

/// Recursively copies a directory tree from `from` to `to`.
///
/// # Errors
/// Returns **`DatabaseError`** if reading folders or copying files fails.
fn copy_directory_recursive(from: &Path, to: &Path) -> Result<(), DatabaseError> {
    fs::create_dir_all(to)?;

    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = to.join(entry.file_name());

        if source_path.is_dir() {
            copy_directory_recursive(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)?;
        }
    }

    Ok(())
}

/// Returns whether `path` is inside the requested scan scope.
fn is_path_in_scope(path: &Path, scope_relative: Option<&Path>, recursive: bool) -> bool {
    match scope_relative {
        None => {
            if recursive {
                true
            } else {
                path.parent()
                    .is_some_and(|parent| parent.as_os_str().is_empty())
            }
        }
        Some(scope_relative) => {
            if recursive {
                path.starts_with(scope_relative) && path != scope_relative
            } else {
                path.parent() == Some(scope_relative)
            }
        }
    }
}

/// Deletes a directory `path` in forced or non-forced mode.
///
/// # Errors
/// Returns **`DatabaseError`** if the remove operation fails.
fn delete_directory<T>(path: &PathBuf, force: T) -> Result<(), DatabaseError>
where
    T: Into<bool>,
{
    if force.into() {
        return Ok(remove_dir_all(path)?);
    } else {
        return Ok(remove_dir(path)?);
    }
}
