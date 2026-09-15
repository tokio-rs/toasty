/// The SQL dialect a driver speaks.
///
/// Everything Toasty renders as SQL is a function of the dialect, not of the
/// live connection, so the dialect is named on
/// [`Capability::sql`](super::Capability::sql) rather than discovered from a
/// driver at runtime.
///
/// Dialect-compatible engines share a variant: Turso reports
/// [`Sqlite`](Self::Sqlite) because it accepts SQLite's SQL.
///
/// # Examples
///
/// ```
/// use toasty_core::driver::{Capability, Dialect};
///
/// assert_eq!(Capability::SQLITE.sql, Some(Dialect::Sqlite));
/// assert_eq!(Capability::DYNAMODB.sql, None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    /// SQLite, and engines that accept SQLite's SQL.
    Sqlite,

    /// PostgreSQL.
    Postgresql,

    /// MySQL.
    Mysql,

    /// MariaDB.
    ///
    /// Separate from [`Mysql`](Self::Mysql) despite the shared wire protocol:
    /// MySQL 8 added syntax MariaDB never picked up, so the same statement
    /// renders differently. The table value constructor is `VALUES ROW(...)`
    /// on MySQL and `VALUES (...)` on MariaDB, and only MariaDB has a native
    /// `UUID` type.
    MariaDb,
}
