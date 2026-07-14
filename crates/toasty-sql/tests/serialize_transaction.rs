//! Verifies the serializer renders transaction-control statements: `BEGIN` /
//! `START TRANSACTION` with isolation level, read-only flag, and (SQLite only)
//! lock-acquisition mode, plus commit / rollback / savepoint.
//!
//! `TransactionMode` is the dimension added by #931: it picks SQLite's
//! `BEGIN` / `BEGIN IMMEDIATE` / `BEGIN EXCLUSIVE` and is inert on PostgreSQL
//! and MySQL (whose drivers reject non-`Default` modes before the serializer).
//!
//! Transaction operations are serialized via `Serializer::serialize_transaction`
//! and reference no tables, so an empty schema is sufficient.

use expect_test::expect;
use toasty_core::{
    driver::operation::{IsolationLevel, Transaction, TransactionMode},
    schema::db::Schema,
};
use toasty_sql::Serializer;

#[derive(Clone, Copy)]
enum Flavor {
    Sqlite,
    Postgresql,
    Mysql,
}

fn render(flavor: Flavor, op: &Transaction) -> String {
    let schema = Schema::default();
    match flavor {
        Flavor::Sqlite => Serializer::sqlite(&schema).serialize_transaction(op),
        Flavor::Postgresql => Serializer::postgresql(&schema).serialize_transaction(op),
        Flavor::Mysql => Serializer::mysql(&schema).serialize_transaction(op),
    }
}

/// `Transaction::Start` with the given knobs.
fn start(isolation: Option<IsolationLevel>, read_only: bool, mode: TransactionMode) -> Transaction {
    Transaction::Start {
        isolation,
        read_only,
        mode,
    }
}

// -----------------------------------------------------------------------------
// SQLite lock-acquisition mode (the logic added by #931)
// -----------------------------------------------------------------------------

/// SQLite's natural default is DEFERRED, so `Default` and `Deferred` are
/// indistinguishable at the serializer — both emit a plain `BEGIN`.
#[test]
fn sqlite_default_and_deferred_emit_plain_begin() {
    expect!["BEGIN;"].assert_eq(&render(
        Flavor::Sqlite,
        &start(None, false, TransactionMode::Default),
    ));
    expect!["BEGIN;"].assert_eq(&render(
        Flavor::Sqlite,
        &start(None, false, TransactionMode::Deferred),
    ));
}

#[test]
fn sqlite_immediate() {
    expect!["BEGIN IMMEDIATE;"].assert_eq(&render(
        Flavor::Sqlite,
        &start(None, false, TransactionMode::Immediate),
    ));
}

#[test]
fn sqlite_exclusive() {
    expect!["BEGIN EXCLUSIVE;"].assert_eq(&render(
        Flavor::Sqlite,
        &start(None, false, TransactionMode::Exclusive),
    ));
}

/// SQLite has no per-transaction isolation level or read-only keyword; only the
/// lock-acquisition mode reaches the SQL, so isolation and `read_only` are
/// dropped regardless of mode.
#[test]
fn sqlite_ignores_isolation_and_read_only() {
    expect!["BEGIN;"].assert_eq(&render(
        Flavor::Sqlite,
        &start(
            Some(IsolationLevel::Serializable),
            true,
            TransactionMode::Default,
        ),
    ));
    expect!["BEGIN EXCLUSIVE;"].assert_eq(&render(
        Flavor::Sqlite,
        &start(
            Some(IsolationLevel::Serializable),
            true,
            TransactionMode::Exclusive,
        ),
    ));
}

// -----------------------------------------------------------------------------
// PostgreSQL: isolation + read-only, lock mode inert
// -----------------------------------------------------------------------------

#[test]
fn postgresql_default() {
    expect!["BEGIN;"].assert_eq(&render(
        Flavor::Postgresql,
        &start(None, false, TransactionMode::Default),
    ));
}

#[test]
fn postgresql_isolation_levels() {
    let cases = [
        (
            IsolationLevel::ReadUncommitted,
            "BEGIN ISOLATION LEVEL READ UNCOMMITTED;",
        ),
        (
            IsolationLevel::ReadCommitted,
            "BEGIN ISOLATION LEVEL READ COMMITTED;",
        ),
        (
            IsolationLevel::RepeatableRead,
            "BEGIN ISOLATION LEVEL REPEATABLE READ;",
        ),
        (
            IsolationLevel::Serializable,
            "BEGIN ISOLATION LEVEL SERIALIZABLE;",
        ),
    ];
    for (level, sql) in cases {
        assert_eq!(
            render(
                Flavor::Postgresql,
                &start(Some(level), false, TransactionMode::Default)
            ),
            sql,
        );
    }
}

#[test]
fn postgresql_read_only_and_isolation_combine() {
    expect!["BEGIN ISOLATION LEVEL SERIALIZABLE READ ONLY;"].assert_eq(&render(
        Flavor::Postgresql,
        &start(
            Some(IsolationLevel::Serializable),
            true,
            TransactionMode::Default,
        ),
    ));
}

/// PostgreSQL has no SQLite-style lock keyword; its driver rejects non-`Default`
/// modes before serialization, so the serializer renders them identically to
/// `Default` rather than degrading silently.
#[test]
fn postgresql_ignores_lock_mode() {
    expect!["BEGIN;"].assert_eq(&render(
        Flavor::Postgresql,
        &start(None, false, TransactionMode::Immediate),
    ));
    expect!["BEGIN;"].assert_eq(&render(
        Flavor::Postgresql,
        &start(None, false, TransactionMode::Exclusive),
    ));
}

// -----------------------------------------------------------------------------
// MySQL: `START TRANSACTION`, isolation as a separate statement, lock mode inert
// -----------------------------------------------------------------------------

#[test]
fn mysql_default() {
    expect!["START TRANSACTION;"].assert_eq(&render(
        Flavor::Mysql,
        &start(None, false, TransactionMode::Default),
    ));
}

/// MySQL sets the isolation level with a separate `SET TRANSACTION` statement
/// preceding `START TRANSACTION`, and appends `READ ONLY` to the start.
#[test]
fn mysql_isolation_and_read_only() {
    expect!["SET TRANSACTION ISOLATION LEVEL SERIALIZABLE; START TRANSACTION READ ONLY;"]
        .assert_eq(&render(
            Flavor::Mysql,
            &start(
                Some(IsolationLevel::Serializable),
                true,
                TransactionMode::Default,
            ),
        ));
}

#[test]
fn mysql_ignores_lock_mode() {
    expect!["START TRANSACTION;"].assert_eq(&render(
        Flavor::Mysql,
        &start(None, false, TransactionMode::Immediate),
    ));
}

// -----------------------------------------------------------------------------
// Lifecycle: commit / rollback / savepoints
// -----------------------------------------------------------------------------

#[test]
fn commit_and_rollback() {
    for flavor in [Flavor::Sqlite, Flavor::Postgresql, Flavor::Mysql] {
        assert_eq!(render(flavor, &Transaction::Commit), "COMMIT;");
        assert_eq!(render(flavor, &Transaction::Rollback), "ROLLBACK;");
    }
}

/// Savepoint identifiers are quoted with the flavor's identifier delimiter —
/// double quotes for SQLite/PostgreSQL, backticks for MySQL.
#[test]
fn savepoints_quote_identifier() {
    expect![[r#"SAVEPOINT "sp_1";"#]].assert_eq(&render(
        Flavor::Sqlite,
        &Transaction::Savepoint("sp_1".into()),
    ));
    expect![[r#"RELEASE SAVEPOINT "sp_1";"#]].assert_eq(&render(
        Flavor::Postgresql,
        &Transaction::ReleaseSavepoint("sp_1".into()),
    ));
    expect![[r#"ROLLBACK TO SAVEPOINT "sp_1";"#]].assert_eq(&render(
        Flavor::Sqlite,
        &Transaction::RollbackToSavepoint("sp_1".into()),
    ));
    expect!["SAVEPOINT `sp_1`;"].assert_eq(&render(
        Flavor::Mysql,
        &Transaction::Savepoint("sp_1".into()),
    ));
}
