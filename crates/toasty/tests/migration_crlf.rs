//! Applying a migration whose breakpoint markers use CRLF line endings must
//! split into individual statements. Regression test for
//! <https://github.com/tokio-rs/toasty/issues/1303>.

use toasty::codegen_support::core::{driver::Connection as _, schema::db::Migration};
use toasty_driver_sqlite::Connection;

const CRLF_MIGRATION: &str = "CREATE TABLE \"users\" (\r\n    \"id\" TEXT NOT NULL,\r\n    \"email\" TEXT NOT NULL,\r\n    PRIMARY KEY (\"id\")\r\n);\r\n-- #[toasty::breakpoint]\r\nCREATE UNIQUE INDEX \"index_users_by_email\" ON \"users\" (\"email\");\r\n";

#[tokio::test]
async fn sqlite_applies_crlf_migration_with_breakpoints() {
    let mut conn = Connection::in_memory();
    let migration = Migration::new_sql(CRLF_MIGRATION.to_string());

    conn.apply_migration(1, "0000_users.sql", &migration)
        .await
        .expect("CRLF migration applies");

    let applied = conn.applied_migrations().await.unwrap();
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].id(), 1);
}
