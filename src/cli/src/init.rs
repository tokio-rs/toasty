use anyhow::Result;
use std::fs;

pub fn exec() -> Result<()> {
    const BLANK: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/empty.toasty"));

    fs::create_dir("db")?;
    fs::create_dir("db/migrations")?;
    fs::write("db/schema.toasty", BLANK)?;

    Ok(())
}
