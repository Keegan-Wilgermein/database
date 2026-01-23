use std::{error::Error};
use database::*;

fn main() -> Result<(), Box<dyn Error>> {
    let path = GenPath::from_closest_name("database")?;

    let mut manager = DatabaseManager::new(&path, "database")?;

    manager.write_new("one", ItemId::database_id())?;
    manager.write_new("two", ItemId::database_id())?;
    manager.write_new("1", ItemId::database_id())?;
    manager.write_new("2", ItemId::id("one"))?;
    manager.write_new("ten", ItemId::id("one"))?;
    manager.write_new("something", ItemId::id("one"))?;

    let child_of_parent = manager.get_by_parent("one", ShouldSort::Sort)?;

    println!("{:?}", child_of_parent);

    manager.delete(ItemId::database_id(), ForceDeletion::Force)?;

    Ok(())
}
