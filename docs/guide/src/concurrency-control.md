# Concurrency Control

When multiple writers modify the same record, you need a way to detect
conflicting writes. Toasty supports optimistic concurrency control (OCC)
through the `#[version]` attribute: Toasty conditions each write on a version
field and atomically increments it, so a stale writer sees its update fail
rather than silently overwrite a newer value.

## Optimistic concurrency with `#[version]`

Add `#[version]` to a `u64` field to enable OCC on a model. Toasty manages the
field — you declare it but never set it manually.

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct Document {
    #[key]
    #[auto]
    id: uuid::Uuid,

    content: String,

    #[version]
    version: u64,
}
```

**Create.** Toasty sets the version to `1` on the new record.

**Instance update.** `doc.update()...exec()` conditions the write on the
current version and atomically increments it. If another writer has updated
the record since you last loaded it, the update returns an error.

**Instance delete.** `doc.delete().exec()` conditions the delete on the
current version. If the record has been updated since you last loaded it, the
delete returns an error.

**Query-based update.** `Document::filter_by_id(id).update()...exec()`
atomically increments the version on every matched row (`version = version +
1`), but does not condition the write on a prior version — a query-based update
is atomic at the database level and may span many rows. Advancing the counter
is enough to make a concurrent instance update or delete from a stale snapshot
fail its version check instead of overwriting the query update.

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key]
#     #[auto]
#     id: uuid::Uuid,
#     content: String,
#     #[version]
#     version: u64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let mut doc = toasty::create!(Document { content: "hello" })
    .exec(&mut db)
    .await?;

assert_eq!(doc.version, 1);

// Load a second handle — both start at version 1
let mut stale = Document::get_by_id(&mut db, &doc.id).await?;

// Advance doc to version 2
doc.update().content("world").exec(&mut db).await?;
assert_eq!(doc.version, 2);

// stale is still at version 1 — the update fails with a conflict error
let result = stale.update().content("conflict").exec(&mut db).await;
assert!(result.is_err());
# Ok(())
# }
```

Only instance updates and instance deletes *check* the version and fail on
conflict. Query-based updates increment the version but apply unconditionally —
they never fail on a version mismatch.

## Driver support

`#[version]` works on every driver: DynamoDB, SQLite, PostgreSQL, and MySQL. How
Toasty applies the version check varies by backend, but the behavior is the
same everywhere:

- DynamoDB conditions the write on the version value in a single request.
- PostgreSQL bundles the check and the update into one statement.
- SQLite and MySQL run the check and the write inside a transaction, reading the
  current version (locking the row where the database supports it) before
  applying the write.

A conflicting write returns `Error::condition_failed`; recover by reloading the
record and retrying.
