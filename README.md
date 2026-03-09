# Toasty

**Current status: Incubating - Toasty is not ready for production usage. The API
is still evolving and documentation is lacking.**

Toasty is an ORM for the Rust programming language that prioritizes ease-of-use.
It currently supports SQL databases (SQLite, PostgreSQL, MySQL) and DynamoDB.
Note that Toasty does not hide database capabilities. Instead, Toasty exposes
features based on the target database.

## Feature Status

The project has more implemented functionality than the high-level docs currently
describe. This README summarizes tested capabilities.

For a detailed matrix of implemented, partial, and missing features, see
[docs/feature-status.md](docs/feature-status.md).

For practical docs on the first five core implemented areas, see
[docs/guide/modeling-and-querying-basics.md](docs/guide/modeling-and-querying-basics.md).

For the next five implemented areas (relationships, loading, transactions, and
batch execution), see
[docs/guide/relationships-loading-transactions-batch.md](docs/guide/relationships-loading-transactions-batch.md).

For the next five implemented areas after that (create macro, embedded and
serialized fields, and numeric/time type support), see
[docs/guide/macros-embedded-serialized-and-numeric-types.md](docs/guide/macros-embedded-serialized-and-numeric-types.md).

For additional implemented advanced patterns (batch create builders, embedded
enum filters, embedded-field indexes, and self-referential relation patterns),
see
[docs/guide/implemented-advanced-patterns.md](docs/guide/implemented-advanced-patterns.md).

For the next five areas (composite-key workflows, migrations, reset, and two
current partial gaps), see
[docs/guide/composite-keys-migrations-and-known-gaps.md](docs/guide/composite-keys-migrations-and-known-gaps.md).

For the next five gaps (DynamoDB edge-case rewriting, missing macros, and
many-to-many), see
[docs/guide/gaps-query-macros-and-many-to-many.md](docs/guide/gaps-query-macros-and-many-to-many.md).

For the next five gaps after that (polymorphic associations, deferred fields,
upsert, raw SQL escape hatch, and DynamoDB migrations), see
[docs/guide/gaps-polymorphic-deferred-upsert-raw-sql-and-dynamodb-migrations.md](docs/guide/gaps-polymorphic-deferred-upsert-raw-sql-and-dynamodb-migrations.md).

For the final remaining documented gap (Cassandra driver support), see
[docs/guide/gaps-cassandra-driver.md](docs/guide/gaps-cassandra-driver.md).

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
    id: u64,

    name: String,

    #[unique]
    email: String,

    #[has_many]
    todos: toasty::HasMany<Todo>,

    moto: Option<String>,
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

Then, you can easily work with the data model:

```rust
// Create a new user and give them some todos.
let user = User::create()
    .name("John Doe")
    .email("john@example.com")
    .todo(Todo::create().title("Make pizza"))
    .todo(Todo::create().title("Finish Toasty"))
    .todo(Todo::create().title("Sleep"))
    .exec(&db)
    .await?;

// Load the user from the database
let user = User::get_by_id(&db, &user.id).await?

// Load and iterate the user's todos
let mut todos = user.todos().all(&db).await.unwrap();

while let Some(todo) = todos.next().await {
    let todo = todo.unwrap();
    println!("{:#?}", todo);
}
```

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
