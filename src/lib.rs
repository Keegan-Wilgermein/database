//! # Database
//! `Database` is a file management system designed to make reading and writing to a local database easier

use std::{borrow::Borrow, collections::HashMap, default, env::{current_dir, current_exe}, error::Error, fmt::Display, fs::{create_dir, remove_dir, remove_dir_all, remove_file}, hash::Hash, path::{Path, PathBuf}};

// -------- Enums --------
/// Used for generating errors on funtions that don't actually produce any errors
#[derive(Debug, PartialEq, Clone)]
enum Errors {
    PathStepOverflow,
    NoClosestDir,
    NoMatchingID,
}

impl Display for Errors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Errors::PathStepOverflow => write!(f, "Steps exceed length of path"),
            Errors::NoClosestDir => write!(f, "Name not found in path"),
            Errors::NoMatchingID => write!(f, "No item matching ID exists"),
        }
    }
}

impl Error for Errors {}

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

// -------- Structs --------
/// Used for generating paths
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
    /// # fn main() -> Result<(), Box<dyn Error>> {
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
    pub fn from_working_dir<T>(steps: T) -> Result<PathBuf, Box<dyn Error>>
    where
        i32: From<T>,
    {

        let working_dir = truncate(current_dir()?, steps.into())?;

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
    /// # fn main() -> Result<(), Box<dyn Error>> {
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
    pub fn from_exe<T>(steps: T) -> Result<PathBuf, Box<dyn Error>>
    where
        i32: From<T>,
    {
        let steps: i32 = steps.into();

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
    /// This function will not return if a file matching `name` is found and will continue searchng until a directory is found or it returns the above error
    /// # Examples
    /// ```no_run
    /// # use database::*;
    /// # use std::path::PathBuf;
    /// // Exe location is ./folder/directory/other/exe
    /// let path = GenPath::from_closest_name("directory").unwrap();
    /// assert_eq!(path, PathBuf::from("./folder/directory"));
    /// ```
    pub fn from_closest_name<P>(name: P) -> Result<PathBuf, Box<dyn Error>>
    where
        P: AsRef<Path>, PathBuf: From<P>,
    {
        let exe = current_exe()?;

        for path in exe.ancestors() {
            if path.ends_with(&name) {
                if path.is_dir() {
                    return Ok(path.to_path_buf())
                }
            }
        }

        Err(Errors::NoClosestDir.into())
    }
}

#[derive(Debug, PartialEq)]
/// Manages the database it was created with
pub struct DatabaseManager {
    path: Box<PathBuf>,
    items: HashMap<String, PathBuf>,
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
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// let path = "./folder/other_folder";
    /// // Creates a folder at "./folder/other_folder/database"
    /// let manager = DatabaseManager::new(&path, "database")?;
    /// #
    /// # Ok(())
    /// # }
    /// ```
    pub fn new<P, T>(path: P, name: T) -> Result<Self, Box<dyn Error>>
    where
        P: AsRef<Path>, PathBuf: From<P>,
        T: AsRef<Path>, PathBuf: From<T>,
    {
        let mut path: PathBuf = path.into();

        path.push(name);

        create_dir(&path)?;

        let manager = Self {
            path: Box::new(path.into()),
            items: HashMap::new(),
        };

        Ok(manager)
    }

    /// Locates the path to the managed database
    /// # Examples
    /// ```no_run
    /// # use database::DatabaseManager;
    /// # use std::error::Error;
    /// # use std::path::PathBuf;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///     let mut path = PathBuf::from("./folder/new_folder");
    ///     let manager = DatabaseManager::new(&path, "database")?;
    /// 
    ///     path.push("database");
    ///     assert_eq!(manager.locate(), path);
    /// #
    ///     # Ok(())
    /// # }
    /// ```
    pub fn locate(&self) -> PathBuf {
        *self.path.clone()
    }

    pub fn write(&mut self,) -> Option<Box<dyn Error>> {
        todo!();
    }

    /// Deletes a directory or a file
    /// 
    /// Pass `""` or equivalent as `id` to delete database
    pub fn delete<'a, K, T>(&mut self, id: &'a K, force: T) -> Option<Box<dyn Error>>
    where
        T: Into<bool>,
        K: AsRef<Path>, PathBuf: From<&'a K>, String: Borrow<K>, K: Eq, K: Hash, K: ?Sized, &'a K: PartialEq<&'a str>,
    {
        if id == "" {
            match delete_directory(&self.locate(), force) {
                Some(error) => return Some(error),
                None => return None,
            }
        }

        let path = match self.locate_item(id) {
            Ok(path) => path,
            Err(error) => return Some(error),
        };

        if path.is_dir() {
            match delete_directory(&path, force) {
                Some(error) => return Some(error),
                None => return None,
            }
        }

        match remove_file(path) {
            Ok(_) => return None,
            Err(error) => return Some(error.into()),
        }
    }

    pub fn locate_item<K>(&self, id: &K) -> Result<PathBuf, Box<dyn Error>>
    where
        String: Borrow<K>,
        K: Eq,
        K: Hash,
        K: ?Sized,
    {
        let location = self.items.get(id);

        if let Some(path) = location {
            Ok(path.clone())
        } else {
            Err(Errors::NoMatchingID.into())
        }
    }
}

// -------- Functions --------
/// Truncates the end of a path the specified amount of times
fn truncate(mut path: PathBuf, steps: i32) -> Result<PathBuf, Errors> {
    let parents = path.ancestors().count() - 1;

    if parents as i32 <= steps {
        return Err(Errors::PathStepOverflow)
    }

    for _ in 0..steps {
        path.pop();
    }

    Ok(path)
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
fn delete_directory<T>(path: &PathBuf, force: T) -> Option<Box<dyn Error>>
where
    T: Into<bool>,
{
    if force.into() {
        match remove_dir_all(path) {
            Ok(_) => (),
            Err(error) => return Some(error.into()),
        };
    } else {
        match remove_dir(path) {
            Ok(_) => (),
            Err(error) => return Some(error.into()),
        };
    }

    None
}
