# Filtering with Expressions

The `filter_by_*` methods generated for indexed fields cover simple equality
lookups. For anything else — comparisons, combining conditions with AND/OR,
checking for null — use `Model::filter()` with field expressions.

| Expression | Description | SQL equivalent |
|---|---|---|
| [`.eq(value)`](#equality-and-inequality) | Equal | `= value` |
| [`.ne(value)`](#equality-and-inequality) | Not equal | `!= value` |
| [`.gt(value)`](#ordering-comparisons) | Greater than | `> value` |
| [`.ge(value)`](#ordering-comparisons) | Greater than or equal | `>= value` |
| [`.lt(value)`](#ordering-comparisons) | Less than | `< value` |
| [`.le(value)`](#ordering-comparisons) | Less than or equal | `<= value` |
| [`.in_list([...])`](#membership-with-in_list) | Value in list | `IN (...)` |
| [`.is_none()`](#null-checks) | Null check (`Option` fields) | `IS NULL` |
| [`.is_some()`](#null-checks) | Not-null check (`Option` fields) | `IS NOT NULL` |
| [`.and(expr)`](#combining-with-and) | Both conditions true | `AND` |
| [`.or(expr)`](#combining-with-or) | Either condition true | `OR` |
| [`.not()` / `!expr`](#negation-with-not) | Negate condition | `NOT` |
| [`.any(expr)`](#filtering-on-associations) | Any related record matches (`HasMany`) | `IN (SELECT ...)` |

## Field paths

Every model has a `fields()` method that returns typed accessors for each field.
These accessors produce field paths that you pass to comparison methods:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[index]
#     country: String,
# }
# fn __example() {
// User::fields() returns a struct with one method per field
let name_path = User::fields().name();
let country_path = User::fields().country();
# }
```

Field paths are the building blocks for filter expressions. Call a comparison
method on a path to get an `Expr<bool>`, then pass that expression to
`Model::filter()`.

## Equality and inequality

`.eq()` tests whether a field equals a value. `.ne()` tests whether it does not:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[index]
#     country: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Find users named "Alice"
let users = User::filter(User::fields().name().eq("Alice"))
    .exec(&mut db)
    .await?;

// Find users not from the US
let users = User::filter(User::fields().country().ne("US"))
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

## Ordering comparisons

Four methods compare field values by order:

| Method | Meaning |
|---|---|
| `.gt(value)` | Greater than |
| `.ge(value)` | Greater than or equal |
| `.lt(value)` | Less than |
| `.le(value)` | Less than or equal |

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Event {
#     #[key]
#     #[auto]
#     id: u64,
#     kind: String,
#     timestamp: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Events after timestamp 1000
let events = Event::filter(Event::fields().timestamp().gt(1000))
    .exec(&mut db)
    .await?;

// Events at or before timestamp 500
let events = Event::filter(Event::fields().timestamp().le(500))
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

## Membership with `in_list`

`.in_list()` tests whether a field's value is in a given list, equivalent to
SQL's `IN` clause:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[index]
#     country: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let users = User::filter(
    User::fields().country().in_list(["US", "CA", "MX"]),
)
.exec(&mut db)
.await?;
# Ok(())
# }
```

## Null checks

For `Option<T>` fields, use `.is_none()` and `.is_some()` to filter by whether
the value is null:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     bio: Option<String>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Users who have not set a bio
let users = User::filter(User::fields().bio().is_none())
    .exec(&mut db)
    .await?;

// Users who have set a bio
let users = User::filter(User::fields().bio().is_some())
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

These methods are only available on paths to `Option<T>` fields. Calling
`.is_none()` on a non-optional field is a compile error.

## Combining with AND

`.and()` combines two expressions so both must be true:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Event {
#     #[key]
#     #[auto]
#     id: u64,
#     kind: String,
#     timestamp: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let events = Event::filter(
    Event::fields()
        .kind()
        .eq("info")
        .and(Event::fields().timestamp().gt(1000)),
)
.exec(&mut db)
.await?;
# Ok(())
# }
```

Chain multiple `.and()` calls to add more conditions:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Event {
#     #[key]
#     #[auto]
#     id: u64,
#     kind: String,
#     timestamp: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let events = Event::filter(
    Event::fields()
        .kind()
        .eq("info")
        .and(Event::fields().timestamp().gt(1000))
        .and(Event::fields().timestamp().lt(2000)),
)
.exec(&mut db)
.await?;
# Ok(())
# }
```

You can also add AND conditions by chaining `.filter()` on a query. Each
`.filter()` call adds another AND condition:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Event {
#     #[key]
#     #[auto]
#     id: u64,
#     kind: String,
#     timestamp: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Equivalent to the previous example
let events = Event::filter(Event::fields().kind().eq("info"))
    .filter(Event::fields().timestamp().gt(1000))
    .filter(Event::fields().timestamp().lt(2000))
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

## Combining with OR

`.or()` combines two expressions so at least one must be true:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     age: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Users named "Alice" or aged 35
let users = User::filter(
    User::fields()
        .name()
        .eq("Alice")
        .or(User::fields().age().eq(35)),
)
.exec(&mut db)
.await?;
# Ok(())
# }
```

Expressions evaluate left to right through method chaining. Each method wraps
everything before it. `a.or(b).and(c)` produces `(a OR b) AND c`:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     age: i64,
#     active: bool,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// (name = "Alice" OR age = 35) AND active = true
let users = User::filter(
    User::fields()
        .name()
        .eq("Alice")
        .or(User::fields().age().eq(35))
        .and(User::fields().active().eq(true)),
)
.exec(&mut db)
.await?;
# Ok(())
# }
```

To group differently, build sub-expressions and pass them as arguments. Here,
`a.or(b.and(c))` produces `a OR (b AND c)`:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     age: i64,
#     active: bool,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// name = "Alice" OR (age = 35 AND active = true)
let users = User::filter(
    User::fields().name().eq("Alice").or(User::fields()
        .age()
        .eq(35)
        .and(User::fields().active().eq(true))),
)
.exec(&mut db)
.await?;
# Ok(())
# }
```

## Negation with NOT

`.not()` negates an expression. The `!` operator works too:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     age: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Users not named "Alice"
let users = User::filter(User::fields().name().eq("Alice").not())
    .exec(&mut db)
    .await?;

// Same thing with the ! operator
let users = User::filter(!User::fields().name().eq("Alice"))
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

NOT works on compound expressions too:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     age: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// NOT (name = "Alice" OR name = "Bob")
let users = User::filter(
    !(User::fields()
        .name()
        .eq("Alice")
        .or(User::fields().name().eq("Bob"))),
)
.exec(&mut db)
.await?;
# Ok(())
# }
```

## Filtering on associations

For `HasMany` relations, `.any()` tests whether at least one related record
matches a condition. This generates a subquery:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_many]
#     todos: toasty::HasMany<Todo>,
# }
# #[derive(Debug, toasty::Model)]
# struct Todo {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     user_id: u64,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::BelongsTo<User>,
#     title: String,
#     complete: bool,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Find users who have at least one incomplete todo
let users = User::filter(
    User::fields()
        .todos()
        .any(Todo::fields().complete().eq(false)),
)
.exec(&mut db)
.await?;
# Ok(())
# }
```

The path `User::fields().todos()` refers to the HasMany relation. Calling
`.any()` on it takes a filter expression on the child model (`Todo`) and
produces a filter expression on the parent (`User`).

