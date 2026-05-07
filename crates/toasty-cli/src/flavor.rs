//! Clap-parsed mirror of [`toasty_sql::Flavor`], so `toasty-sql` doesn't need
//! a `clap` dependency. Converted via [`From`] before reaching `toasty-sql`.

use clap::ValueEnum;

/// SQL dialect for `--flavor`.
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
