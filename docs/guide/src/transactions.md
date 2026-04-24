# Transactions

A transaction groups multiple database operations so they either all succeed or
all fail. Toasty supports interactive transactions on SQL databases (SQLite,
PostgreSQL, MySQL).

> **Tip:** If you just need multiple operations to execute atomically, consider
> using [batch operations](./batch-operations.md) first. Batch operations are
> atomic and more efficient — they can be sent as a single statement, avoiding
> the extra round-trips that interactive transactions require (begin, execute,
> commit). Use interactive transactions when you need to read data and branch on
> the results within the same atomic scope.

## Starting a transaction

Call `db.transaction()` to begin a transaction:

```rust
# use toasty::{Model, Executor};
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let mut tx = db.transaction().await?;

toasty::create!(User { name: "Alice" }).exec(&mut tx).await?;
toasty::create!(User { name: "Bob" }).exec(&mut tx).await?;

tx.commit().await?;
# Ok(())
# }
```

The transaction borrows `&mut Db`, preventing other operations on the same `Db`
handle while the transaction is open. Pass `&mut tx` to query builders the same
way you pass `&mut db`.

The exclusive borrow is deliberate. Without it, it would be easy to run a
statement against `db` while holding `tx` — that statement would execute on a
separate connection pulled from the pool, bypassing the transaction entirely.
The `&mut` keeps `db` unusable until the transaction ends, so every statement
has to go through `&mut tx`.

If you genuinely need a second handle while a transaction is open — for
example, an independent task doing unrelated work — clone the `Db` before
starting the transaction:

```rust,ignore
let mut db2 = db.clone();
let mut tx = db.transaction().await?;

// `db2` is a separate handle backed by the same pool; use it freely
// for work that is not part of `tx`.
```

Clones share the underlying pool, so cloning is cheap and does not open a new
connection.

The same rule applies to transactions started from a `Connection` and to
nested transactions created from an existing `Transaction`: each takes
`&mut self` so statements cannot accidentally bypass the innermost scope.

## Running queries in a transaction

All the same operations work inside a transaction — creates, queries, updates,
and deletes:

```rust,ignore
let mut tx = db.transaction().await?;

// Create
let user = toasty::create!(User { name: "Alice" }).exec(&mut tx).await?;

// Query
let users = User::all().exec(&mut tx).await?;

// Update
user.update().name("Bob").exec(&mut tx).await?;

// Delete
User::filter_by_id(user.id).delete().exec(&mut tx).await?;

tx.commit().await?;
```

Reads inside a transaction see the writes made earlier in the same transaction,
even before commit:

```rust,ignore
let mut tx = db.transaction().await?;

toasty::create!(User { name: "Alice" }).exec(&mut tx).await?;

// This sees the record we just created
let users = User::all().exec(&mut tx).await?;
assert_eq!(users.len(), 1);

tx.commit().await?;
```

## Commit and rollback

Call `.commit()` to save all changes made in the transaction:

```rust,ignore
let mut tx = db.transaction().await?;
toasty::create!(User { name: "Alice" }).exec(&mut tx).await?;
tx.commit().await?;

// The record is now visible outside the transaction
let users = User::all().exec(&mut db).await?;
assert_eq!(users.len(), 1);
```

Call `.rollback()` to discard all changes:

```rust,ignore
let mut tx = db.transaction().await?;
toasty::create!(User { name: "Alice" }).exec(&mut tx).await?;
tx.rollback().await?;

// The record was never persisted
let users = User::all().exec(&mut db).await?;
assert!(users.is_empty());
```

## Auto-rollback on drop

If a transaction is dropped without calling `.commit()` or `.rollback()`, it
automatically rolls back. This means you don't need explicit rollback when an
error occurs — just let the transaction go out of scope:

```rust,ignore
let mut tx = db.transaction().await?;
toasty::create!(User { name: "Alice" }).exec(&mut tx).await?;
// tx is dropped here without commit — changes are rolled back
```

This is useful in functions that return `Result`. If an operation inside the
transaction fails with `?`, the transaction is dropped and rolled back:

```rust,ignore
async fn transfer(db: &mut Db) -> toasty::Result<()> {
    let mut tx = db.transaction().await?;

    // If this fails, tx is dropped and rolled back
    let user = User::get_by_id(&mut tx, &1).await?;
    user.update().balance(user.balance - 100).exec(&mut tx).await?;

    let other = User::get_by_id(&mut tx, &2).await?;
    other.update().balance(other.balance + 100).exec(&mut tx).await?;

    tx.commit().await?;
    Ok(())
}
```

## Nested transactions

Call `.transaction()` on an existing transaction to create a nested transaction.
Nested transactions use database savepoints:

```rust,ignore
let mut tx = db.transaction().await?;
toasty::create!(User { name: "Alice" }).exec(&mut tx).await?;

{
    let mut nested = tx.transaction().await?;
    toasty::create!(User { name: "Bob" }).exec(&mut nested).await?;
    nested.commit().await?; // releases the savepoint
}

tx.commit().await?; // commits both Alice and Bob
```

Rolling back a nested transaction only undoes the work done inside it. The outer
transaction continues:

```rust,ignore
let mut tx = db.transaction().await?;
toasty::create!(User { name: "Alice" }).exec(&mut tx).await?;

{
    let mut nested = tx.transaction().await?;
    toasty::create!(User { name: "Bob" }).exec(&mut nested).await?;
    nested.rollback().await?; // rolls back to savepoint — Bob is discarded
}

tx.commit().await?; // only Alice is committed
```

Nested transactions also auto-rollback on drop, just like top-level
transactions.

## Transaction options

Use `transaction_builder()` to configure a transaction before starting it:

```rust,ignore
use toasty::IsolationLevel;

let mut tx = db.transaction_builder()
    .isolation(IsolationLevel::Serializable)
    .read_only(true)
    .begin()
    .await?;
```

### Isolation levels

Toasty supports four isolation levels:

| Level | Description |
|---|---|
| `ReadUncommitted` | Allows dirty reads |
| `ReadCommitted` | Only reads committed data |
| `RepeatableRead` | Consistent reads within the transaction |
| `Serializable` | Full isolation between transactions |

Driver support varies. SQLite only supports `Serializable`. PostgreSQL and MySQL
support all four levels.

### Read-only transactions

Set `.read_only(true)` to create a read-only transaction. The database rejects
write operations inside a read-only transaction.
