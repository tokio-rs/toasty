# Modeling and Querying Basics

This guide covers core modeling and query workflows that are implemented and
actively exercised in Toasty:

1. Core model derive and schema attributes
2. Auto/default/update field behavior
3. CRUD and generated query methods
4. Filter DSL
5. Sorting, limit, and pagination

## 1) Core Model Derive and Schema Attributes

Toasty models are plain Rust structs with `#[derive(toasty::Model)]`.

Common field and schema attributes:

- `#[key]`: marks a primary key field.
- `#[index]`: creates an index and generated `filter_by_*` helpers.
- `#[unique]`: creates a unique constraint/index.
- `#[column(...)]`: customizes DB column name/type.

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    #[unique]
    email: String,

    #[index]
    name: String,

    #[column("display_name", type = varchar(80))]
    nickname: String,
}
```

## 2) Auto / Default / Update Field Behavior

Toasty supports three common value-population mechanisms:

- `#[auto]`: generated values (for example key auto-increment/UUID).
- `#[default(expr)]`: default on create when omitted.
- `#[update(expr)]`: value refreshed on create + update unless overridden.

```rust
#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: uuid::Uuid,

    title: String,

    #[default(0)]
    views: i64,

    #[update(jiff::Timestamp::now())]
    updated_at: jiff::Timestamp,
}
```

```rust
let mut post = Post::create().title("hello").exec(&mut db).await?;

// updated_at refreshed automatically
post.update().title("hello again").exec(&mut db).await?;
```

## 3) CRUD and Generated Query Methods

Toasty generates create/read/update/delete flows from your model schema.

- Create: `Model::create()...exec(&mut db).await?`
- Read by key: `Model::get_by_<key>(...)`
- Indexed queries: `Model::filter_by_<indexed_field>(...)`
- Update: instance update or query update builder
- Delete: instance delete or query delete

```rust
let user = User::create()
    .name("Alice")
    .email("alice@example.com")
    .exec(&mut db)
    .await?;

let user = User::get_by_id(&mut db, &user.id).await?;

let users = User::filter_by_name("Alice")
    .collect::<Vec<_>>(&mut db)
    .await?;
```

## 4) Filter DSL

Beyond generated `filter_by_*` methods, Toasty provides a typed field DSL.

Supported patterns include:

- Comparisons: `eq`, `ne`, `gt`, `ge`, `lt`, `le`
- Boolean composition: `.and(...)`, `.or(...)`, `.not()`
- Nullable checks: `.is_some()`, `.is_none()`

```rust
let named_users = User::filter(
    User::fields()
        .name()
        .eq("Alice")
        .or(User::fields().nickname().eq("Ali"))
)
.collect::<Vec<_>>(&mut db)
.await?;
```

## 5) Sorting, Limit, and Pagination

Toasty supports:

- Ordering: `.order_by(...)`
- Direct limits: `.limit(n)`
- Cursor pagination: `.paginate(n)` with `Page<T>`, `.next()`, `.prev()`,
  `.after(cursor)`, `.before(cursor)`

```rust
use toasty::Page;

let page: Page<Post> = Post::all()
    .order_by(Post::fields().created_at().desc())
    .paginate(20)
    .collect(&mut db)
    .await?;

let top_five: Vec<Post> = Post::all()
    .order_by(Post::fields().score().desc())
    .limit(5)
    .collect(&mut db)
    .await?;
```

For deeper pagination details, see [pagination.md](pagination.md).

For implemented advanced patterns not covered in this basics guide (for
example `create_many`, embedded enum filter helpers, and self-referential
relations), see [implemented-advanced-patterns.md](implemented-advanced-patterns.md).

For the next five implemented areas, continue with
[relationships-loading-transactions-batch.md](relationships-loading-transactions-batch.md).
