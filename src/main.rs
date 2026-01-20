use std::{env::current_dir, error::Error};
use database::*;

fn main() -> Result<(), Box<dyn Error>> {
    let path = current_dir().unwrap();
    let path = path.as_os_str().to_str().unwrap();
    let path = format!("{}/database", path);
    let manager = DatabaseManager::new(path)?;

    println!("{:?}", manager.locate());

    Ok(())
}
