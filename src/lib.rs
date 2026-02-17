//! # Database
//! `Database` is a file management system designed to make reading and writing to a local database easier

use std::{collections::HashMap, env::{current_dir, current_exe}, ffi::OsStr, fs::{self, File, create_dir, remove_dir, remove_dir_all, remove_file}, hash::Hash, io::{self, Write}, path::{Path, PathBuf}, time::SystemTime};
use thiserror::Error;
use fs_more::{self, directory::{BrokenSymlinkBehaviour, CollidingSubDirectoryBehaviour, DestinationDirectoryRule, DirectoryMoveAllowedStrategies, DirectoryMoveByCopyOptions, DirectoryMoveOptions, SymlinkBehaviour, move_directory}, error::MoveDirectoryError, file::CollidingFileBehaviour};

// Constants
const ZERO: u64 = 0;
const THOUSAND: u64 = 1_000;
const MILLION: u64 = 1_000_000;
const BILLION: u64 = 1_000_000_000;
const TRILLION: u64 = 1_000_000_000_000;
const QUADRILLION: u64 = 1_000_000_000_000_000;

// -------- Enums --------
/// Error messages
#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("Steps '{0}' greater than path length '{1}'")]
    PathStepOverflow(i32, i32),
    #[error("Directory '{0}' not found along path to executable")]
    NoClosestDir(String),
    #[error("ID '{0}' doesn't point to a known path")]
    NoMatchingID(String),
    #[error("ID '{0}' already exists")]
    IdAlreadyExists(String),
    #[error("Path '{0}' doesn't point to a directory")]
    NotADirectory(PathBuf),
    #[error("Path '{0}' doesn't point to a file")]
    NotAFile(PathBuf),
    #[error("Couldn't convert OsString to String")]
    OsStringConversion,
    #[error("ID '{0}' doesn't have a parent")]
    NoParent(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    PathBufConversion(#[from] std::path::StripPrefixError),
    #[error(transparent)]
    MigrationError(#[from] MoveDirectoryError)
}

#[derive(Debug, PartialEq, Clone, Default)]
/// A replacement for `bool` simply for readability
pub enum ForceDeletion {
    #[default]
    Force,
    NoForce,
}

impl Into<bool> for ForceDeletion {
    fn into(self) -> bool {
        match self {
            ForceDeletion::Force => true,
            ForceDeletion::NoForce => false,
        }
    }
}

impl From<bool> for ForceDeletion {
    fn from(value: bool) -> Self {
        match value {
            true => ForceDeletion::Force,
            false => ForceDeletion::NoForce,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
/// A replacement for `bool` simply for readability
pub enum ShouldSort {
    #[default]
    Sort,
    NoSort,
}

impl Into<bool> for ShouldSort {
    fn into(self) -> bool {
        match self {
            ShouldSort::Sort => true,
            ShouldSort::NoSort => false,
        }
    }
}

impl From<bool> for ShouldSort {
    fn from(value: bool) -> Self {
        match value {
            true => ShouldSort::Sort,
            false => ShouldSort::NoSort,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
/// A replacement for `bool` simply for readability
pub enum Serialize {
    #[default]
    Serialize,
    NoSerialize,
}

impl Into<bool> for Serialize {
    fn into(self) -> bool {
        match self {
            Serialize::Serialize => true,
            Serialize::NoSerialize => false,
        }
    }
}

impl From<bool> for Serialize {
    fn from(value: bool) -> Self {
        match value {
            true => Serialize::Serialize,
            false => Serialize::NoSerialize,
        }
    }
}

#[derive(Debug, Default, PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
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
/// Automatic path generation
#[derive(PartialEq, Debug, Clone, Default)]
pub struct GenPath;

impl GenPath {
    /// Generates a path from the working directory
    /// # Params
    /// - Truncates the end of the path `steps` number of times
    /// # Errors
    /// This function returns an error when:
    /// - The working directory doesn't exist
    /// - User lacks permissions to access the working directory
    /// 
    /// **Note**: The function will still fail if the user can access the truncated directory but not the working directory
    /// - `Steps` is greater than the length of the path
    /// # Examples
    /// ```no_run
    /// # use database::*;
    /// # use std::error::Error;
    /// # use std::path::PathBuf;
    /// #
    /// # fn main() -> Result<(), DatabaseError> {
    /// #
    /// let working_dir = PathBuf::from("./folder1/folder2/folder3");
    /// let path = GenPath::from_working_dir(0)?;
    /// assert_eq!(working_dir, path);
    /// 
    /// let truncated = PathBuf::from("./folder1/folder2");
    /// let path = GenPath::from_working_dir(1)?;
    /// assert_eq!(truncated, path);
    /// #
    /// # Ok(())
    /// #
    /// # }
    /// ```
    pub fn from_working_dir(steps: i32) -> Result<PathBuf, DatabaseError> {
        let working_dir = truncate(current_dir()?, steps)?;

        Ok(working_dir)
    }

    /// Generates a path from the directory of the current executable
    /// # Params
    /// - `Steps` is used to truncate the end of the path the specified amount of times
    /// # Errors
    /// - `Steps` is greater than the length of the path
    /// # Examples
    /// ```no_run
    /// # use database::*;
    /// # use std::error::Error;
    /// # use std::path::PathBuf;
    /// #
    /// # fn main() -> Result<(), DatabaseError> {
    /// #
    /// let current_exe = PathBuf::from("./folder1/folder2/folder3");
    /// let path = GenPath::from_exe(0)?;
    /// assert_eq!(current_exe, path);
    /// 
    /// let truncated = PathBuf::from("./folder1/folder2");
    /// let path = GenPath::from_exe(1)?;
    /// assert_eq!(truncated, path);
    /// #
    /// # Ok(())
    /// #
    /// # }
    /// ```
    pub fn from_exe(steps: i32) -> Result<PathBuf, DatabaseError> {
        let exe = truncate(current_exe()?, steps + 1)?;

        Ok(exe)
    }

    /// Generates a `PathBuf` from the name of a directory
    /// 
    /// Looks for directories along the path to the current executable and returns the first match
    /// # Params
    /// - `name` of directory to look for
    /// 
    /// # Errors
    /// This function will return an error when:
    /// - `name` not found in path to current exe
    /// 
    /// This function will not return if a file matching `name` is found and will continue searchng until a directory is found or it finds nothing
    /// # Examples
    /// ```no_run
    /// # use database::*;
    /// # use std::path::PathBuf;
    /// // Exe location is ./folder/directory/other/exe
    /// let path = GenPath::from_closest_name("directory").unwrap();
    /// assert_eq!(path, PathBuf::from("./folder/directory"));
    /// ```
    pub fn from_closest_match(name: impl AsRef<Path>) -> Result<PathBuf, DatabaseError> {
        let exe = current_exe()?;

        for path in exe.ancestors() {
            if path.ends_with(&name) {
                if path.is_dir() {
                    return Ok(path.to_path_buf())
                }
            }
        }

        let name_as_string = match name.as_ref().to_owned().into_os_string().into_string() {
            Ok(string) => string,
            Err(_) => return Err(DatabaseError::OsStringConversion)
        };

        Err(DatabaseError::NoClosestDir(name_as_string))
    }
}

/// Item identification and lookup
#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct ItemId(String);

impl<T> From<T> for ItemId
where
    T: Into<String>,
{
    fn from(s: T) -> Self {
        ItemId(s.into())
    }
}

impl ItemId {
    /// Returns the ID used for the actual database
    /// 
    /// This is equivalant to an empty string
    pub fn database_id() -> Self {
        Self(String::new())
    }

    /// Returns `Self` with the given `id`
    pub fn id(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_string(self) -> String {
        self.0
    }
}

#[derive(Debug, Default, PartialEq, PartialOrd, Clone, Copy)]
pub struct FileSize {
    size: u64,
    unit: FileSizeUnit,
}

impl FileSize {
    pub fn get_size(&self) -> u64 {
        self.size
    }

    pub fn get_unit(&self) -> FileSizeUnit {
        self.unit
    }

    /// Returns the name of the stored `FileSizeUnit` as a `String`, appending an 's' if the `size` is greater than 1
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

    /// Recalculate size in a different unit
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

    /// Creates `FileSize` from input
    fn from(bytes: u64) -> Self {
        let (size, unit) = match bytes {
            ZERO..THOUSAND => (bytes, FileSizeUnit::Byte),
            THOUSAND..MILLION => (bytes / THOUSAND, FileSizeUnit::Kilobyte),
            MILLION..BILLION => (bytes / MILLION, FileSizeUnit::Megabyte),
            BILLION..TRILLION => (bytes / BILLION, FileSizeUnit::Gigabyte),
            TRILLION..QUADRILLION => (bytes / TRILLION, FileSizeUnit::Terabyte),
            _ => (bytes / QUADRILLION, FileSizeUnit::Petabyte),
        };

        Self {
            size,
            unit,
        }
    }
}

#[derive(Debug, Default, PartialEq, PartialOrd, Clone)]
/// Represents important file information
pub struct FileInformation {
    name: Option<String>,
    extension: Option<String>,
    size: FileSize,
    created: Option<u64>,
    last_opened: Option<u64>,
    last_modified: Option<u64>,
}

impl FileInformation {
    pub fn get_name(&self) -> Option<String> {
        self.name.clone()
    }

    pub fn get_extension(&self) -> Option<String> {
        self.extension.clone()
    }

    pub fn get_size(&self) -> FileSize {
        self.size.clone()
    }

    pub fn get_created(&self) -> Option<u64> {
        self.created.clone()
    }

    pub fn get_opened(&self) -> Option<u64> {
        self.last_opened.clone()
    }

    pub fn get_modified(&self) -> Option<u64> {
        self.last_modified.clone()
    }
}

#[derive(Debug, PartialEq)]
/// Manages the database it was created with
pub struct DatabaseManager {
    path: PathBuf,
    items: HashMap<ItemId, PathBuf>,
}

impl DatabaseManager {
    /// Creates a new directory at `path` and returns `Self`
    /// 
    /// # Params
    /// - Appends `name` to the end of `path`
    /// 
    /// **Note**: `name` is case insensitive
    /// - Creates a new directory at `path`
    /// 
    /// # Errors
    /// This function returns an error when:
    /// - Any parent directory in `path` doesn't exist
    /// - `path` already exists
    /// - The user lacks permission to write at `path`
    /// 
    /// # Examples
    /// #### Creating a new `DatabaseManager`
    /// ```no_run
    /// # use database::DatabaseManager;
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), DatabaseError> {
    /// let path = "./folder/other_folder";
    /// // Creates a folder at "./folder/other_folder/database"
    /// let manager = DatabaseManager::new(&path, "database")?;
    /// #
    /// # Ok(())
    /// # }
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

    /// Creates a new file or folder
    pub fn write_new(&mut self, id: impl Into<ItemId>, parent: impl Into<ItemId>) -> Result<(), DatabaseError> {
        let id = id.into();
        let parent = parent.into();

        if self.items.contains_key(&id) {
            return Err(DatabaseError::IdAlreadyExists(id.as_string()));
        }

        let absolute_parent_path = self.locate_absolute(parent.clone())?;
        let relative_parent_path = self.locate_relative(parent)?;
        let mut absolute_path: PathBuf;
        let mut relative_path: PathBuf;

        // If parent isn't root
        if absolute_parent_path != self.locate_absolute(ItemId::database_id())? {
            relative_path = relative_parent_path;
            relative_path.push(&id.0);
        } else {
            relative_path = PathBuf::from(&id.0);
        }

        absolute_path = absolute_parent_path;
        absolute_path.push(&id.0);

        if relative_path.extension().is_none() {
            create_dir(&absolute_path)?;
        } else {
            File::create_new(&absolute_path)?;
        }

        self.items.insert(id, relative_path);
        Ok(())
    }

    /// Overwrite an existing file with new data
    pub fn overwrite_existing<T>(&self, id: impl Into<ItemId>, data: T) -> Result<(), DatabaseError>
    where
        T: AsRef<[u8]>,
    {
        let id = id.into();

        let path = self.locate_absolute(id)?;

        if path.is_dir() {
            return Err(DatabaseError::NotAFile(path));
        }

        let buffer = path.with_extension("tmp");

        let mut file = File::create(&buffer)?;
        file.write_all(data.as_ref())?;
        file.sync_all()?;
        fs::rename(&buffer, &path)?;

        Ok(())
    }

    // pub fn read<T>(&self, id: impl Into<ItemId>) -> Result<T, DatabaseError> {
    //     let id = id.into();

    //     let path = self.locate(id)?;

    //     if path.is_dir() {
    //         return Err(DatabaseError::NotAFile(path));
    //     }

    //     let data = fs::read_to_string(path)?;

    //     Ok(deserialized_data)
    // }

    /// Returns all existing `ItemId` as a `Vec<ItemId>`
    pub fn get_all(&self, sorted: impl Into<bool>) -> Vec<ItemId> {
        let sorted = sorted.into();

        let mut list: Vec<ItemId> = self.items.keys().map(|key| key.to_owned()).collect();

        if sorted {
            list.sort();
        }

        list
    }

    /// Returns all `ItemId` that belong to a certain parent
    /// 
    /// Empty strings are returned if there is an error reading the item
    pub fn get_by_parent(&self, parent: impl Into<ItemId>, sorted: impl Into<bool>) -> Result<Vec<ItemId>, DatabaseError> {
        let parent = parent.into();

        let path = self.locate_absolute(parent)?;

        if !path.is_dir() {
            return Err(DatabaseError::NotADirectory(path))
        }

        let mut list: Vec<ItemId> = path.read_dir()?.map(|directory|{
            match directory {
                Ok(path) => {
                    let path_name_as_string = match path.file_name().into_string() {
                        Ok(string) => string,
                        Err(_) => String::new(),
                    };

                    path_name_as_string.into()
                },
                Err(_) => ItemId::database_id(),
            }
        }).collect();

        if sorted.into() {
            list.sort();
        }

        Ok(list)
    }

    pub fn get_parent(&self, id: impl Into<ItemId>) -> Result<ItemId, DatabaseError> {
        let id = id.into();
        let path = self.locate_absolute(id.clone())?;

        let parent = match path.parent() {
            Some(parent) => {
                let string = os_str_to_string(parent.file_name())?;
                ItemId::id(string)
            },
            None => return Err(DatabaseError::NoParent(id.0)),
        };

        Ok(parent)
    }

    pub fn rename(&mut self, id: impl Into<ItemId>, to: impl AsRef<str>) -> Result<(), DatabaseError> {
        let id = id.into();
        let name = to.as_ref().to_owned();

        let path = self.locate_absolute(id.clone())?;
        let mut relative_path = self.locate_relative(id.clone())?;
        
        match self.items.remove_entry(&id) {
            Some(_) => (),
            None => return Err(DatabaseError::NoMatchingID(id.0)),
        }
        
        relative_path = match relative_path.pop() {
            true => {
                relative_path.push(&name);
                PathBuf::from(relative_path)
            },
            false => {
                PathBuf::from(&name)
            }
        };
        
        self.items.insert(ItemId::id(name.clone()), relative_path);

        fs::rename(&path, self.locate_absolute(name)?)?;

        Ok(())
    }

    /// Deletes a directory or a file
    /// 
    /// Pass `""` or equivalent as `id` to delete database
    pub fn delete(&mut self, id: impl Into<ItemId>, force: impl Into<bool>) -> Result<(), DatabaseError> {
        let id = id.into();

        if id.0.is_empty() {
            match delete_directory(&self.locate_absolute(id)?, force) {
                Ok(_) => {
                    self.path = PathBuf::new();
                    self.items.drain();
                    return Ok(());
                },
                Err(error) => return Err(error),
            }
        }

        let path = self.locate_absolute(id.clone())?;

        self.items.remove(&id);

        if path.is_dir() {
            delete_directory(&path, force)?;
            return Ok(());
        }

        remove_file(path)?;

        Ok(())
    }

    /// Locate the database by id and return an absolute path
    pub fn locate_absolute(&self, id: impl Into<ItemId>) -> Result<PathBuf, DatabaseError> {
        let id = id.into();

        let database_path = self.path.clone();

        if id.0.is_empty() {
            return Ok(database_path);
        }

        let location = self.items.get(&id);

        if let Some(path) = location {
            let mut absolute_path = database_path;
            absolute_path.push(path);
            Ok(absolute_path)
        } else {
            Err(DatabaseError::NoMatchingID(id.as_string()))
        }
    }

    /// Locate the database by id and return a relative path
    /// 
    /// An absolute path will be output if `id` matches the database ID
    pub fn locate_relative(&self, id: impl Into<ItemId>) -> Result<PathBuf, DatabaseError> {
        let id = id.into();

        let database_path = self.path.clone();

        if id.0.is_empty() {
            return Ok(database_path);
        }

        let location = self.items.get(&id);

        if let Some(path) = location {
            Ok(path.to_path_buf())
        } else {
            Err(DatabaseError::NoMatchingID(id.as_string()))
        }
    }

    /// Migrate the database to a different directory overwriting any collisions
    pub fn migrate(&mut self, to: impl AsRef<Path>) -> Result<(), DatabaseError> {
        let destination = to.as_ref().to_path_buf();

        let move_options = DirectoryMoveOptions {
            destination_directory_rule: DestinationDirectoryRule::AllowNonEmpty {
                colliding_file_behaviour: CollidingFileBehaviour::Overwrite,
                colliding_subdirectory_behaviour: CollidingSubDirectoryBehaviour::Continue
            },
            allowed_strategies: DirectoryMoveAllowedStrategies::OnlyCopyAndDelete {
                options: DirectoryMoveByCopyOptions {
                    symlink_behaviour: SymlinkBehaviour::Keep,
                    broken_symlink_behaviour: BrokenSymlinkBehaviour::Keep
                }
            }
        };

        move_directory(&self.path, &destination, move_options)?;
        
        self.path = destination;

        Ok(())
    }

    /// Returns the information about a folder or file
    pub fn get_file_information(&self, id: impl Into<ItemId>) -> Result<FileInformation, DatabaseError> {
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

        let created = sys_time_to_unsigned_int(metadata.created());

        let last_opened = sys_time_to_unsigned_int(metadata.accessed());

        let last_modified = sys_time_to_unsigned_int(metadata.modified());
    
        Ok(FileInformation {
            name,
            extension,
            size,
            created,
            last_opened,
            last_modified,
        })
    }
}

// -------- Functions --------
/// Truncates the end of a path the specified amount of times
fn truncate(mut path: PathBuf, steps: i32) -> Result<PathBuf, DatabaseError> {
    let parents = (path.ancestors().count() - 1) as i32;

    if parents <= steps {
        return Err(DatabaseError::PathStepOverflow(steps, parents))
    }

    for _ in 0..steps {
        path.pop();
    }

    Ok(path)
}

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

fn sys_time_to_unsigned_int(time: io::Result<SystemTime>) -> Option<u64> {
    match time {
        Ok(time) => {
            match time.elapsed() {
                Ok(duration) => Some(duration.as_secs()),
                Err(_) => None,
            }
        },
        Err(_) => None,
    }
}

/// Deletes the passed directory
/// 
/// # Params
/// If `force` is true, all items in the database will be deleted
/// 
/// If `force` is false, the database will be deleted only if it is empty
/// 
/// # Errors
/// This function returns an error when:
/// - `path` doesn't exist 
/// - The user lacks permissions to write at `path`
/// 
/// #### If `force` is false
/// - `path` is not empty
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
