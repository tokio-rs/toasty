# Relationships, Loading, Transactions, and Batch Queries

This guide documents the next five implemented feature areas in Toasty:

6. Relationship CRUD (`HasMany`, `BelongsTo`, `HasOne`)
7. Association link/unlink and scoped relation operations
8. Eager loading with `.include(...)` (including nested preloads)
9. Interactive SQL transactions (including nested savepoints)
10. Batch query API (`toasty::batch`)

## 6) Relationship CRUD (`HasMany`, `BelongsTo`, `HasOne`)

Toasty supports relationship modeling and CRUD flows across related models.

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    #[has_many]
    todos: toasty::HasMany<Todo>,
}

#[derive(Debug, toasty::Model)]
struct Todo {
    #[key]
    #[auto]
    id: u64,

    #[index]
    user_id: u64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,

    title: String,
}
```

Create and read through relationships:

```rust
let user = User::create().exec(&mut db).await?;

let todo = user
    .todos()
    .create()
    .title("write docs")
    .exec(&mut db)
    .await?;

let owner = todo.user().get(&mut db).await?;
assert_eq!(owner.id, user.id);
```

`HasOne` is also supported for one-to-one patterns (optional or required,
depending on your field types).

## 7) Association Link/Unlink and Scoped Relation Operations

For existing records, you can explicitly link and unlink associations.

```rust
// Link an existing todo to the user
user.todos().insert(&mut db, &todo).await?;

// Scoped query: only searches within user.todos()
let same_todo = user.todos().get_by_id(&mut db, &todo.id).await?;

// Unlink/remove association
user.todos().remove(&mut db, &todo).await?;
```

Notes:
- Scoped query/update/delete builders (for example `user.todos().filter_by_id(...)`)
  apply relation constraints automatically.
- Unlink behavior depends on schema semantics (for example optional FK can
  become `NULL`, required FK may cause delete/reassign semantics).

## 8) Eager Loading with `.include(...)`

Toasty can preload related records to avoid follow-up N+1 query patterns.

Basic include:

```rust
let user = User::filter_by_id(user_id)
    .include(User::fields().todos())
    .get(&mut db)
    .await?;
```

Multiple includes:

```rust
let todos = Todo::all()
    .include(Todo::fields().user())
    .include(Todo::fields().user().todos())
    .collect::<Vec<_>>(&mut db)
    .await?;
```

Nested include:

```rust
let user = User::filter_by_id(user_id)
    .include(User::fields().todos().user())
    .get(&mut db)
    .await?;
```

## 9) Interactive SQL Transactions

Transactions are supported on SQL backends (SQLite/PostgreSQL/MySQL).

Basic transaction:

```rust
let mut tx = db.transaction().await?;

User::create().name("Alice").exec(&mut tx).await?;
User::create().name("Bob").exec(&mut tx).await?;

tx.commit().await?;
```

Nested transaction (savepoint):

```rust
let mut tx = db.transaction().await?;

{
    let mut nested = tx.transaction().await?;
    User::create().name("temp").exec(&mut nested).await?;
    nested.rollback().await?;
}

tx.commit().await?;
```

Advanced configuration:

```rust
use toasty::IsolationLevel;

let mut tx = db
    .transaction_builder()
    .isolation(IsolationLevel::Serializable)
    .read_only(true)
    .begin()
    .await?;
```

For full details, see [transactions.md](transactions.md).

## 10) Batch Query API (`toasty::batch`)

You can execute multiple typed queries in one batch and deserialize into tuples.

```rust
let (users, posts): (Vec<User>, Vec<Post>) = toasty::batch((
    User::filter_by_name("Alice"),
    Post::filter_by_title("Hello"),
))
.exec(&mut db)
.await?;
```

You can also batch multiple queries of the same model:

```rust
let (alices, bobs): (Vec<User>, Vec<User>) = toasty::batch((
    User::filter_by_name("Alice"),
    User::filter_by_name("Bob"),
))
.exec(&mut db)
.await?;
```

Current status:
- Integration-tested on SQL backends.

For additional implemented advanced patterns around relation batch creation and
automatic SQL atomic wrapping, see
[implemented-advanced-patterns.md](implemented-advanced-patterns.md).

For the next five implemented areas, continue with
[macros-embedded-serialized-and-numeric-types.md](macros-embedded-serialized-and-numeric-types.md).
