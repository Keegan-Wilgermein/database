use std::{error::Error};
use database::*;

fn main() -> Result<(), Box<dyn Error>> {
    let path = GenPath::working_dir(0)?;

    let manager = DatabaseManager::new(path, "Database")?;

    println!("{:?}", manager.locate());

    Ok(())
}
