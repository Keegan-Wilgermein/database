# file_database

`file_database` is a local filesystem database for files and folders.

It manages one root directory and keeps an in-memory index so you can target items by ID instead of raw paths.

## What problem this solves
It reduces the code needed for managing a file system by keeping track of everything for you

## Install

`Cargo.toml`:

```toml
[dependencies]
file_database = "1.0.0"
```

## Quick start

```rust
use file_database::{DatabaseError, DatabaseManager, ItemId};

fn main() -> Result<(), DatabaseError> {
  let mut db = DatabaseManager::create_database(".", "database")?;

    db.write_new(ItemId::id("notes.txt"), ItemId::database_id())?;
    db.overwrite_existing(ItemId::id("notes.txt"), b"hello world")?;

    let bytes = db.read_existing(ItemId::id("notes.txt"))?;
    assert_eq!(bytes, b"hello world");

    Ok(())
}
```

  ## `ItemId`
  
  An `ItemId` has two parts:
  
  - `name`: shared key (example: `"test_file.txt"`)
  - `index`: stable slot under that shared key
  
  This means duplicate names are allowed. `ItemId::id("name")` always means index `0`. Use `ItemId::with_index("name", i)` for a specific slot.
  When IDs are auto-generated (for example when opening an existing directory), file names keep the extension because indexing uses `file_name()`.
  
  `ItemId::database_id()` is the root ID that refers to the database root directory.

### Create and organize

`create_database` works as create-or-open:
if the directory does not exist, it creates it

if the directory already exists, it opens it and indexes current contents recursively

- `write_new(id, parent)`
- `rename(id, new_name)`
- `migrate_item(id, to_parent)`
- `duplicate_item(id, to_parent, new_name)`
- `delete(id, force)`

### Locate and list

- `locate_absolute(id)`
- `locate_relative(id)`
- `get_all(sorted)`
- `get_by_parent(parent, sorted)`
- `get_parent(id)`
- `get_ids_by_name(name)`
- `get_ids_by_index(index)`

### Read and write file data

- Raw bytes:
  - `overwrite_existing(id, data)`
  - `read_existing(id)`
- JSON:
  - `overwrite_existing_json(id, &value, pretty)`
  - `read_existing_json::<T>(id)`
- Binary (bincode):
  - `overwrite_existing_binary(id, &value)`
  - `read_existing_binary::<T>(id)`
- Streaming overwrite:
  - `overwrite_existing_from_reader(id, &mut reader)`

### Move across database boundaries

- `import_item(from_external_path, to_database_parent)`
- `export_item(id, to_external_directory, mode)` where `mode` is `ExportMode::Copy` or `ExportMode::Move`
- `migrate_database(new_parent_dir)`

### Metadata

- `get_file_information(id)` returns `FileInformation` with:
  - name and extension
  - normalized size (`FileSize`)
  - unix timestamps and `time_since_*` values when available

## Scan for external changes

If files are changed outside this library (for example, another tool drops files into the database), use:

- `scan_for_changes(scan_from, policy, recursive)`

Policy options:

- `ScanPolicy::DetectOnly`: detect new files, do not index them
- `ScanPolicy::AddNew`: detect and index new files
- `ScanPolicy::RemoveNew`: delete new files from disk and do not keep them in the `added` list

Important behavior: missing tracked items are always removed from the in-memory index during scan.

The result is `ScanReport` with:

- scanned scope (`scan_from`)
- `added`
- `removed`
- `unchanged_count`
- `total_changed_count`

## `GenPath`

`GenPath` helps build base paths for database setup:

- `GenPath::from_working_dir(steps)`
- `GenPath::from_exe(steps)`
- `GenPath::from_closest_match("dir_name")`

Example:

```rust
use file_database::{DatabaseError, DatabaseManager, GenPath};

fn main() -> Result<(), DatabaseError> {
    let root = GenPath::from_working_dir(0)?;
    let _db = DatabaseManager::create_database(root, "database")?;
    Ok(())
}
```

## Errors

All fallible functions return `Result<_, DatabaseError>`.

Common variants include:

- `NoMatchingID`
- `NotADirectory`
- `NotAFile`
- `IdAlreadyExists`
- `RootIdUnsupported`
- `Io`
- `SerdeJson`
- `Bincode`

## Notes on indexing behavior

- Internal storage is `HashMap<String, StableVec<PathBuf>>`.
- `index` is a stable slot in the per-name `StableVec`, not a shifting position in a plain `Vec`.
- Different `ItemId` values can share the same `name` and still point to different paths.
- If one item is removed, other occupied slots keep their index.
- If you need all IDs for one shared name, call `get_ids_by_name`.

## License

This crate is licensed under the MIT license

Feel free to create an issue in the repo if anything isn't working correctly

### Repo
[file_database](https://github.com/Keegan-Wilgermein/database)
