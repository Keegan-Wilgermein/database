use std::{error::Error};
use database::*;

// #[derive(Default, Debug)]
// struct Test {
//     one: i32,
//     two: String,
// }

fn main() -> Result<(), Box<dyn Error>> {
    let path = GenPath::from_closest_match("database")?;

    let mut database = DatabaseManager::new(&path, "database")?;

    // let file = Test::default();

    let test_id = ItemId::id("test_folder");

    database.write_new(&test_id, ItemId::database_id())?;
    database.write_new("test_file.txt", &test_id)?;

    println!("{:?}", database.get_all(ShouldSort::Sort));
    
    // database.overwrite_existing("test.json", file);
    
    // database.delete(ItemId::database_id(), ForceDeletion::Force)?;

    Ok(())
}
