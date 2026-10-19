use serde::{Deserialize, Serialize};
use toasty::db::Capability;

/// The database flavor a migration targets: selects the capability the schema
/// is lowered with and the SQL dialect it is rendered in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Flavor {
    /// SQLite
    Sqlite,

    /// PostgreSQL
    Postgresql,

    /// MySQL
    Mysql,

    /// MariaDB
    ///
    /// Distinct from [`Flavor::Mysql`]: MariaDB has its own dialect and a
    /// native `UUID` column type.
    #[value(name = "mariadb")]
    MariaDb,

    /// Turso
    Turso,
}

impl Flavor {
    /// The name passed to the schema dumper in `TOASTY_DUMP_SCHEMA`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Flavor::Sqlite => "sqlite",
            Flavor::Postgresql => "postgresql",
            Flavor::Mysql => "mysql",
            Flavor::MariaDb => "mariadb",
            Flavor::Turso => "turso",
        }
    }

    /// The capability describing the target database.
    pub fn capability(&self) -> &'static Capability {
        match self {
            Flavor::Sqlite => &Capability::SQLITE,
            Flavor::Postgresql => &Capability::POSTGRESQL,
            Flavor::Mysql => &Capability::MYSQL,
            Flavor::MariaDb => &Capability::MARIADB,
            Flavor::Turso => &Capability::TURSO,
        }
    }
}

impl std::fmt::Display for Flavor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
