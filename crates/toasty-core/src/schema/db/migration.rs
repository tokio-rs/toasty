/// Database migration generate from a [`super::SchemaDiff`] by a driver.
pub enum Migration {
    Sql(String),
}

impl Migration {
    /// Create a new SQL migration from a single SQL string.
    pub fn new_sql(sql: String) -> Self {
        Migration::Sql(sql)
    }

    /// Create a new SQL migration from multiple SQL statements.
    /// Statements are joined with breakpoint markers.
    pub fn new_sql_with_breakpoints<S: AsRef<str>>(statements: &[S]) -> Self {
        let sql = statements
            .iter()
            .map(|s| s.as_ref())
            .collect::<Vec<_>>()
            .join("\n-- #[toasty::breakpoint]\n");
        Migration::Sql(sql)
    }

    /// Get individual SQL statements by splitting on breakpoint markers.
    pub fn statements(&self) -> Vec<&str> {
        match self {
            Migration::Sql(sql) => sql.split("\n-- #[toasty::breakpoint]\n").collect(),
        }
    }
}

/// Metadata about a migration that has already been applied to a database.
pub struct AppliedMigration {
    id: u64,
}

impl AppliedMigration {
    pub fn new(id: u64) -> Self {
        Self { id }
    }

    pub fn id(&self) -> u64 {
        self.id
    }
}
