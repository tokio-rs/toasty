# Toasty

**Current status: Incubating - Toasty is not ready for production usage. The API
is still evolving and documentation is lacking.**

Toasty is an ORM for the Rust programming language that prioritizes ease-of-use.
It currently supports SQL databases (SQLite, PostgreSQL, MySQL) and DynamoDB.
Note that Toasty does not hide database capabilities. Instead, Toasty exposes
features based on the target database.

## Documentation Map

If you are evaluating Toasty, start here:

- Full feature matrix (implemented, partial, missing):
  [docs/feature-status.md](docs/feature-status.md)

Implemented feature guides:

- [modeling-and-querying-basics.md](docs/guide/modeling-and-querying-basics.md)
- [relationships-loading-transactions-batch.md](docs/guide/relationships-loading-transactions-batch.md)
- [macros-embedded-serialized-and-numeric-types.md](docs/guide/macros-embedded-serialized-and-numeric-types.md)
- [implemented-advanced-patterns.md](docs/guide/implemented-advanced-patterns.md)
- [composite-keys-migrations-and-known-gaps.md](docs/guide/composite-keys-migrations-and-known-gaps.md)

Known-gap guides:

- [gaps-query-macros-and-many-to-many.md](docs/guide/gaps-query-macros-and-many-to-many.md)
- [gaps-polymorphic-deferred-upsert-raw-sql-and-dynamodb-migrations.md](docs/guide/gaps-polymorphic-deferred-upsert-raw-sql-and-dynamodb-migrations.md)
- [gaps-cassandra-driver.md](docs/guide/gaps-cassandra-driver.md)

## Feature Snapshot

### What Works Today (Implemented and Well-Exercised)

- Modeling and schema attributes: `#[derive(toasty::Model)]`, `#[key]`,
  `#[index]`, `#[unique]`, `#[column(...)]`, `#[auto]`, `#[default]`,
  `#[update]`. (confidence: 94%)
- Querying: generated `filter_by_*` methods, field DSL filters, logical
  composition (`and`, `or`, `not`), nullable filters (`is_some`, `is_none`),
  sorting, `.limit()`, and cursor pagination. (confidence: 90%)
- Relationships: `HasMany`, `BelongsTo`, and `HasOne` CRUD flows, scoped
  queries, link/unlink operations, and eager loading with `.include(...)`
  including nested preloads. (confidence: 90%)
- Transactions (SQL): interactive transactions, nested savepoints, rollback on
  drop, and transaction builder controls (isolation level and read-only mode).
  (confidence: 97%)
- Data types and field encodings: primitive types, UUIDs, jiff time types,
  `rust_decimal::Decimal`, `bigdecimal::BigDecimal`, embedded structs/enums, and
  `#[serialize(json)]` fields. (confidence: 88%)
- Composite-key workflows: tested paths for batch get by composite key, and
  partition/local key update/delete/query behavior. (confidence: 80%)
- Batch and macros: `toasty::batch(...)` and `toasty::create!(...)`.
  (confidence: 93%)
- Schema management: migration CLI commands (`generate`, `apply`, `snapshot`,
  `drop`, `reset`) for SQL backends. (confidence: 90%)

### What's Partial or Missing

- Composite-key parity is still partial in some engine and DynamoDB paths.
  (confidence: 95%)
- `toasty::query!` and `include_schema!` macros are currently stubs.
  (confidence: 99%)
- `toasty::update!` macro is not implemented. (confidence: 96%)
- `.then_by()` convenience ordering is not implemented (manual multi-column
  ordering works via `OrderBy::from([...])`). (confidence: 95%)
- Many-to-many relations, polymorphic associations, deferred fields, upsert,
  and raw SQL escape-hatch APIs are still roadmap items. (confidence: 82%)
- DynamoDB migration generation/apply support is not implemented.
  (confidence: 99%)
- Cassandra driver support is not implemented. (confidence: 96%)

## Using Toasty

You will define your data model using Rust structs annotated with the
`#[derive(toasty::Model)]` derive macro. Here is the
[hello-toasty](examples/hello-toasty/src/main.rs) example.

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: uuid::Uuid,

    name: String,

    #[unique]
    email: String,

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
```

Then, you can easily work with the data model:

```rust
// Create a new user and give them some todos.
let user = User::create()
    .name("John Doe")
    .email("john@example.com")
    .todo(Todo::create().title("Make pizza"))
    .todo(Todo::create().title("Finish Toasty"))
    .todo(Todo::create().title("Sleep"))
    .exec(&mut db)
    .await?;

// Load the user from the database
let user = User::get_by_id(&mut db, &user.id).await?;

// Load and iterate the user's todos
let mut todos = user.todos().all(&mut db).await?;

while let Some(todo) = todos.next().await {
    let todo = todo?;
    println!("{:#?}", todo);
}
```

## Quick Feature Guides

These are short, copyable patterns for the main implemented features.

### Querying and Filtering

Generated methods and the field DSL can be used together:

```rust
let john = User::filter_by_email("john@example.com")
    .get(&mut db)
    .await?;

let users: Vec<User> = User::filter(
    User::fields()
        .name()
        .eq("John Doe")
        .or(User::fields().name().eq("Jane Doe"))
)
.order_by(User::fields().name().asc())
.limit(20)
.collect(&mut db)
.await?;
```

### Eager Loading (`.include(...)`)

Preload relations to avoid N+1 query patterns:

```rust
let user = User::filter_by_id(user.id)
    .include(User::fields().todos())
    .get(&mut db)
    .await?;

for todo in user.todos.get() {
    println!("preloaded todo: {}", todo.title);
}
```

### Cursor Pagination

Use `.paginate(n)` and navigate with `next()` / `prev()`:

```rust
let page = Todo::all()
    .order_by(Todo::fields().title().asc())
    .paginate(10)
    .collect(&mut db)
    .await?;

if let Some(next_page) = page.next(&mut db).await? {
    println!("next page size = {}", next_page.len());
}
```

### Transactions (SQL Backends)

Interactive transactions support rollback and nested savepoints:

```rust
let mut tx = db.transaction().await?;

User::create().name("Alice").email("alice@example.com").exec(&mut tx).await?;
User::create().name("Bob").email("bob@example.com").exec(&mut tx).await?;

tx.commit().await?;
```

### Batch APIs and Create Macros

Batch reads:

```rust
let (users, todos): (Vec<User>, Vec<Todo>) = toasty::batch((
    User::filter_by_name("John Doe"),
    Todo::all(),
))
.exec(&mut db)
.await?;
```

Batch create builder:

```rust
let todos = Todo::create_many()
    .item(Todo::create().title("one"))
    .item(Todo::create().title("two"))
    .exec(&mut db)
    .await?;
```

Create macro:

```rust
let user = toasty::create!(User, {
    name: "Carl",
    email: "carl@example.com",
    todos: [{ title: "first" }, { title: "second" }]
})
.exec(&mut db)
.await?;
```

### Embedded and Serialized Fields

Model rich fields with `Embed` and JSON serialization:

```rust
#[derive(Debug, toasty::Embed)]
struct Address {
    street: String,
    city: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Meta {
    tags: Vec<String>,
}

#[derive(Debug, toasty::Model)]
struct Profile {
    #[key]
    #[auto]
    id: uuid::Uuid,
    address: Address,
    #[serialize(json)]
    meta: Meta,
}
```

### Migrations and Reset (SQL)

Migration CLI commands:

```bash
my-app-cli migration generate --name add_todo_status
my-app-cli migration apply
my-app-cli migration snapshot
```

Reset helpers:

```rust
db.reset_db().await?;
db.push_schema().await?;
```

For deeper coverage and backend caveats, use the guide set in
[Documentation Map](#documentation-map).

## SQL and NoSQL

Toasty supports both SQL and NoSQL databases. Current drivers are SQLite,
PostgreSQL, MySQL, and DynamoDB. However, it does not aim to abstract the
database. Instead, Toasty leans into the target database's capabilities and
aims to help the user avoid issuing inefficient queries for that database.

When targeting both SQL and NoSQL databases, Toasty generates query methods
(e.g. `find_by_id` only for access patterns that are indexed). When targeting a
SQL database, Toasty might allow arbitrary additional query constraints. When
targeting a NoSQL database, Toasty will only allow constraints that the
specific target database can execute. For example, with DynamoDB, query methods
might be generated based on the table's primary key, and additional constraints
may be set for the sort key.

## Application data model vs. database schema

Toasty decouples the application data model from the database's schema. By
default, a toasty application schema will map 1-1 with a database schema.
However, additional annotations may be specified to customize how the
application data model maps to the database schema.

## Current status and roadmap

Toasty is still in the early development stages and is considered
**incubating**. There is no commitment to on-going maintenance or development.
At some point in the future, as the project evolves, this may change. As such,
we encourage you to explore, experiment, and contribute to Toasty, but do not
try using it in production.

Immediate next steps for the project are to fill obvious gaps, such as implement
error handling, remove panics throughout the code base, support additional data
types, and write documentation. After that, development will be based on
feedback and contribution.

## License

This project is licensed under the [MIT license].

[MIT license]: LICENSE

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Toasty by you, shall be licensed as MIT, without any additional
terms or conditions.
