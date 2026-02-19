use std::{
    env,
    error::Error,
    fs,
    io,
    path::Path,
    time::{Duration, Instant},
};

use database::*;

const DEFAULT_RUNS: u32 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Interactive,
    Bench,
}

#[derive(Debug, Clone, Copy)]
struct Config {
    mode: Mode,
    runs: u32,
}

fn parse_config() -> Result<Config, Box<dyn Error>> {
    let mut mode = Mode::Bench;
    let mut runs = DEFAULT_RUNS;

    let mut args = env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "interactive" | "--interactive" | "--mode=interactive" => {
                mode = Mode::Interactive;
                runs = 1;
            }
            "bench" | "--bench" | "--mode=bench" => {
                mode = Mode::Bench;
            }
            "--mode" => {
                let value = args
                    .next()
                    .ok_or("Missing value after --mode (use interactive or bench)")?;
                match value.as_str() {
                    "interactive" => {
                        mode = Mode::Interactive;
                        runs = 1;
                    }
                    "bench" => mode = Mode::Bench,
                    _ => return Err("Invalid --mode value (use interactive or bench)".into()),
                }
            }
            "--runs" => {
                let value = args
                    .next()
                    .ok_or("Missing value after --runs")?;
                runs = value.parse::<u32>()?;
            }
            value if value.starts_with("--runs=") => {
                runs = value[7..].parse::<u32>()?;
            }
            _ => {
                return Err(
                    "Usage: cargo run -- [interactive|bench] [--mode interactive|bench] [--runs N]"
                        .into(),
                )
            }
        }
    }

    if runs == 0 {
        return Err("--runs must be greater than 0".into());
    }

    if mode == Mode::Interactive {
        runs = 1;
    }

    Ok(Config { mode, runs })
}

fn step(interactive: bool, message: impl AsRef<str>) -> Result<(), Box<dyn Error>> {
    if interactive {
        let message = message.as_ref();
        println!("\n[STEP] {message}");
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
    }
    Ok(())
}

fn run_scenario(path: &Path, interactive: bool) -> Result<Duration, Box<dyn Error>> {
    let db_path = path.join("database");
    if db_path.exists() {
        fs::remove_dir_all(&db_path)?;
    }

    let start = Instant::now();

    let mut database = DatabaseManager::new(path, "database")?;
    if interactive {
        println!("DatabaseManager::new OK");
    }
    step(interactive, "Created DatabaseManager (press Enter)")?;

    let test_folder_name = "test_folder";
    let test_folder = ItemId::id(test_folder_name);
    database.write_new(&test_folder, ItemId::database_id())?;
    if interactive {
        println!("write_new folder OK: {:?}", test_folder);
    }
    step(interactive, format!("Created folder {} (press Enter)", test_folder_name))?;

    let test_file_name = "test_file.txt";
    let test_file = ItemId::id(test_file_name);
    database.write_new(&test_file, &test_folder)?;
    if interactive {
        println!("write_new nested file OK: {:?}", test_file);
    }
    step(interactive, format!("Created nested {} (press Enter)", test_file_name))?;

    let root_test_file = ItemId::with_index(test_file_name, 1);
    database.write_new(ItemId::id(test_file_name), ItemId::database_id())?;
    if interactive {
        println!("write_new root file OK: {:?}", root_test_file);
    }
    step(interactive, format!("Created root {} (press Enter)", test_file_name))?;

    let renamed_root_name = "renamed_root.txt";
    let renamed_root = ItemId::id(renamed_root_name);

    let all = database.get_all(ShouldSort::Sort);
    if interactive {
        println!("get_all => {:?}", all);
    }
    step(interactive, "Fetched all IDs (press Enter)")?;

    let root_children = database.get_by_parent(ItemId::database_id(), ShouldSort::Sort)?;
    if interactive {
        println!("get_by_parent(root) => {:?}", root_children);
    }
    step(interactive, "Fetched root children (press Enter)")?;

    let folder_children = database.get_by_parent(&test_folder, ShouldSort::Sort)?;
    if interactive {
        println!("get_by_parent({}) => {:?}", test_folder_name, folder_children);
    }
    step(interactive, "Fetched folder children (press Enter)")?;

    let folder_relative = database.locate_relative(&test_folder)?;
    let folder_absolute = database.locate_absolute(&test_folder)?;
    if interactive {
        println!("locate_relative({}) => {}", test_folder_name, folder_relative.display());
        println!("locate_absolute({}) => {}", test_folder_name, folder_absolute.display());
    }
    step(interactive, "Located folder paths (press Enter)")?;

    let file_relative = database.locate_relative(&test_file)?;
    let file_absolute = database.locate_absolute(&test_file)?;
    if interactive {
        println!("locate_relative({}) => {}", test_file_name, file_relative.display());
        println!("locate_absolute({}) => {}", test_file_name, file_absolute.display());
    }
    step(interactive, "Located file paths (press Enter)")?;

    let file_paths = database.get_paths_for_id(&test_file)?;
    if interactive {
        println!("get_paths_for_id({}) => {:?}", test_file_name, file_paths);
    }
    step(interactive, format!("Fetched shared paths for {} (press Enter)", test_file_name))?;

    let file_ids = database.get_ids_from_shared_id(&test_file)?;
    if interactive {
        println!("get_ids_from_shared_id({}) => {:?}", test_file_name, file_ids);
    }
    step(interactive, format!("Fetched shared IDs for {} (press Enter)", test_file_name))?;

    database.rename(&root_test_file, renamed_root_name)?;
    if interactive {
        println!("rename(root {} -> {}) OK", test_file_name, renamed_root_name);
    }
    step(interactive, "Renamed root file (press Enter)")?;

    database.overwrite_existing(&test_file, b"hello from overwrite_existing")?;
    if interactive {
        println!("overwrite_existing({}) OK", test_file_name);
    }
    step(interactive, "Overwrote file contents (press Enter)")?;

    let file_info = database.get_file_information(&test_file)?;
    if interactive {
        println!("get_file_information => {:?}", file_info);
    }
    step(interactive, "Fetched file information (press Enter)")?;

    let renamed_name = "renamed.txt";
    database.rename(&test_file, renamed_name)?;
    let renamed = ItemId::id(renamed_name);
    if interactive {
        println!("rename({} -> {}) OK", test_file_name, renamed_name);
    }
    step(interactive, "Renamed nested file (press Enter)")?;

    let parent = database.get_parent(&renamed)?;
    if interactive {
        println!("get_parent({}) => {:?}", renamed_name, parent);
    }
    step(interactive, "Fetched parent for renamed file (press Enter)")?;

    database.delete(&renamed, ForceDeletion::NoForce)?;
    step(interactive, "Deleted renamed nested file (press Enter)")?;
    database.delete(&renamed_root, ForceDeletion::NoForce)?;
    step(interactive, "Deleted renamed root file (press Enter)")?;
    database.delete(&test_folder, ForceDeletion::NoForce)?;
    step(interactive, "Deleted test folder (press Enter)")?;
    database.delete(ItemId::database_id(), ForceDeletion::Force)?;
    step(interactive, "Deleted database root (press Enter)")?;

    if interactive {
        println!("Cleanup OK");
    }

    Ok(start.elapsed())
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_config()?;
    let path = GenPath::from_closest_match("database")?;

    match config.mode {
        Mode::Interactive => {
            run_scenario(&path, true)?;
        }
        Mode::Bench => {
            let mut total = Duration::ZERO;
            for _ in 0..config.runs {
                total += run_scenario(&path, false)?;
            }

            let average = Duration::from_secs_f64(total.as_secs_f64() / config.runs as f64);
            println!("Runs: {}", config.runs);
            println!("Total: {:.3?}", total);
            println!("Average: {:.3?}", average);
        }
    }

    Ok(())
}
