# Final Gap: Cassandra Driver

This guide documents feature item 31:

31. Cassandra driver support (not implemented)

## 31) Cassandra Driver Support (Not Implemented)

Toasty currently ships drivers for:

- SQLite
- PostgreSQL
- MySQL
- DynamoDB

There is no built-in Cassandra (or ScyllaDB) driver crate in the current
workspace.

Practical impact:

- Toasty model/query APIs cannot target Cassandra directly today.
- Migration and schema-management workflows in Toasty do not cover Cassandra.

Current workaround:

- Use Toasty for supported backends.
- For Cassandra-backed workloads, use a dedicated Cassandra client in a
  separate data-access layer for those code paths.

Example pattern:

```rust
// Keep Toasty for supported stores.
let sql_db = toasty::Db::builder()
    .register::<User>()
    .connect("sqlite::memory:")
    .await?;

// Use Cassandra client separately for Cassandra-specific operations.
// (Pseudo-code: depends on the chosen Cassandra crate.)
// let cassandra = scylla::SessionBuilder::new().known_node("127.0.0.1:9042").build().await?;
// cassandra.query_unpaged("SELECT id, name FROM users WHERE id = ?", (id,)).await?;
```

This is the last outstanding item in the current feature-status list.
