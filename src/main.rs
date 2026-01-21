use std::{error::Error};
use database::*;

fn main() -> Result<(), Box<dyn Error>> {
    let path = GenPathFrom::working_dir("database", 0)?;

    let manager = DatabaseManager::new(path)?;

    println!("{:?}", manager);

    Ok(())
}
