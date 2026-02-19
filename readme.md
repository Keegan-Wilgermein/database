# file_database

`file_database` is a local filesystem database for files and folders.

It manages one root directory and keeps an in-memory index so you can target items by ID instead of raw paths.

## What problem this solves

- It cuts down boilerplate for common file-management tasks.
- It gives you a simple ID-based way to track items instead of passing raw paths around.
- It hides low-level filesystem bookkeeping behind a single API, so day-to-day usage stays straightforward.

## Core idea: `ItemId`

An `ItemId` has two parts:

- `name`: shared key (example: `"test_file.txt"`)
- `index`: which match under that shared key

This means duplicate names are allowed. `ItemId::id("name")` always means index `0`. Use `ItemId::with_index("name", i)` for other matches.

`ItemId::database_id()` is the root ID that refers to the database root directory.

## Install

`Cargo.toml`:

```toml
[dependencies]
file_database = "0.1.0"
```

## Quick start

```rust,no_run
use file_database::{DatabaseError, DatabaseManager, ItemId};

fn main() -> Result<(), DatabaseError> {
    let mut db = DatabaseManager::new(".", "database")?;

    db.write_new(ItemId::id("notes.txt"), ItemId::database_id())?;
    db.overwrite_existing(ItemId::id("notes.txt"), b"hello world")?;

    let bytes = db.read_existing(ItemId::id("notes.txt"))?;
    assert_eq!(bytes, b"hello world");

    Ok(())
}
```

## Main API surface

### Create and organize

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
- `get_paths_for_id(id)`
- `get_ids_from_shared_id(id)`

### Read and write file data

- Raw bytes:
  - `overwrite_existing(id, data)`
  - `read_existing(id)`
- JSON:
  - `overwrite_existing_json(id, &value)`
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

- `ScanPolicy::DetectOnly`: report new files, do not index them
- `ScanPolicy::AddNew`: report and index new files
- `ScanPolicy::RemoveNew`: report and delete new files from disk

Important behavior: missing tracked items are always removed from the in-memory index during scan.

The result is `ScanReport` with:

- scanned scope (`scan_from`)
- `added`
- `removed`
- `unchanged_count`
- `total_changed_count`

## `GenPath` helper

`GenPath` helps build base paths for database setup:

- `GenPath::from_working_dir(steps)`
- `GenPath::from_exe(steps)`
- `GenPath::from_closest_match("dir_name")`

Example:

```rust,no_run
use file_database::{DatabaseError, DatabaseManager, GenPath};

fn main() -> Result<(), DatabaseError> {
    let root = GenPath::from_working_dir(0)?;
    let _db = DatabaseManager::new(root, "database")?;
    Ok(())
}
```

## Errors

All fallible functions return `Result<_, DatabaseError>`.

Common variants include:

- `NoMatchingID`
- `IndexOutOfBounds`
- `NotADirectory`
- `NotAFile`
- `IdAlreadyExists`
- `RootIdUnsupported`
- `Io`
- `SerdeJson`
- `Bincode`

## Notes on indexing behavior

- `index` is zero-based.
- Vectors preserve insertion order.
- Removing or renaming items can shift later indexes for the same `name`.
- If you need all indexes for one shared name, call `get_ids_from_shared_id`.

## License

This crate is licensed under the MIT licesne
