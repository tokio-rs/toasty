# Implemented Advanced Patterns

This guide documents implemented, user-facing patterns that are exercised by
integration tests but were not previously covered in the main walkthrough
guides.

## 1) Model Batch Create with `create_many()`

Every model includes a `create_many()` builder for creating multiple records of
the same model in one operation.

```rust
#[derive(Debug, toasty::Model)]
struct Todo {
    #[key]
    #[auto]
    id: uuid::Uuid,
    title: String,
}

let todos = Todo::create_many()
    .item(Todo::create().title("one"))
    .with_item(|c| c.title("two"))
    .exec(&mut db)
    .await?;
```

Notes:

- Empty batches are supported and return an empty `Vec`.
- This API is separate from `toasty::create!(...)` macro batch syntax.

Evidence:

- [`one_model_batch_create.rs`](../../crates/toasty-driver-integration-suite/src/tests/one_model_batch_create.rs)

## 2) Batch Relation Create in Parent Builders

`has_many` create builders support all of the following:

- Singular inserts: `.todo(Todo::create()...)`
- Plural inserts: `.todos([Todo::create()..., ...])`
- Closure-based bulk builder: `.with_todos(|many| many.with_item(...))`

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,
    name: String,
    #[has_many]
    todos: toasty::HasMany<Todo>,
}

#[derive(Debug, toasty::Model)]
struct Todo {
    #[key]
    #[auto]
    id: uuid::Uuid,
    #[index]
    user_id: uuid::Uuid,
    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,
    title: String,
}

let user = User::create()
    .name("Ann")
    .todo(Todo::create().title("one"))
    .todos([Todo::create().title("two"), Todo::create().title("three")])
    .with_todos(|many| many.with_item(|c| c.title("four")))
    .exec(&mut db)
    .await?;
```

Evidence:

- [`has_many_batch_create.rs`](../../crates/toasty-driver-integration-suite/src/tests/has_many_batch_create.rs)

## 3) Embedded Enum Filter APIs (`is_*`, `matches`)

Embedded enums generate typed filter helpers:

- Variant checks: `.is_email()`, `.is_done()`, etc.
- Variant-field predicates: `.email().matches(|e| e.address().eq(...))`

```rust
let email_users = User::filter(User::fields().contact().is_email())
    .collect::<Vec<_>>(&mut db)
    .await?;

let alice = User::filter(
    User::fields().contact().email().matches(|e| e.address().eq("alice@example.com"))
)
.collect::<Vec<_>>(&mut db)
.await?;
```

Backend caveat:

- SQL backends support arbitrary embedded-enum filter predicates in tested
  scenarios.
- DynamoDB requires supported key access patterns; combine these filters with a
  valid partition-key predicate for portable behavior.

Evidence:

- [`filter_data_enum.rs`](../../crates/toasty-driver-integration-suite/src/tests/filter_data_enum.rs)
- [`filter_data_enum_variant_field.rs`](../../crates/toasty-driver-integration-suite/src/tests/filter_data_enum_variant_field.rs)

## 4) Index and Unique on Embedded Fields

`#[index]` and `#[unique]` work inside embedded structs and embedded enum
variant fields. Toasty flattens embedded fields into table columns and builds
indices on those flattened columns.

```rust
#[derive(Debug, toasty::Embed)]
struct Contact {
    #[unique]
    email: String,
    #[index]
    country: String,
}

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    id: String,
    contact: Contact,
}
```

```rust
#[derive(Debug, toasty::Embed)]
enum ContactInfo {
    #[column(variant = 1)]
    Email {
        #[unique]
        address: String,
    },
    #[column(variant = 2)]
    Phone {
        #[index]
        number: String,
    },
}
```

Evidence:

- [`embedded_struct_index.rs`](../../crates/toasty-driver-integration-suite/src/tests/embedded_struct_index.rs)
- [`embedded_enum_index.rs`](../../crates/toasty-driver-integration-suite/src/tests/embedded_enum_index.rs)

## 5) Self-Referential and One-Way `BelongsTo`

Toasty supports self-referential relations and one-way optional `belongs_to`
relations.

Self-referential example:

```rust
#[derive(Debug, toasty::Model)]
struct Person {
    #[key]
    #[auto]
    id: uuid::Uuid,
    name: String,

    #[index]
    parent_id: Option<uuid::Uuid>,

    #[belongs_to(key = parent_id, references = id)]
    parent: toasty::BelongsTo<Option<Person>>,

    #[has_many(pair = parent)]
    children: toasty::HasMany<Person>,
}
```

One-way optional `belongs_to` example:

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    #[index]
    profile_id: Option<uuid::Uuid>,

    #[belongs_to(key = profile_id, references = id)]
    profile: toasty::BelongsTo<Option<Profile>>,
}

#[derive(Debug, toasty::Model)]
struct Profile {
    #[key]
    #[auto]
    id: uuid::Uuid,
}
```

Evidence:

- [`belongs_to_self_referential.rs`](../../crates/toasty-driver-integration-suite/src/tests/belongs_to_self_referential.rs)
- [`belongs_to_one_way.rs`](../../crates/toasty-driver-integration-suite/src/tests/belongs_to_one_way.rs)

## 6) Automatic SQL Atomic Wrapping for Multi-Op Mutations

When a mutation plan requires multiple SQL operations (for example, creating a
parent and dependent relations), Toasty wraps the plan in a transaction
automatically. In tested SQL paths:

- Multi-op plans are wrapped in `BEGIN ... COMMIT`.
- Failure in later operations triggers `ROLLBACK`.
- Single-op plans skip transaction wrapping overhead.

Evidence:

- [`tx_atomic_stmt.rs`](../../crates/toasty-driver-integration-suite/src/tests/tx_atomic_stmt.rs)

For composite-key and migration-focused behavior, continue with
[composite-keys-migrations-and-known-gaps.md](composite-keys-migrations-and-known-gaps.md).
