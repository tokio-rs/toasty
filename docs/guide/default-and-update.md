# Default and Update Expressions

Toasty supports `#[default]` and `#[update]` field attributes for automatically populating field values on create and update operations. This eliminates boilerplate for common patterns like timestamps and counters.

## `#[default(expr)]`

Provides a default value when creating a record. The field becomes optional in the create builder — if the user doesn't set it, the expression is used.

```rust
#[derive(toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: uuid::Uuid,

    title: String,

    #[default(0)]
    view_count: i64,

    #[default(jiff::Timestamp::now())]
    created_at: jiff::Timestamp,
}
```

```rust
// view_count defaults to 0, created_at defaults to now()
let post = Post::create()
    .title("Hello")
    .exec(&db)
    .await?;

assert_eq!(post.view_count, 0);

// Explicit values override the default
let post = Post::create()
    .title("Hello")
    .view_count(42)
    .created_at(some_past_timestamp)
    .exec(&db)
    .await?;
```

The expression is evaluated at builder construction time (when `Model::create()` is called).

## `#[update(expr)]`

Automatically sets a value on every **create and update** operation. This is a superset of `#[default]` — a field with `#[update]` does not also need `#[default]`.

```rust
#[derive(toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: uuid::Uuid,

    title: String,

    #[update(jiff::Timestamp::now())]
    updated_at: jiff::Timestamp,
}
```

```rust
// On create: updated_at is set to now()
let mut post = Post::create()
    .title("Hello")
    .exec(&db)
    .await?;

// On update: updated_at is automatically refreshed
post.update()
    .title("Updated")
    .exec(&db)
    .await?;

// Explicit values still override the automatic value
post.update()
    .title("Backdated")
    .updated_at(some_past_timestamp)
    .exec(&db)
    .await?;
```

On create, the expression is evaluated at builder construction time. On update, it is evaluated at builder construction time as well, so the user's explicit `.field(val)` call naturally overwrites it.

## Combining `#[default]` and `#[update]`

When both are present on the same field, `#[default]` controls the create-time value and `#[update]` controls the update-time value. On create, `#[default]` takes priority.

```rust
#[derive(toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: uuid::Uuid,

    title: String,

    // On create: defaults to "draft". On update: automatically set to "edited".
    #[default("draft".to_string())]
    #[update("edited".to_string())]
    status: String,
}
```

```rust
let mut post = Post::create()
    .title("Hello")
    .exec(&db)
    .await?;

assert_eq!(post.status, "draft");  // #[default] applied on create

post.update()
    .title("Updated")
    .exec(&db)
    .await?;

assert_eq!(post.status, "edited"); // #[update] applied on update
```

## `#[auto]` on Timestamp Fields

As a shorthand, `#[auto]` is extended for well-known timestamp field names:

- `created_at` with `#[auto]` is equivalent to `#[default(jiff::Timestamp::now())]`
- `updated_at` with `#[auto]` is equivalent to `#[update(jiff::Timestamp::now())]`

```rust
#[derive(toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: uuid::Uuid,

    title: String,

    #[auto]
    created_at: jiff::Timestamp,

    #[auto]
    updated_at: jiff::Timestamp,
}
```

This is identical in behavior to the explicit form — `created_at` is set once at creation, `updated_at` is refreshed on every create and update.

## Validation Rules

| Combination | Allowed? |
|---|---|
| `#[default]` alone | Yes |
| `#[update]` alone | Yes |
| `#[default]` + `#[update]` | Yes |
| `#[default]` + `#[auto]` | No — compile error |
| `#[update]` + `#[auto]` | No — compile error |
| `#[default]` on relation field | No — compile error |
| `#[update]` on relation field | No — compile error |

## Design Notes

- These are **app-level defaults** evaluated in Rust. They do not generate `DEFAULT` clauses in database DDL.
- The expressions are arbitrary Rust code — you must ensure correct imports and types are in scope.
- Both attributes are only valid on primitive (non-relation) fields.
