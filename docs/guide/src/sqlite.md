# SQLite

Toasty's SQLite driver uses [`rusqlite`] under the hood. It runs the
full Toasty query surface — `SELECT`, `INSERT`, `UPDATE`, `DELETE`,
`RETURNING`, transactions, scalar arrays via JSON1 — against either a
file-backed database or an ephemeral in-memory one.

[`rusqlite`]: https://docs.rs/rusqlite

## Enabling the driver

Add the `sqlite` feature to Toasty in `Cargo.toml`:

```toml
[dependencies]
toasty = { version = "{{toasty_version}}", features = ["sqlite"] }
```

Then pass a `sqlite:` URL to `Db::builder`:

```rust,ignore
let db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .connect("sqlite::memory:")
    .await?;
```

For a file-backed database, point at a path on disk:

```rust,ignore
let db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .connect("sqlite:./app.db")
    .await?;
```

You can also construct the driver yourself and pass it to `build()`
instead of `connect()` — useful in tests that want an in-memory database
without parsing a URL:

```rust,ignore
let driver = toasty_driver_sqlite::Sqlite::in_memory();
let db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .build(driver)
    .await?;
```

## Connection URL options

The driver recognizes two URL forms:

| URL | Meaning |
|---|---|
| `sqlite::memory:` | An in-memory database. Each connection opens a fresh database — see the section on in-memory databases below. |
| `sqlite:<path>` | A file-backed database at `<path>`. Relative paths resolve against the process's working directory. |

The driver does not parse query parameters from the URL. To set
SQLite pragmas (`journal_mode`, `synchronous`, `foreign_keys`, …),
construct the driver directly and issue the pragmas through your own
connection-setup code, or open the database with `Sqlite::open` and
work from there.

## Type mapping

Toasty maps Rust types to SQLite columns as follows. SQLite uses type
affinity rather than strict types, so several Rust types share an
underlying SQLite storage class.

| Rust type | SQLite column type |
|---|---|
| `bool` | `BOOLEAN` (stored as `INTEGER` 0/1) |
| `i8`, `i16` | `SMALLINT` |
| `i32` | `INTEGER` |
| `i64` | `BIGINT` |
| `u8`, `u16`, `u32`, `u64` | `INTEGER` |
| `f32`, `f64` | `REAL` |
| `String` | `TEXT` by default; `VARCHAR(N)` with `#[column(type = varchar(N))]` |
| `Vec<u8>` | `BLOB` |
| `uuid::Uuid` | `BLOB` |
| `rust_decimal::Decimal` *(feature)* | `TEXT` |
| `bigdecimal::BigDecimal` *(feature)* | `TEXT` |
| `jiff::Timestamp` *(feature)* | `TEXT` (ISO 8601) |
| `jiff::Zoned` *(feature)* | `TEXT` (ISO 8601) |
| `jiff::civil::Date` *(feature)* | `TEXT` (ISO 8601) |
| `jiff::civil::Time` *(feature)* | `TEXT` (ISO 8601) |
| `jiff::civil::DateTime` *(feature)* | `TEXT` (ISO 8601) |
| `Vec<T>` *(T scalar)* | `TEXT` holding a JSON array |
| Embedded `enum` | `TEXT` with a `CHECK` constraint over the variant names |

### Notes on the type mapping

**UUIDs are stored as `BLOB`.** SQLite has no native UUID type. The
driver writes the 16-byte representation and reads it back through the
`Uuid` parser. Pick `#[column(type = text)]` on the field if you'd
rather store the hyphenated string form — at the cost of more bytes
per row and a slower comparison.

**Temporal types are ISO 8601 text.** Every `jiff` type lands in a
`TEXT` column. Values round-trip losslessly, but range and ordering
queries compare strings rather than packed timestamps. Index the
column if you query by date frequently; the lexicographic order of
ISO 8601 matches chronological order.

**Decimals are stored as `TEXT`.** SQLite has no native fixed-point
type. Both `rust_decimal::Decimal` and `bigdecimal::BigDecimal`
round-trip through text. Arithmetic in SQL coerces to `REAL`, which
loses precision — keep decimal math in Rust.

**`VARCHAR(N)` does not enforce `N`.** SQLite ignores the length
specifier on `VARCHAR`, `CHAR`, and `TEXT`-affinity types. A field
declared `#[column(type = varchar(10))]` accepts strings of any
length; the only hard cap is SQLite's `SQLITE_MAX_LENGTH`, which is
one billion by default. Validate lengths in your application code if
you need the limit enforced.

**Unsigned integers cap at `i64::MAX`.** SQLite's `INTEGER` is a
signed 64-bit value. `u8`, `u16`, and `u32` round-trip without
trouble; a `u64` value above `i64::MAX` (≈9.22 × 10¹⁸) rejects on
insert.

**[Embedded enums](./embedded-types.md) become `TEXT` plus a `CHECK`.**
SQLite has no `ENUM` type. Each variant stores as its name, and the
column carries a `CHECK` constraint listing the allowed values.

## Behavior specific to SQLite

Most Toasty features work the same on SQLite as on PostgreSQL or
MySQL — filters, joins implemented via subqueries, `RETURNING` on
mutations, [batch operations](./batch-operations.md),
[pagination in both directions](./sorting-limits-and-pagination.md),
[embedded types](./embedded-types.md),
[`#[unique]`](./indexes-and-unique-constraints.md#unique-fields),
[association preloading](./preloading-associations.md), and
serializable [transactions](./transactions.md) all run natively. A few
behaviors differ from the other SQL backends:

**`LIKE` is case-insensitive for ASCII.** SQLite's default `LIKE`
ignores case for ASCII characters. The
[`.ilike()`](./filtering-with-expressions.md#ilike) filter lowers to
the same `LIKE`, so case-insensitive matching works — but a
`.like("Rust")` filter also matches `"rust"` and `"RUST"`. Use
`GLOB` (which Toasty does not currently expose) or a `CHECK` against
exact bytes if you need case-sensitive pattern matching.

**No native prefix-match operator.**
[`.starts_with("abc")`](./filtering-with-expressions.md#starts_with)
lowers to `LIKE 'abc%'`. The optimizer can use a regular index for the
common-prefix lookup.

**Scalar arrays use JSON1.** A
[`Vec<T>` field](./field-options.md#scalar-arrays) lives in a `TEXT`
column holding a JSON array. The array predicates lower to JSON1
expressions:

| Method | SQLite expression |
|---|---|
| `.contains(value)` | `value IN (SELECT value FROM json_each(col))` |
| `.is_superset(values)` | `NOT EXISTS (SELECT 1 FROM json_each(rhs) AS r WHERE r.value NOT IN (SELECT value FROM json_each(col)))` |
| `.intersects(values)` | `EXISTS (SELECT 1 FROM json_each(rhs) AS r WHERE r.value IN (SELECT value FROM json_each(col)))` |
| `.len()` | `json_array_length(col)` |

These subqueries scan the JSON document, so array predicates against
a large table do not use an index. See
[Field Options](./field-options.md#scalar-arrays) for the model-level
view.

**No row-level locking.** SQLite has no `SELECT ... FOR UPDATE`.
Toasty's [transaction](./transactions.md) layer falls back to
serializable isolation — which is SQLite's only isolation level — so
the guarantees are still sound; you just can't pin individual rows for
the duration of a transaction.

**Only `Serializable` isolation.** Starting a transaction with any
other isolation level returns `Error::UnsupportedFeature`. The
default (no explicit level) is accepted and runs as serializable.

```rust,ignore
let users = User::filter(User::fields().email().ilike("%@example.com"))
    .exec(&mut db)
    .await?;
```

## In-memory databases

The `sqlite::memory:` URL opens an ephemeral database that lives only
as long as the connection. Two practical consequences fall out of
that:

**The pool caps at one connection.** The driver reports
`max_connections() = Some(1)` for in-memory mode, so the pool will
never open a second connection — opening one would land in a
different, empty database. Concurrent queries serialize on that one
slot. For tests this is usually fine; for anything else, use a
file-backed database.

**`reset_db` is a no-op.** Each connect produces a fresh in-memory
database, so there is nothing to clear between runs.

In-memory mode is the standard choice for unit tests, embedded
examples, and the rustdoc examples throughout this guide.

## Migrations

`apply_migration` wraps each migration in `BEGIN` / `COMMIT`; a
statement failure rolls the migration back. The migration generator
emits SQLite-compatible DDL, with one important caveat:

**SQLite cannot `ALTER COLUMN` a type.** Changing a column's type,
nullability, or auto-increment status requires rebuilding the table.
The migration generator handles this automatically with the standard
six-step rebuild:

1. `PRAGMA foreign_keys = OFF`
2. `CREATE TABLE _toasty_new_<name>` with the target schema
3. `INSERT INTO _toasty_new_<name> SELECT ... FROM <name>` (renames
   are tracked, so the column mapping uses the old names on the
   source side)
4. `DROP TABLE <name>`
5. `ALTER TABLE _toasty_new_<name> RENAME TO <name>`
6. `PRAGMA foreign_keys = ON`

The rebuild copies every row, so a column type change on a large
table is proportional to the table size. Renaming a column on its
own goes through `ALTER TABLE ... RENAME COLUMN` and does not
trigger a rebuild. Adding or dropping a column uses the corresponding
`ALTER TABLE` form when no type-changing alterations are present in
the same diff.

The migration tooling does not yet manage zero-downtime online
migrations.

## Errors and the connection pool

The driver does not classify errors into Toasty's typed retry
variants. Every `rusqlite` failure surfaces as
`Error::DriverOperationFailed`, with two specific exceptions:

| Condition | Toasty error |
|---|---|
| URL with a non-`sqlite` scheme | `Error::InvalidConnectionUrl` |
| Transaction started with an isolation level other than `Serializable` | `Error::UnsupportedFeature` |

SQLite has no out-of-process backend to lose, so there are no
`ConnectionLost` errors and no health-check pings to perform. The
driver's `is_valid` and `ping` implementations are the defaults — a
constant `true` and a no-op — and the pool's background sweep does
no work against a SQLite connection.

The pool sizing knobs from
[Database Setup](./database-setup.md#connection-pool) still apply for
file-backed databases. In-memory mode pins the pool to a single
connection regardless of `max_pool_size`.
