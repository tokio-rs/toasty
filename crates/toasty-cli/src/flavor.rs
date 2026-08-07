use serde::{Deserialize, Serialize};
use toasty::db::Capability;

/// The database flavor a migration targets.
///
/// Selects the capability used to lower the schema and the SQL dialect used
/// to render migration statements. Passed to the schema dumper through the
/// `TOASTY_DUMP_SCHEMA` environment variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Flavor {
    /// SQLite
    Sqlite,

    /// PostgreSQL
    #[value(alias = "postgres")]
    Postgresql,

    /// MySQL
    Mysql,

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
            Flavor::Turso => "turso",
        }
    }

    /// The capability describing the target database.
    pub fn capability(&self) -> &'static Capability {
        toasty::schema_dump::capability_for_flavor(self.as_str())
            .expect("every Flavor variant maps to a capability")
    }
}

impl std::fmt::Display for Flavor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
