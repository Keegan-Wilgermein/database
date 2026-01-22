use std::{error::Error};
use database::*;

fn main() -> Result<(), Box<dyn Error>> {
    let path = GenPath::from_closest_name("database")?;

    let manager = DatabaseManager::new(&path, "database")?;

    println!("{:?}", manager.locate());

    manager.delete_database(ForceDeletion::NoForce)?;

    Ok(())
}
