//! # Database
//! `Database` is a file management system designed to make reading and writing to a local database easier

use std::{env::{current_dir, current_exe}, error::Error, fmt::Display, fs::{create_dir, remove_dir, remove_dir_all}, path::{Path, PathBuf}};

// -------- Enums --------
/// Used for generating errors on funtions that don't actually produce any errors
#[derive(Debug, PartialEq, Clone)]
enum Errors {
    PathStepOverflow,
    NoClosestDir,
}

impl Display for Errors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Errors::PathStepOverflow => write!(f, "Steps exceed length of path"),
            Errors::NoClosestDir => write!(f, "Name not found in path"),
        }
    }
}

impl Error for Errors {}

#[derive(Debug, PartialEq, Clone)]
pub enum ForceDeletion {
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
#[derive(PartialEq, Debug, Clone)]
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
    pub fn new<'a, P, T>(path: &'a P, name: T) -> Result<Self, Box<dyn Error>>
    where
        P: AsRef<Path>, PathBuf: From<&'a P>, P: ?Sized,
        T: AsRef<Path>, PathBuf: From<T>,
    {
        let mut path: PathBuf = path.into();

        path.push(name);

        create_dir(&path)?;

        let manager = Self {
            path: Box::new(path.into()),
        };

        Ok(manager)
    }

    /// Deletes the passed database
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
    /// 
    /// # Examples
    /// #### Removing database when force is false
    /// ```no_run
    /// # use database::DatabaseManager;
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///     # let path = "./folder/new_folder";
    ///     # let manager = DatabaseManager::new(&path, "Database")?;
    /// #
    ///     manager.delete_database(false);
    /// #
    ///     # Ok(())
    /// # }
    /// ```
    /// #### Removing database when `force` is true
    /// ```no_run
    /// # use database::DatabaseManager;
    /// # use std::error::Error;
    /// #
    /// # fn main() -> Result<(), Box<dyn Error>> {
    ///     # let path = "./folder/new_folder";
    ///     # let manager = DatabaseManager::new(&path, "Database")?;
    /// #
    ///     manager.delete_database(true);
    /// #
    ///     # Ok(())
    /// # }
    /// ```
    pub fn delete_database<T>(self, force: T) -> Result<(), Box<dyn Error>>
    where
        T: Into<bool>,
    {
        if force.into() {
            remove_dir_all(*self.path)?;
        } else {
            remove_dir(*self.path)?;
        }

        Ok(())
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

    pub fn get_children() {
        
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
