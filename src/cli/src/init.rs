use crate::utils::{create_example, generate_main_file_for_example};
use anyhow::Result;
use std::fs;

pub fn exec() -> Result<()> {
    const BLANK: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/empty.toasty"));

    fs::create_dir("db")?;
    fs::create_dir("db/migrations")?;
    fs::write("schema.toasty", BLANK)?;

    Ok(())
}

pub fn exec_example(name: Option<String>) -> Result<()> {
    let target = create_example(name)?;
    const BLANK: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/empty.toasty"));

    fs::create_dir_all(&target)?;
    let db_dir = target.join("db");

    if db_dir.exists() {
        return Err(anyhow::anyhow!("Example already exists"));
    } else {
        fs::create_dir(&db_dir)?;
        fs::create_dir(&db_dir.join("migrations"))?;
        fs::write(&target.join("schema.toasty"), BLANK)?;
        generate_main_file_for_example(&target)?;
    }

    Ok(())
}
