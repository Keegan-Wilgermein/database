//! # Database
//! `Database` is a file management system designed to make reading and writing to a local database easier

use std::{collections::HashSet, error::Error, fs::{self, remove_dir, remove_dir_all}, path::{Path, PathBuf}};

// -------- Structs --------
#[derive(Debug, Default)]
pub struct DatabaseManager {
    path: Box<PathBuf>,
}

impl DatabaseManager {

    /// Creates a new directory at `path` and returns `Self`
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
    /// 
    /// fn main() -> Result<(), Box<dyn Error>> {
    ///     let path = "./folder/new_folder";
    ///     let manager = DatabaseManager::new(path)?;
    /// 
    ///     Ok(())
    /// }
    /// ```
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>>
    where
        PathBuf: From<P>
    {
        fs::create_dir(&path)?;

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
    ///     # let manager = DatabaseManager::new(path)?;
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
    ///     # let manager = DatabaseManager::new(path)?;
    /// #
    ///     manager.delete_database(true);
    /// #
    ///     # Ok(())
    /// # }
    /// ```
    pub fn delete_database(self, force: bool) -> Result<(), Box<dyn Error>> {
        if force {
            remove_dir_all(*self.path)?;
        } else {
            remove_dir(*self.path)?;
        }

        Ok(())
    }
}

// -------- Functions --------
