# Gaps: Query Index Edge Cases, Macros, and Many-to-Many

This guide documents feature items 21 through 25:

21. DynamoDB query/index rewrite edge cases (partial)
22. `toasty::query!` macro (not implemented)
23. `include_schema!` macro (not implemented)
24. `toasty::update!` macro (not implemented)
25. Many-to-many relationships (not implemented)

## 21) DynamoDB Query/Index Rewrite Edge Cases (Partial)

Toasty supports many indexed query paths on DynamoDB, including OR rewriting in
some cases. However, a subset of branch-shape combinations is still marked TODO
in the index rewrite/match pipeline.

Practical impact:

- Common key-based predicates work.
- Some complex OR forms on key predicates may fail or be unsupported.

Practical guidance:

- Prefer simple, uniform branch shapes in OR predicates on key fields.
- If a specific query shape fails, split it into separate queries at the
  application layer and merge results.

## 22) `toasty::query!` Macro (Not Implemented)

`toasty::query!` exists as a proc-macro entry point but is currently a stub.

Current workaround: use the builder DSL directly.

```rust
let users = User::all()
    .filter(User::fields().name().eq("Alice"))
    .order_by(User::fields().id().asc())
    .collect::<Vec<_>>(&db)
    .await?;
```

## 23) `include_schema!` Macro (Not Implemented)

`include_schema!` is currently a placeholder and not available for production
use.

Current workaround:

- Use normal `#[derive(toasty::Model)]` / `#[derive(toasty::Embed)]` definitions.
- Initialize schema through your usual app startup flow and migrations.

## 24) `toasty::update!` Macro (Not Implemented)

`toasty::update!` is a roadmap item and is not currently implemented.

Current workaround: use generated update builders.

```rust
user.update()
    .name("Bob")
    .exec(&db)
    .await?;

User::filter_by_id(user.id)
    .update()
    .name("Bob")
    .exec(&db)
    .await?;
```

## 25) Many-to-Many Relationships (Not Implemented)

Direct many-to-many relation syntax is not yet available.

Current workaround: model an explicit junction table.

```rust
#[derive(Debug, toasty::Model)]
struct UserRole {
    #[key]
    user_id: u64,

    #[key]
    role_id: u64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,

    #[belongs_to(key = role_id, references = id)]
    role: toasty::BelongsTo<Role>,
}
```

This preserves full control over association metadata and works with current
CRUD/query patterns.

For the next five gap areas, continue with
[gaps-polymorphic-deferred-upsert-raw-sql-and-dynamodb-migrations.md](gaps-polymorphic-deferred-upsert-raw-sql-and-dynamodb-migrations.md).
