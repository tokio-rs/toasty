/// A database migration generated from a [`diff::Schema`](super::super::diff::Schema) by a driver.
///
/// Currently only SQL migrations are supported. Multiple SQL statements
/// within a single migration are separated by breakpoint markers
/// (`-- #[toasty::breakpoint]`).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::Migration;
///
/// let m = Migration::new_sql("CREATE TABLE users (id INTEGER PRIMARY KEY)".to_string());
/// assert_eq!(m.statements(), vec!["CREATE TABLE users (id INTEGER PRIMARY KEY)"]);
/// ```
#[derive(Debug)]
pub enum Migration {
    /// A SQL migration containing one or more statements.
    Sql(String),
}

impl Migration {
    /// Creates a SQL migration from a single SQL string.
    pub fn new_sql(sql: String) -> Self {
        Migration::Sql(sql)
    }

    /// Creates a SQL migration from multiple SQL statements.
    /// Statements are joined with `-- #[toasty::breakpoint]` markers.
    pub fn new_sql_with_breakpoints<S: AsRef<str>>(statements: &[S]) -> Self {
        let sql = statements
            .iter()
            .map(|s| s.as_ref())
            .collect::<Vec<_>>()
            .join(&format!("\n{BREAKPOINT}\n"));
        Migration::Sql(sql)
    }

    /// Returns individual SQL statements by splitting on breakpoint markers.
    pub fn statements(&self) -> Vec<&str> {
        match self {
            Migration::Sql(sql) => split_on_breakpoints(sql),
        }
    }
}

/// The comment that separates statements within a SQL migration.
const BREAKPOINT: &str = "-- #[toasty::breakpoint]";

/// Splits `sql` on lines that consist of a breakpoint marker.
///
/// Neither the line ending before a marker nor the marker's own line ending
/// belongs to a statement, whether it is `\n` or `\r\n`. Pieces that hold
/// only whitespace, such as the one after a trailing marker, are dropped.
fn split_on_breakpoints(sql: &str) -> Vec<&str> {
    let mut statements = Vec::new();
    let mut statement_start = 0;
    let mut line_start = 0;

    for line in sql.split_inclusive('\n') {
        let line_end = line_start + line.len();

        if line.trim() == BREAKPOINT {
            statements.push(strip_line_ending(&sql[statement_start..line_start]));
            statement_start = line_end;
        }

        line_start = line_end;
    }

    statements.push(&sql[statement_start..]);
    statements.retain(|s| !s.trim().is_empty());
    statements
}

fn strip_line_ending(s: &str) -> &str {
    s.strip_suffix("\r\n")
        .or_else(|| s.strip_suffix('\n'))
        .unwrap_or(s)
}

/// Metadata about a migration that has already been applied to a database.
///
/// Stores the unique migration ID assigned by the migration system.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::AppliedMigration;
///
/// let applied = AppliedMigration::new(42);
/// assert_eq!(applied.id(), 42);
/// ```
pub struct AppliedMigration {
    id: u64,
}

impl AppliedMigration {
    /// Creates a new `AppliedMigration` with the given ID.
    pub fn new(id: u64) -> Self {
        Self { id }
    }

    /// Returns the migration's unique ID.
    pub fn id(&self) -> u64 {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CREATE_TABLE: &str = "CREATE TABLE users (id INTEGER PRIMARY KEY);";
    const CREATE_INDEX: &str = "CREATE INDEX idx ON users (id);";

    #[test]
    fn generated_breakpoints_round_trip() {
        let m = Migration::new_sql_with_breakpoints(&[CREATE_TABLE, CREATE_INDEX]);
        let Migration::Sql(sql) = &m;
        assert_eq!(
            sql,
            "CREATE TABLE users (id INTEGER PRIMARY KEY);\n-- #[toasty::breakpoint]\nCREATE INDEX idx ON users (id);"
        );
        assert_eq!(m.statements(), vec![CREATE_TABLE, CREATE_INDEX]);
    }

    #[test]
    fn splits_on_lf_and_crlf_breakpoints() {
        // A multi-line CRLF statement, an LF breakpoint, a CRLF breakpoint,
        // and a trailing newline on the final statement.
        let sql = "CREATE TABLE users (\r\n    id INTEGER PRIMARY KEY\r\n);\r\n\
                   -- #[toasty::breakpoint]\n\
                   CREATE INDEX idx ON users (id);\n\
                   -- #[toasty::breakpoint]\r\n\
                   SELECT 1;\r\n";
        let m = Migration::new_sql(sql.to_string());
        assert_eq!(
            m.statements(),
            vec![
                "CREATE TABLE users (\r\n    id INTEGER PRIMARY KEY\r\n);",
                CREATE_INDEX,
                "SELECT 1;\r\n",
            ]
        );
    }

    #[test]
    fn only_whole_marker_lines_split() {
        // Whitespace around the marker is ignored, but a marker inside a
        // string literal or after a statement on the same line is not a split.
        let sql = format!(
            "SELECT '-- #[toasty::breakpoint]';\n  -- #[toasty::breakpoint]  \n{CREATE_INDEX} -- #[toasty::breakpoint]"
        );
        let m = Migration::new_sql(sql);
        assert_eq!(
            m.statements(),
            vec![
                "SELECT '-- #[toasty::breakpoint]';",
                "CREATE INDEX idx ON users (id); -- #[toasty::breakpoint]",
            ]
        );
    }

    #[test]
    fn drops_empty_statements() {
        // Markers at the start, doubled, and at the end leave no empty pieces.
        let sql = format!(
            "-- #[toasty::breakpoint]\n{CREATE_TABLE}\n-- #[toasty::breakpoint]\n\n-- #[toasty::breakpoint]\n{CREATE_INDEX}\n-- #[toasty::breakpoint]\n"
        );
        let m = Migration::new_sql(sql);
        assert_eq!(m.statements(), vec![CREATE_TABLE, CREATE_INDEX]);
        assert!(Migration::new_sql(String::new()).statements().is_empty());
    }
}
