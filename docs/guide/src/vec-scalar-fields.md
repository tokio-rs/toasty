# `Vec<scalar>` Fields

A `Vec<scalar>` field stores a homogeneous, ordered collection of
scalar values in a single column — `tags: Vec<String>`, `scores:
Vec<i64>`, `weights: Vec<f64>`. Toasty stores the collection directly;
you do not wrap it in JSON by hand or manage a separate join table.

The element type must be a scalar: any primitive other than `u8`, plus
`String`, `Uuid`, the decimal types, and the `jiff` date/time types.
`Vec<u8>` keeps its existing meaning — a single binary blob, not a
collection of one-byte integers.

Storage depends on the driver:

| Driver | Representation |
|---|---|
| PostgreSQL | Native array column (`text[]`, `int8[]`, `double precision[]`, …) |
| MySQL | `JSON` column |
| SQLite | JSON-encoded text |
| DynamoDB | List `L` attribute |

All four built-in drivers support `Vec<scalar>` fields. A driver that
does not will reject the model at schema build with an error naming the
unsupported field rather than mis-storing it. The incremental update
builders have narrower support — see [Driver support](#driver-support).

## Defining a scalar collection field

Declare the field as a `Vec<T>` for a scalar `T`. No attribute is
needed:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct Article {
    #[key]
    #[auto]
    id: u64,

    title: String,

    tags: Vec<String>,
    scores: Vec<i64>,
}
```

A `Vec<scalar>` field is always present — there is no unset state. A
row with no elements holds an empty list, not `NULL`.

## Creating records

The field accepts any value that converts into a list: a `Vec<T>`, an
array literal `[T; N]`, or a slice. The `create!` macro and the create
builder take the same forms.

With the `create!` macro — an array literal works, no `vec!` needed:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Article {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     tags: Vec<String>,
#     scores: Vec<i64>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let article = toasty::create!(Article {
    title: "Hello",
    tags: ["rust", "toasty"],
    scores: [1, 2, 3],
})
.exec(&mut db)
.await?;

// A `Vec` works the same way.
let tags = vec!["rust".to_string(), "toasty".to_string()];
let article = toasty::create!(Article {
    title: "Hello",
    tags: tags,
    scores: Vec::<i64>::new(),
})
.exec(&mut db)
.await?;
# Ok(())
# }
```

With the create builder, the per-field setter accepts the same forms:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Article {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     tags: Vec<String>,
#     scores: Vec<i64>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let article = Article::create()
    .title("Hello")
    .tags(["rust", "toasty"])
    .scores(vec![1, 2, 3])
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

The batch form of `create!` works as well:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Article {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     tags: Vec<String>,
#     scores: Vec<i64>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
toasty::create!(Article::[
    { title: "First", tags: ["rust"], scores: [1] },
    { title: "Second", tags: ["toasty", "orm"], scores: [2, 3] },
])
.exec(&mut db)
.await?;
# Ok(())
# }
```

## Querying

A path to a `Vec<scalar>` field exposes array predicates:

| Method | Meaning |
|---|---|
| `.contains(value)` | The array contains `value`. |
| `.is_superset(values)` | The array contains every element of `values`. |
| `.intersects(values)` | The array shares at least one element with `values`. |
| `.len()` | The array's length, as `Expr<i64>`. |
| `.is_empty()` | The array is empty. |

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Article {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     tags: Vec<String>,
#     scores: Vec<i64>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Articles tagged "rust".
let tagged = Article::filter(Article::fields().tags().contains("rust"))
    .exec(&mut db)
    .await?;

// Articles tagged with both "rust" and "orm".
let both = Article::filter(
    Article::fields().tags().is_superset(["rust", "orm"]),
)
.exec(&mut db)
.await?;

// Articles sharing at least one tag with this set.
let related = Article::filter(
    Article::fields().tags().intersects(["rust", "toasty"]),
)
.exec(&mut db)
.await?;

// Articles with more than three tags.
let many = Article::filter(Article::fields().tags().len().gt(3))
    .exec(&mut db)
    .await?;

// Articles with no tags.
let untagged = Article::filter(Article::fields().tags().is_empty())
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

`.len()` produces an `Expr<i64>` rather than a boolean, so pair it with
a comparison (`.gt()`, `.eq()`, …) to form a predicate.

These predicates lower to PostgreSQL-specific operators (`@>`, `&&`,
`= ANY(col)`, `cardinality`). On document-backed drivers the engine
substitutes equivalent JSON or list operations. A few carry
backend-specific restrictions — see the per-database pages (for
example, [DynamoDB](./dynamodb.md), where `is_superset` and
`intersects` require a literal right-hand side).

## Updating

### Replacing the whole list

Passing a list to the field setter replaces the entire value:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Article {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     tags: Vec<String>,
#     scores: Vec<i64>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let mut article = toasty::create!(Article {
#     title: "Hello",
#     tags: ["rust"],
#     scores: [1],
# }).exec(&mut db).await?;
article.update()
    .tags(["x", "y", "z"])
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

`toasty::stmt::set(value)` is the explicit form of the same whole-value
replacement, useful when building an assignment programmatically.

### Incremental mutations

For changes relative to the stored value, the `toasty::stmt` module
provides builders. Each produces one update statement and refreshes the
in-memory field after `.exec()`:

| Function | What it does |
|---|---|
| `stmt::push(value)` | Append one element. |
| `stmt::extend(iter)` | Append every element of an iterator, in order. |
| `stmt::pop()` | Remove the last element. |
| `stmt::remove(value)` | Remove every element equal to the value. |
| `stmt::remove_at(idx)` | Remove the element at a 0-based index. |
| `stmt::clear()` | Replace the field with an empty list. |
| `stmt::apply([ops])` | Apply several of the above in order, in one statement. |

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Article {
#     #[key]
#     #[auto]
#     id: u64,
#     title: String,
#     tags: Vec<String>,
#     scores: Vec<i64>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let mut article = toasty::create!(Article {
#     title: "Hello",
#     tags: ["rust"],
#     scores: [1],
# }).exec(&mut db).await?;
// Append one element.
article.update()
    .tags(toasty::stmt::push("toasty"))
    .exec(&mut db)
    .await?;

// Append several. `stmt::extend` of an empty iterator is a no-op.
article.update()
    .tags(toasty::stmt::extend(["orm", "async"]))
    .exec(&mut db)
    .await?;

// Remove the last element.
article.update()
    .tags(toasty::stmt::pop())
    .exec(&mut db)
    .await?;

// Remove every element equal to "orm".
article.update()
    .tags(toasty::stmt::remove("orm"))
    .exec(&mut db)
    .await?;

// Remove the element at index 0.
article.update()
    .tags(toasty::stmt::remove_at(0usize))
    .exec(&mut db)
    .await?;

// Remove every element.
article.update()
    .tags(toasty::stmt::clear())
    .exec(&mut db)
    .await?;

// Combine operations into one statement, applied in order.
article.update()
    .tags(toasty::stmt::apply([
        toasty::stmt::push("rust"),
        toasty::stmt::push("toasty"),
    ]))
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

Each operation is atomic against the existing column value — the
database applies it to whatever the row currently holds, not to the
in-memory snapshot. Concurrent writers can still interleave between
operations, but no single operation reads then writes in a way another
writer can split.

`pop` on an empty list, `remove` of an absent value, and `remove_at`
past the end of the list are all no-ops rather than errors. `remove`
deletes every matching element, not just the first.

`stmt::apply` runs each operation in order, against the result of the
previous one. An `apply` is valid only where every operation it
contains is valid on the target backend.

After `.exec()`, the in-memory field reflects the new value.

## Driver support

Defining a `Vec<scalar>` field, creating and reading rows, and the
array query predicates work on every built-in driver. Whole-value
replacement and the appending builders — `set`, `push`, `extend`,
`clear` — also work everywhere.

The element-removal builders are narrower:

| Operation | PostgreSQL | MySQL | SQLite | DynamoDB |
|---|---|---|---|---|
| Define field, create, read | ✓ | ✓ | ✓ | ✓ |
| `contains`, `len`, `is_empty` | ✓ | ✓ | ✓ | ✓ |
| `is_superset`, `intersects` | ✓ | ✓ | ✓ | literal right-hand side only |
| Replace, `set`, `push`, `extend`, `clear` | ✓ | ✓ | ✓ | ✓ |
| `pop`, `remove`, `remove_at` | ✓ | — | — | — |

`pop`, `remove`, and `remove_at` currently require PostgreSQL, where
they lower to `array_remove` and array slicing. On the other drivers
they return an error. See the per-database pages for the storage and
operator details specific to each backend.
