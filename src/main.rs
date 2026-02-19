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

fn step(interactive: bool, message: &str) -> Result<(), Box<dyn Error>> {
    if interactive {
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

    let test_folder = ItemId::id("test_folder");
    database.write_new(&test_folder, ItemId::database_id())?;
    if interactive {
        println!("write_new folder OK: {:?}", test_folder);
    }
    step(interactive, "Created folder test_folder (press Enter)")?;

    let test_file = ItemId::id("test_file.txt");
    database.write_new(&test_file, &test_folder)?;
    if interactive {
        println!("write_new nested file OK: {:?}", test_file);
    }
    step(interactive, "Created nested test_file.txt (press Enter)")?;

    let root_test_file = ItemId::with_index("test_file.txt", 1);
    database.write_new(ItemId::id("test_file.txt"), ItemId::database_id())?;
    if interactive {
        println!("write_new root file OK: {:?}", root_test_file);
    }
    step(interactive, "Created root test_file.txt (press Enter)")?;

    database.rename(&root_test_file, "renamed_root.txt")?;
    let renamed_root = ItemId::id("renamed_root.txt");
    if interactive {
        println!("rename(root test_file.txt -> renamed_root.txt) OK");
    }
    step(interactive, "Renamed root file (press Enter)")?;

    let all = database.get_all(ShouldSort::Sort);
    if interactive {
        println!("get_all => {:?}", all);
    }

    let root_children = database.get_by_parent(ItemId::database_id(), ShouldSort::Sort)?;
    if interactive {
        println!("get_by_parent(root) => {:?}", root_children);
    }

    let folder_children = database.get_by_parent(&test_folder, ShouldSort::Sort)?;
    if interactive {
        println!("get_by_parent(test_folder) => {:?}", folder_children);
    }

    let folder_relative = database.locate_relative(&test_folder)?;
    let folder_absolute = database.locate_absolute(&test_folder)?;
    if interactive {
        println!("locate_relative(test_folder) => {}", folder_relative.display());
        println!("locate_absolute(test_folder) => {}", folder_absolute.display());
    }

    let file_relative = database.locate_relative(&test_file)?;
    let file_absolute = database.locate_absolute(&test_file)?;
    if interactive {
        println!("locate_relative(test_file.txt) => {}", file_relative.display());
        println!("locate_absolute(test_file.txt) => {}", file_absolute.display());
    }

    let file_paths = database.get_paths_for_id(&test_file)?;
    if interactive {
        println!("get_paths_for_id(test_file.txt) => {:?}", file_paths);
    }

    database.overwrite_existing(&test_file, b"hello from overwrite_existing")?;
    if interactive {
        println!("overwrite_existing(test_file.txt) OK");
    }

    let file_info = database.get_file_information(&test_file)?;
    if interactive {
        println!("get_file_information => {:?}", file_info);
    }

    database.rename(&test_file, "renamed.txt")?;
    let renamed = ItemId::id("renamed.txt");
    if interactive {
        println!("rename(test_file.txt -> renamed.txt) OK");
    }

    let parent = database.get_parent(&renamed)?;
    if interactive {
        println!("get_parent(renamed.txt) => {:?}", parent);
    }

    database.delete(&renamed, ForceDeletion::NoForce)?;
    database.delete(&renamed_root, ForceDeletion::NoForce)?;
    database.delete(&test_folder, ForceDeletion::NoForce)?;
    database.delete(ItemId::database_id(), ForceDeletion::Force)?;

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
            let elapsed = run_scenario(&path, true)?;
            println!("Total elapsed from first to last call: {:.3?}", elapsed);
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
