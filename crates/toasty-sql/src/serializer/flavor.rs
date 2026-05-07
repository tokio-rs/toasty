use super::Serializer;

use toasty_core::driver::Capability;
use toasty_core::schema::db::{self, Migration, SchemaDiff};

/// SQL dialect for migration generation and serialization.
#[derive(Copy, Clone, Debug)]
pub enum Flavor {
    /// SQLite
    Sqlite,
    /// PostgreSQL
    Postgresql,
    /// MySQL
    Mysql,
}

impl Flavor {
    /// The driver capability associated with this flavor.
    pub fn capability(&self) -> &'static Capability {
        match self {
            Flavor::Sqlite => &Capability::SQLITE,
            Flavor::Postgresql => &Capability::POSTGRESQL,
            Flavor::Mysql => &Capability::MYSQL,
        }
    }

    /// Serialize a schema diff into a dialect-specific migration.
    pub fn generate_migration(&self, diff: &SchemaDiff<'_>) -> Migration {
        let statements = crate::MigrationStatement::from_diff(diff, self.capability());
        let sql_strings: Vec<String> = statements
            .iter()
            .map(|stmt| Serializer::for_flavor(*self, stmt.schema()).serialize(stmt.statement()))
            .collect();

        // Postgres emits a single joined blob; Sqlite/MySQL use breakpoints.
        match self {
            Flavor::Sqlite | Flavor::Mysql => Migration::new_sql_with_breakpoints(&sql_strings),
            Flavor::Postgresql => Migration::new_sql(sql_strings.join("\n")),
        }
    }
}

impl<'a> Serializer<'a> {
    /// Creates a serializer for the given dialect.
    pub fn for_flavor(flavor: Flavor, schema: &'a db::Schema) -> Self {
        Serializer { schema, flavor }
    }

    /// Creates a serializer that emits SQLite SQL.
    pub fn sqlite(schema: &'a db::Schema) -> Self {
        Self::for_flavor(Flavor::Sqlite, schema)
    }

    /// Returns `true` if this serializer targets SQLite.
    pub fn is_sqlite(&self) -> bool {
        matches!(self.flavor, Flavor::Sqlite)
    }

    /// Creates a serializer that emits PostgreSQL SQL.
    pub fn postgresql(schema: &'a db::Schema) -> Self {
        Self::for_flavor(Flavor::Postgresql, schema)
    }

    /// Creates a serializer that emits MySQL SQL.
    pub fn mysql(schema: &'a db::Schema) -> Self {
        Self::for_flavor(Flavor::Mysql, schema)
    }

    pub(super) fn is_mysql(&self) -> bool {
        matches!(self.flavor, Flavor::Mysql)
    }
}
