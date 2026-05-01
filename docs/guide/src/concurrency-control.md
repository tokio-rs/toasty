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

Query-based updates (`Document::filter_by_id(id).update()...`) neither check
nor increment the version. OCC applies only to instance updates and instance
deletes.

> **Note:** `#[version]` is supported by the DynamoDB driver only. SQL drivers
> do not yet implement OCC.
