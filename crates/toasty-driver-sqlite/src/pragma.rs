//! Per-connection `PRAGMA` configuration.

use std::{borrow::Cow, fmt};

use indexmap::IndexMap;
use rusqlite::{Connection as RusqliteConnection, ffi};
use toasty_core::{
    Result,
    driver::{ExecResponse, QueryLogConfig, log::QueryLog},
    stmt,
};

/// The [journal mode](https://www.sqlite.org/pragma.html#pragma_journal_mode)
/// set by [`Sqlite::journal_mode`](crate::Sqlite::journal_mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JournalMode {
    /// Delete the rollback journal at the end of each transaction.
    Delete,
    /// Truncate the rollback journal to zero length instead of deleting it.
    Truncate,
    /// Overwrite the rollback journal header instead of deleting the file.
    Persist,
    /// Keep the rollback journal in memory.
    Memory,
    /// Use a [write-ahead log](https://www.sqlite.org/wal.html).
    Wal,
    /// Disable the rollback journal.
    Off,
}

impl JournalMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Delete => "DELETE",
            Self::Truncate => "TRUNCATE",
            Self::Persist => "PERSIST",
            Self::Memory => "MEMORY",
            Self::Wal => "WAL",
            Self::Off => "OFF",
        }
    }
}

/// The [synchronous](https://www.sqlite.org/pragma.html#pragma_synchronous)
/// setting set by [`Sqlite::synchronous`](crate::Sqlite::synchronous).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Synchronous {
    /// Hand data to the operating system without syncing.
    Off,
    /// Sync at the most critical moments. Durable in WAL mode except across
    /// power loss.
    Normal,
    /// Sync before each transaction commits.
    Full,
    /// Like `Full`, and also sync the directory after deleting a rollback
    /// journal.
    Extra,
}

impl Synchronous {
    fn as_str(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Normal => "NORMAL",
            Self::Full => "FULL",
            Self::Extra => "EXTRA",
        }
    }
}

/// The [locking mode](https://www.sqlite.org/pragma.html#pragma_locking_mode)
/// set by [`Sqlite::locking_mode`](crate::Sqlite::locking_mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockingMode {
    /// Release file locks at the end of each transaction.
    Normal,
    /// Hold file locks until the connection closes.
    Exclusive,
}

impl LockingMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Exclusive => "EXCLUSIVE",
        }
    }
}

/// The [auto-vacuum](https://www.sqlite.org/pragma.html#pragma_auto_vacuum)
/// setting set by [`Sqlite::auto_vacuum`](crate::Sqlite::auto_vacuum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutoVacuum {
    /// Never shrink the database file automatically.
    None,
    /// Shrink the database file at every commit.
    Full,
    /// Shrink the database file only on `PRAGMA incremental_vacuum`.
    Incremental,
}

impl AutoVacuum {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Full => "FULL",
            Self::Incremental => "INCREMENTAL",
        }
    }
}

impl From<JournalMode> for Cow<'static, str> {
    fn from(mode: JournalMode) -> Self {
        Cow::Borrowed(mode.as_str())
    }
}

impl From<Synchronous> for Cow<'static, str> {
    fn from(synchronous: Synchronous) -> Self {
        Cow::Borrowed(synchronous.as_str())
    }
}

impl From<LockingMode> for Cow<'static, str> {
    fn from(mode: LockingMode) -> Self {
        Cow::Borrowed(mode.as_str())
    }
}

impl From<AutoVacuum> for Cow<'static, str> {
    fn from(auto_vacuum: AutoVacuum) -> Self {
        Cow::Borrowed(auto_vacuum.as_str())
    }
}

/// Pragmas whose value must not reach the query log, error text, or
/// `Debug` output.
const SECRET: &[&str] = &["key", "rekey", "hexkey", "hexrekey", "cipher_salt"];

/// The pragmas a driver applies to each connection, in application order.
///
/// The map starts with an empty slot for each pragma whose position
/// matters. `IndexMap::insert` keeps an existing key's position, so setting
/// one fills its slot rather than appending. Pragmas without a slot are
/// appended and run after every slotted one, in the order first set.
#[derive(Clone)]
pub(crate) struct Pragmas {
    entries: IndexMap<Cow<'static, str>, Option<Cow<'static, str>>>,
}

impl Default for Pragmas {
    fn default() -> Self {
        let slots = [
            // SQLCipher requires `key` before any other statement, and the
            // remaining cipher settings before any read of the database.
            "key",
            "cipher_plaintext_header_size",
            "cipher_salt",
            "kdf_iter",
            "cipher_kdf_algorithm",
            "cipher_use_hmac",
            "cipher_compatibility",
            "cipher_page_size",
            "cipher_hmac_algorithm",
            // Must precede any write to the database file.
            "page_size",
            // `locking_mode` before `journal_mode`: an exclusive lock taken
            // before WAL is first used lets SQLite run WAL without a
            // shared-memory file
            // (https://www.sqlite.org/wal.html#use_of_wal_without_shared_memory).
            // `auto_vacuum` before `journal_mode` too: changing
            // `journal_mode` first marks the database dirty and the
            // `auto_vacuum` change is then dropped.
            "locking_mode",
            "auto_vacuum",
            "journal_mode",
            "foreign_keys",
            "synchronous",
        ];

        let mut pragmas = Self {
            entries: slots
                .into_iter()
                .map(|name| (Cow::Borrowed(name), None))
                .collect(),
        };

        // Bundled SQLite already defaults this ON; set it explicitly so it
        // doesn't depend on that build flag.
        pragmas.set("foreign_keys", "ON");
        pragmas
    }
}

impl Pragmas {
    /// Sets `name` to `value`, filling its slot or replacing an earlier
    /// value. Names match exactly.
    pub(crate) fn set(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        value: impl Into<Cow<'static, str>>,
    ) {
        self.entries.insert(name.into(), Some(value.into()));
    }

    fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries
            .iter()
            .filter_map(|(name, value)| Some((name.as_ref(), value.as_deref()?)))
    }

    /// Applies the pragmas to a freshly opened connection.
    pub(crate) fn apply(
        &self,
        conn: &RusqliteConnection,
        query_log: &QueryLogConfig,
    ) -> Result<()> {
        for (name, value) in self.iter() {
            exec(conn, name, value, query_log)?;
        }

        Ok(())
    }
}

/// Lists the pragmas that are set, withholding secret values.
impl fmt::Debug for Pragmas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.iter().map(|(name, value)| {
                let value = if SECRET.contains(&name) {
                    "[redacted]"
                } else {
                    value
                };
                (name, value)
            }))
            .finish()
    }
}

fn exec(
    conn: &RusqliteConnection,
    name: &str,
    value: &str,
    query_log: &QueryLogConfig,
) -> Result<()> {
    let secret = SECRET.contains(&name);
    let sql = format!("PRAGMA {name} = {value}");

    let logged = if secret {
        Cow::Owned(format!("PRAGMA {name} = [redacted]"))
    } else {
        Cow::Borrowed(sql.as_str())
    };
    let log = QueryLog::sql(
        query_log,
        "sqlite",
        &logged,
        std::iter::empty::<&stmt::Value>(),
    );

    // `execute_batch` steps past any rows the pragma returns, such as the
    // new mode `journal_mode` reports.
    let result = conn
        .execute_batch(&sql)
        .map_err(|err| {
            // SQLite's error message can quote the offending statement, which
            // contains the secret value.
            let err = if secret { redact(err) } else { err };
            toasty_core::Error::driver_operation_failed(err)
        })
        .map(|()| ExecResponse::count(0));
    log.finish(&result);

    result.map(|_| ())
}

/// Keeps a `rusqlite` error's SQLite result code and drops its message.
fn redact(err: rusqlite::Error) -> rusqlite::Error {
    let code = err
        .sqlite_error()
        .copied()
        .unwrap_or_else(|| ffi::Error::new(ffi::SQLITE_ERROR));
    rusqlite::Error::SqliteFailure(code, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(pragmas: &Pragmas) -> Vec<&str> {
        pragmas.iter().map(|(name, _)| name).collect()
    }

    #[test]
    fn only_foreign_keys_is_set_by_default() {
        let pragmas = Pragmas::default();
        assert_eq!(pragmas.iter().collect::<Vec<_>>(), [("foreign_keys", "ON")]);
    }

    #[test]
    fn slotted_pragmas_apply_in_slot_order() {
        let mut pragmas = Pragmas::default();
        pragmas.set("journal_mode", "WAL");
        pragmas.set("auto_vacuum", "FULL");
        pragmas.set("page_size", "8192");
        pragmas.set("locking_mode", "NORMAL");
        pragmas.set("key", "'secret'");

        assert_eq!(
            order(&pragmas),
            [
                "key",
                "page_size",
                "locking_mode",
                "auto_vacuum",
                "journal_mode",
                "foreign_keys"
            ]
        );
    }

    #[test]
    fn unslotted_pragmas_follow_in_the_order_first_set() {
        let mut pragmas = Pragmas::default();
        pragmas.set("mmap_size", "0");
        pragmas.set("journal_mode", "WAL");
        pragmas.set("cache_size", "-64000");
        pragmas.set("mmap_size", "1");

        assert_eq!(
            order(&pragmas),
            ["journal_mode", "foreign_keys", "mmap_size", "cache_size"]
        );
    }

    #[test]
    fn names_match_exactly() {
        let mut pragmas = Pragmas::default();
        pragmas.set("JOURNAL_MODE", "WAL");

        assert_eq!(order(&pragmas), ["foreign_keys", "JOURNAL_MODE"]);
    }
}
