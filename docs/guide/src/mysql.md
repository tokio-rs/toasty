# MySQL

Toasty's MySQL driver uses [`mysql_async`] under the hood. It covers
the SQL feature set Toasty exercises — row locking, native temporal
types, inline enum columns, full unsigned 64-bit integers, and both
fixed-precision and arbitrary-precision decimals — and integrates
with Toasty's connection pool for retry and recovery.

[`mysql_async`]: https://docs.rs/mysql_async

## Enabling the driver

Add the `mysql` feature to Toasty in `Cargo.toml`:

```toml
[dependencies]
toasty = { version = "{{toasty_version}}", features = ["mysql"] }
```

Then pass a `mysql://` URL to `Db::builder`:

```rust,ignore
let db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .connect("mysql://user:pass@localhost:3306/mydb")
    .await?;
```

The URL must include a database name in the path; `mysql_async`
refuses URLs without one. TLS uses `native-tls` and is built in.

## Connection URL options

`mysql_async` parses query parameters out of the URL. The ones worth
knowing about for a typical service:

| Parameter | Purpose |
|---|---|
| `require_ssl=<bool>` | Require TLS. Without it, the driver connects in plaintext. |
| `verify_ca=<bool>` | When `true` (the default once TLS is on), verify the server certificate chains to a trusted root. Set `false` to accept any certificate. |
| `verify_identity=<bool>` | When `true` (default), verify the certificate matches the server hostname. Set `false` to skip hostname validation. |
| `built_in_roots=<bool>` | When `false`, do not trust the system root store — useful when pinning a private CA. |
| `socket=<path>` | Connect over a Unix socket instead of TCP. |
| `prefer_socket=<bool>` | When connecting to `localhost`, try a Unix socket first and fall back to TCP. |
| `compression=<level>` | Enable wire-protocol compression. Accepts `fast`, `on`, `best`, or a digit 0–9. |
| `tcp_keepalive=<ms>` | TCP keepalive interval in milliseconds. |
| `tcp_nodelay=<bool>` | Disable Nagle's algorithm on the TCP socket. |
| `max_allowed_packet=<bytes>` | Cap the client-side max packet size, clamped to 1024–1073741824. |
| `wait_timeout=<secs>` | Server-side `wait_timeout` for the session. |
| `stmt_cache_size=<n>` | Per-connection prepared-statement cache size. Defaults to 32; set to `0` to disable caching. |

Client certificates for mutual TLS are not exposed through URL
parameters; build them programmatically with `mysql_async`'s
`SslOpts::with_client_identity` and pass a constructed driver to
`Db::builder().build(driver)` (see
[Database Setup](./database-setup.md#using-a-driver-directly)).

```rust,ignore
.connect("mysql://app:secret@db.internal/store\
          ?require_ssl=true&verify_identity=true&compression=on")
```

## Type mapping

Toasty maps Rust types to MySQL columns as follows. MySQL has a few
quirks worth knowing about; the notes below the table call them out.

| Rust type | MySQL column type |
|---|---|
| `bool` | `BOOLEAN` (a `TINYINT(1)` alias) |
| `i8` | `TINYINT` |
| `i16` | `SMALLINT` |
| `i32` | `INTEGER` |
| `i64` | `BIGINT` |
| `u8` | `TINYINT UNSIGNED` |
| `u16` | `SMALLINT UNSIGNED` |
| `u32` | `INTEGER UNSIGNED` |
| `u64` | `BIGINT UNSIGNED` |
| `f32` | `FLOAT` |
| `f64` | `DOUBLE` |
| `String` | `VARCHAR(191)` by default; override with `#[column(type = varchar(N))]` |
| `Vec<u8>` | `BLOB` |
| `uuid::Uuid` | `VARCHAR(36)` |
| `rust_decimal::Decimal` *(feature)* | `DECIMAL(p, s)` — precision and scale required |
| `bigdecimal::BigDecimal` *(feature)* | `DECIMAL(p, s)` — precision and scale required |
| `jiff::Timestamp` *(feature)* | `DATETIME(6)`, stored in UTC |
| `jiff::civil::Date` *(feature)* | `DATE` |
| `jiff::civil::Time` *(feature)* | `TIME(6)` |
| `jiff::civil::DateTime` *(feature)* | `DATETIME(6)` |
| `Vec<T>` *(T scalar)* | `JSON` |
| Embedded `enum` | Inline `ENUM('a', 'b', ...)` column |

### Notes on the type mapping

**`VARCHAR(191)` is the default string type.** MySQL's row format
caps the total row size at 65,535 bytes, and `utf8mb4` consumes up to
four bytes per character. An indexed `VARCHAR` column has a per-index
prefix limit of 767 bytes on older InnoDB row formats — 191
characters fits inside that limit with `utf8mb4`, so the default lets
you index a string field without configuring anything. Override with
`#[column(type = varchar(N))]` for `N` up to 65,535 (see
[Field Options](./field-options.md#explicit-column-types)). The
schema builder rejects larger values.

**Full unsigned 64-bit range.** MySQL has native unsigned integer
types, so `u64` rides `BIGINT UNSIGNED` and can hold the full
0..=2⁶⁴−1 range. This is the only Toasty backend where `u64` is not
capped at `i64::MAX`.

**UUIDs go in `VARCHAR(36)`.** MySQL has no native UUID type. Toasty
stores UUIDs as their hyphenated text form. A 16-byte `BINARY(16)`
column would pack tighter, but `VARCHAR(36)` is easier to inspect from
a SQL prompt.

**`jiff::Timestamp` maps to `DATETIME(6)`, not `TIMESTAMP`.** MySQL's
`TIMESTAMP` only spans 1970-01-01 to 2038-01-19. Toasty uses
`DATETIME(6)` instead and converts to and from UTC at the driver
layer, so values round-trip as UTC instants without being bound to
the 2038 cutoff.

**`Decimal` requires fixed precision and scale.** Unlike PostgreSQL's
`NUMERIC`, MySQL's `DECIMAL` always has a declared precision and
scale; there is no arbitrary-precision mode. Set them with
`#[column(type = decimal(p, s))]` when declaring the field.

**`BigDecimal` works natively on MySQL.** Toasty rides `DECIMAL(p,
s)` for `bigdecimal::BigDecimal` here. PostgreSQL falls back to text
for `BigDecimal`; MySQL is currently the only backend that exchanges
it as a native decimal value over the wire.

**[`Vec<scalar>`](./field-options.md#scalar-arrays) goes in a `JSON`
column.** Toasty serializes the list to a JSON array at bind time and
parses it back on read. Array predicates (`contains`, `is_superset`,
`intersects`, `len`, `is_empty`) lower to MySQL's `JSON_CONTAINS`,
`JSON_LENGTH`, and related functions.

**`jiff::Zoned` stores as `TEXT`.** MySQL has no column type that
carries an IANA zone name alongside an instant, so zoned values
round-trip through text.

## Behavior specific to MySQL

Toasty enables these features automatically when the driver is MySQL.
No configuration is required.

**Full unsigned 64-bit integers.** Values up to `u64::MAX` round-trip
through `BIGINT UNSIGNED` without truncation. SQLite and PostgreSQL
cap unsigned types at `i64::MAX`.

**Inline enum columns.** An [`embed`-tagged Rust enum](./embedded-types.md)
maps to a column declared `ENUM('variant_a', 'variant_b', ...)`. There
is no separate named type to maintain; adding a variant emits an
`ALTER TABLE ... MODIFY COLUMN` against the same column.

**Both `Decimal` and `BigDecimal` are native.** Enable the
`rust_decimal` feature for `rust_decimal::Decimal`, the `bigdecimal`
feature for `bigdecimal::BigDecimal`, or both. Each maps to
`DECIMAL(p, s)` with declared precision and scale.

**Row-level locking.** Generated [transactions](./transactions.md) can
use `SELECT ... FOR UPDATE` to lock rows for the duration of a
transaction.

**Backward pagination.**
[`.paginate(per_page).prev(&db)`](./sorting-limits-and-pagination.md#navigating-pages)
walks backwards from a page cursor.

A few things that exist on PostgreSQL are absent here:

**No `ILIKE`.** MySQL's [`.ilike()`](./filtering-with-expressions.md#ilike)
filter lowers to plain `LIKE`, whose case sensitivity depends on the
collation of the column.
`utf8mb4_unicode_ci` and similar `_ci` collations match
case-insensitively for free; binary or `_bin` collations match
case-sensitively. Pick the collation that matches the semantics you
want when declaring the column.

**No `RETURNING` from INSERT or UPDATE.** MySQL does not support
`RETURNING` clauses on mutations. For inserts into a table with an
auto-increment primary key, Toasty fetches the generated ID with
`LAST_INSERT_ID()` on the same connection and synthesizes the same
result a `RETURNING` clause would produce. This is transparent at the
API level — [`Model::create()`](./creating-records.md) returns a
populated model the same way it does on other backends. The constraint to know about: if you wire
up a `RETURNING`-style read through a non-auto-increment column,
Toasty will reject the query rather than silently issue a second
round-trip.

**No CTE-driven updates.** MySQL does not allow `UPDATE` inside a
common table expression. The query planner avoids generating those.
Most update patterns do not need them; the engine routes complex
multi-statement updates differently on this backend.

## Migrations

The migration generator emits DDL that MySQL can apply inside a
single transaction, and `apply_migration` wraps each migration in
`START TRANSACTION` / `COMMIT`. A failure rolls back the migration
and the bookkeeping row in `__toasty_migrations` together.

Two MySQL-specific behaviors worth knowing:

**Column changes are atomic.** MySQL's `ALTER TABLE ... MODIFY
COLUMN` rewrites name, type, nullability, and default in a single
statement. The migration generator takes advantage of this — a
property change emits one statement, not several. PostgreSQL needs
one statement per property; MySQL does not.

**Enum types live inline on the column.** Adding a variant emits an
`ALTER TABLE ... MODIFY COLUMN` against the column whose type is the
enum. There is no separate `CREATE TYPE` step.

The migration tooling does not yet manage zero-downtime online
migrations on MySQL (`pt-online-schema-change`-style copy and swap,
gh-ost integration, etc.); migrations assume exclusive access to the
schema for their duration.

## Errors and the connection pool

The driver classifies `mysql_async` errors into Toasty's typed error
variants so the pool and caller can react sensibly.

| MySQL error or condition | Toasty error |
|---|---|
| Error `1213` *(`ER_LOCK_DEADLOCK`)* | `Error::SerializationFailure` — retryable. InnoDB rolled back the transaction to break a deadlock; retry the unit of work. |
| Error `1792` *(`ER_CANT_EXECUTE_IN_READ_ONLY_TRANSACTION`)* | `Error::ReadOnlyTransaction` — the connection is read-only. |
| Other server errors with a SQLSTATE | `Error::DriverOperationFailed` |
| Socket / protocol errors (closed connection, pool disconnected) | `Error::ConnectionLost` |

A `ConnectionLost` error tells the pool to evict the failed
connection and flips the connection's internal validity flag so the
pool does not hand it back out. The next acquire pings idle
connections, drops the ones that fail, and opens a fresh slot if
needed — so a backend restart typically costs one failed user query
rather than one per pooled connection. See
[Database Setup](./database-setup.md#connection-pool) for the pool
knobs (`max_pool_size`, `pool_pre_ping`,
`pool_health_check_interval`, …) and what they do.

`mysql_async` caches prepared statements per connection (up to 32 by
default, tunable via the `stmt_cache_size` URL parameter). The cache
is bound to the connection and is dropped when the connection is
evicted, so it does not cause stale-state issues after a backend
restart.
