//! CLI-facing SQL dialect flag, parsed by clap. Converts into the canonical
//! [`toasty_sql::Flavor`] used by the rest of the migration pipeline.

use clap::ValueEnum;

/// SQL dialect for `migration generate`.
///
/// Thin wrapper around [`toasty_sql::Flavor`] that adds the clap-derived
/// parsing for `--flavor` without forcing a clap dependency on `toasty-sql`.
#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum Flavor {
    /// SQLite
    Sqlite,
    /// PostgreSQL
    Postgresql,
    /// MySQL
    Mysql,
}

impl From<Flavor> for toasty_sql::Flavor {
    fn from(flavor: Flavor) -> Self {
        match flavor {
            Flavor::Sqlite => toasty_sql::Flavor::Sqlite,
            Flavor::Postgresql => toasty_sql::Flavor::Postgresql,
            Flavor::Mysql => toasty_sql::Flavor::Mysql,
        }
    }
}
