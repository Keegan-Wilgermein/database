use std::{error::Error};
use database::*;

fn main() -> Result<(), Box<dyn Error>> {
    let path = GenPath::from_exe(2)?;

    let manager = DatabaseManager::new(&path, "database")?;

    println!("{:?}", manager.locate());

    Ok(())
}
