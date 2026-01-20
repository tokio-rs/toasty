use crate::Config;
use super::SnapshotFile;
use anyhow::Result;
use clap::Parser;
use toasty::Db;

#[derive(Parser, Debug)]
pub struct SnapshotCommand {
    // Future options can be added here
}

impl SnapshotCommand {
    pub(crate) fn run(self, db: &Db, _config: &Config) -> Result<()> {
        let snapshot_file = SnapshotFile::new(toasty::schema::db::Schema::clone(&db.schema().db));
        println!("{}", snapshot_file);
        Ok(())
    }
}
