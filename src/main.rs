use std::{error::Error};
use database::*;

// #[derive(Default, Debug)]
// struct Test {
//     one: i32,
//     two: String,
// }

fn main() -> Result<(), Box<dyn Error>> {
    let path = GenPath::from_closest_name("database")?;

    let mut database = DatabaseManager::new(&path, "database")?;

    // let file = Test::default();

    database.write_new("test_folder", ItemId::database_id())?;
    database.write_new("test_file.txt", ItemId::id("test_folder"))?;

    let test = database.locate_absolute("test_file.txt")?;

    println!("{:?}", test);

    database.delete("test_file.txt", ForceDeletion::NoForce)?;

    let test = database.get_all(ShouldSort::Sort);

    println!("{:#?}", test);
    
    // database.overwrite_existing("test.json", file);

    // database.delete(ItemId::database_id(), ForceDeletion::Force)?;

    Ok(())
}
