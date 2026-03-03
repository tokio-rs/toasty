# Transactions

Toasty provides interactive transactions that let you group multiple database operations into an atomic unit. Either all operations commit together, or they all roll back, leaving the database unchanged.

Transactions work with all SQL backends (SQLite, PostgreSQL, MySQL). They are not currently supported for DynamoDB.

## Basic Usage

Start a transaction from a `Db` handle, perform operations, and then commit or roll back:

```rust
let mut tx = db.transaction().await?;

User::create().name("Alice").exec(&mut tx).await?;
User::create().name("Bob").exec(&mut tx).await?;

tx.commit().await?;
```

If something goes wrong, roll back explicitly:

```rust
let mut tx = db.transaction().await?;

User::create().name("Alice").exec(&mut tx).await?;

// Something went wrong — discard the insert
tx.rollback().await?;
```

### Automatic Rollback on Drop

If a `Transaction` is dropped without calling `commit()` or `rollback()`, it automatically rolls back. This means you don't need to handle rollback manually in error paths:

```rust
{
    let mut tx = db.transaction().await?;
    User::create().name("Alice").exec(&mut tx).await?;
    // tx is dropped here — Alice is NOT persisted
}
```

This is especially useful with the `?` operator. If any operation inside the transaction returns an error, the transaction goes out of scope and rolls back:

```rust
async fn transfer(db: &mut Db) -> toasty::Result<()> {
    let mut tx = db.transaction().await?;

    // If either operation fails, `tx` is dropped and both are rolled back
    debit_account(&mut tx, from, amount).await?;
    credit_account(&mut tx, to, amount).await?;

    tx.commit().await?;
    Ok(())
}
```

## The Executor Trait

If you want to create a function that is generic over whether the `Db` as an active `Transaction` you can use the `toasty::Executor` trait which is implemented for both.

```rust
async fn create_user(executor: &mut dyn toasty::Executor, name: &str) -> toasty::Result<User> {
    User::create().name(name).exec(executor).await
}

// Works with Db directly
let user = create_user(&mut db, "Alice").await?;

// Works inside a transaction
let mut tx = db.transaction().await?;
let user = create_user(&mut tx, "Bob").await?;
tx.commit().await?;
```

## Nested Transactions

Calling `.transaction()` on an existing `Transaction` creates a nested transaction, implemented under the hood using database savepoints. Rolling back a nested transaction discards only its changes — the outer transaction can still commit its own work:

```rust
let mut tx = db.transaction().await?;
User::create().name("Alice").exec(&mut tx).await?;

{
    let mut nested = tx.transaction().await?;
    User::create().name("Ghost").exec(&mut nested).await?;
    nested.rollback().await?;
    // "Ghost" is discarded
}

tx.commit().await?;
// Only "Alice" is persisted
```

Nested transactions also support automatic rollback on drop, just like top-level transactions.

You can nest multiple levels deep, and you can create sequential nested transactions within the same parent:

```rust
let mut tx = db.transaction().await?;

// First nested transaction — committed
{
    let mut sp1 = tx.transaction().await?;
    User::create().name("Alice").exec(&mut sp1).await?;
    sp1.commit().await?;
}

// Second nested transaction — rolled back
{
    let mut sp2 = tx.transaction().await?;
    User::create().name("Ghost").exec(&mut sp2).await?;
    sp2.rollback().await?;
}

tx.commit().await?;
// Only "Alice" is persisted
```

Note that rolling back the outer transaction discards everything, including work from nested transactions that were already committed:

```rust
let mut tx = db.transaction().await?;

{
    let mut nested = tx.transaction().await?;
    User::create().name("Bob").exec(&mut nested).await?;
    nested.commit().await?;  // committed at savepoint level
}

tx.rollback().await?;
// Bob is gone — the outer rollback discards everything
```

## Isolation Levels and Read-Only Mode

For advanced use cases, the `TransactionBuilder` lets you configure isolation level and read-only mode:

```rust
use toasty::IsolationLevel;

let mut tx = db
    .transaction_builder()
    .isolation(IsolationLevel::Serializable)
    .read_only(true)
    .begin()
    .await?;

let users = User::all().collect::<Vec<_>>(&mut tx).await?;
tx.commit().await?;
```

Available isolation levels:

| Level | Description |
|---|---|
| `ReadUncommitted` | Lowest isolation; may see uncommitted changes from other transactions |
| `ReadCommitted` | Only sees data committed before each statement |
| `RepeatableRead` | Sees a consistent snapshot from the start of the transaction |
| `Serializable` | Strongest isolation; transactions appear to execute sequentially |

Support for specific isolation levels depends on the database backend. SQLite effectively operates at `Serializable`. PostgreSQL and MySQL support all four levels.

