# PostgreSQL

Toasty's PostgreSQL driver uses [`tokio-postgres`] under the hood. It
covers the full SQL feature set Toasty exercises — row locking, native
arrays, native temporal types, named enum types, and `ILIKE` — and
integrates with Toasty's connection pool for retry and recovery.

[`tokio-postgres`]: https://docs.rs/tokio-postgres

## Enabling the driver

Add the `postgresql` feature to Toasty in `Cargo.toml`:

```toml
[dependencies]
toasty = { version = "{{toasty_version}}", features = ["postgresql"] }
```

Then pass a `postgresql://` (or `postgres://`) URL to `Db::builder`:

```rust,ignore
let db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .connect("postgresql://user:pass@localhost:5432/mydb")
    .await?;
```

TLS support is on by default through the driver's `tls` feature, which
pulls in `rustls`. To build the driver without TLS, depend on
`toasty-driver-postgresql` directly with `default-features = false`.

## Connection URL options

Append query parameters to the URL to tune the connection:

| Parameter | Purpose |
|---|---|
| `application_name=<string>` | Reported to PostgreSQL as the connecting client. Appears in `pg_stat_activity` and the server log — useful for distinguishing services sharing a database. |
| `sslmode=<mode>` | TLS negotiation mode. See the table below. |
| `sslrootcert=<path>` | PEM file with root certificates to trust. |
| `sslcert=<path>` and `sslkey=<path>` | Client certificate and matching private key, for mutual TLS. |
| `channel_binding=<mode>` | `disable`, `prefer` (default), or `require`. |
| `sslnegotiation=<mode>` | `postgres` (default — SSL request over a plain socket) or `direct` (TLS from the first byte). |

```rust,ignore
.connect("postgresql://app:secret@db.internal/store\
          ?sslmode=verify-full&application_name=store-api")
```

### TLS modes

| `sslmode` | What it does |
|---|---|
| `disable` | Plain TCP. The driver does not negotiate TLS. |
| `prefer` *(default)* | Attempt TLS; fall back to plain if the server rejects it. The certificate is not verified. |
| `require` | Require TLS, but accept any certificate. |
| `verify-ca` | Require TLS and verify the certificate chains to a trusted root. |
| `verify-full` | `verify-ca` plus verify the certificate matches the server hostname. |

Without the driver's `tls` feature, any `sslmode` other than `disable`
fails at connect time.

## Type mapping

Toasty maps Rust types to PostgreSQL columns as follows. Most types
land in their native PostgreSQL form; the exceptions are called out
below the table.

| Rust type | PostgreSQL column type |
|---|---|
| `bool` | `BOOL` |
| `i8`, `i16` | `SMALLINT` (`INT2`) |
| `i32` | `INTEGER` (`INT4`) |
| `i64` | `BIGINT` (`INT8`) |
| `u8` | `SMALLINT` (`INT2`) |
| `u16` | `INTEGER` (`INT4`) |
| `u32`, `u64` | `BIGINT` (`INT8`) |
| `f32` | `REAL` (`FLOAT4`) |
| `f64` | `DOUBLE PRECISION` (`FLOAT8`) |
| `String` | `TEXT` by default; `VARCHAR(N)` with `#[column(type = varchar(N))]` |
| `Vec<u8>` | `BYTEA` |
| `uuid::Uuid` | `UUID` |
| `rust_decimal::Decimal` *(feature)* | `NUMERIC` |
| `jiff::Timestamp` *(feature)* | `TIMESTAMPTZ` with microsecond precision |
| `jiff::civil::Date` *(feature)* | `DATE` |
| `jiff::civil::Time` *(feature)* | `TIME` with microsecond precision |
| `jiff::civil::DateTime` *(feature)* | `TIMESTAMP` with microsecond precision |
| `Vec<T>` *(T scalar)* | Native array (`text[]`, `int8[]`, `uuid[]`, …) |
| Embedded `enum` | Native `ENUM` type (`CREATE TYPE ... AS ENUM`) |

### Notes on the type mapping

**Unsigned integers cap at `i64::MAX`.** PostgreSQL has no unsigned
integer types, so Toasty stores them in the next-wider signed type
(`u8` in `SMALLINT`, `u16` in `INTEGER`, `u32`/`u64` in `BIGINT`).
A `u64` value above `i64::MAX` (≈9.22 × 10¹⁸) rejects on insert.

**`jiff::Timestamp` vs `jiff::civil::DateTime`.** `Timestamp` is an
instant in time; it stores in `TIMESTAMPTZ` and round-trips as UTC.
`civil::DateTime` is a wall-clock value with no zone; it stores in
`TIMESTAMP` (without time zone). Pick the one that matches the
semantics you want.

**`jiff::Zoned` and `bigdecimal::BigDecimal` store as `TEXT`.**
PostgreSQL's `TIMESTAMPTZ` does not carry an IANA zone name (only an
instant), so a true zoned value round-trips through text. Likewise
`bigdecimal::BigDecimal` does not yet ride PostgreSQL's `NUMERIC` wire
encoding and falls back to text. Use `rust_decimal::Decimal` if you
want native `NUMERIC`.

**`VARCHAR` length cap.** PostgreSQL's `VARCHAR(N)` allows `N` up to
10,485,760. Toasty rejects larger values at schema-build time.

## Behavior specific to PostgreSQL

Toasty enables these features automatically when the driver is
PostgreSQL. No configuration is required.

**Native arrays for [`Vec<scalar>` fields](./field-options.md#scalar-arrays).**
A `tags: Vec<String>` field is a `text[]` column. The array predicates
(`contains`, `is_superset`, `intersects`, `len`, `is_empty`) lower to
PostgreSQL's native operators (`= ANY(col)`, `@>`, `&&`,
`cardinality(col)`):

```rust,ignore
let admins = User::filter(User::fields().roles().contains("admin"))
    .exec(&mut db)
    .await?;
```

**Native `ILIKE`.** The [`.ilike()`](./filtering-with-expressions.md#ilike)
filter method lowers directly to the SQL `ILIKE` operator. On other
SQL drivers `.ilike()` falls back to plain `LIKE`, which is
case-sensitive on PostgreSQL — so the behavior actually differs
between backends. Reach for `.ilike()` when you need case-insensitive
matching here.

**Native prefix match.** The
[`.starts_with()`](./filtering-with-expressions.md#starts_with) filter
lowers to PostgreSQL's `^@` operator. The optimizer can use a
`text_pattern_ops` index for `^@` queries the same way it would for an
anchored `LIKE 'prefix%'`.

**Named enum types.** An [`embed`-tagged Rust enum](./embedded-types.md)
maps to a real PostgreSQL enum type created with `CREATE TYPE ... AS
ENUM`. Adding a new variant requires an `ALTER TYPE ... ADD VALUE`,
which the migration generator handles.

**Row-level locking.** Generated [transactions](./transactions.md) can
use `SELECT ... FOR UPDATE` to lock rows for the duration of a
transaction. SQLite and DynamoDB do not have row-level locking;
Toasty's transaction layer falls back to serializable transaction
isolation on those backends.

**Backward pagination.**
[`.paginate(per_page).prev(&db)`](./sorting-limits-and-pagination.md#navigating-pages)
walks backwards from a page cursor. DynamoDB cannot do this;
PostgreSQL (like the other SQL backends) can.

## Migrations

The migration generator emits DDL that PostgreSQL can apply inside a
single transaction, and `apply_migration` wraps each migration in
`BEGIN` / `COMMIT`. A failure rolls back cleanly.

Two PostgreSQL-specific behaviors worth knowing:

**Enum types come first.** Each migration emits `CREATE TYPE` or
`ALTER TYPE` statements for enum types before any `CREATE TABLE` /
`ALTER TABLE` that references them.

**Column changes are not always atomic.** PostgreSQL can `ALTER TABLE
... ALTER COLUMN` to change a column's type, but Toasty emits each
property change (type, name, NOT NULL, default) as a separate
statement. Inside a single migration this is invisible — they all
commit together — but at the SQL level the change is several
statements, not one.

The migration tooling does not yet manage zero-downtime online
migrations on PostgreSQL (concurrent index creation, dual-write
schemes, etc.); migrations assume exclusive access to the schema for
their duration.

## Errors and the connection pool

The driver classifies `tokio-postgres` errors into Toasty's typed
error variants so the pool and caller can react sensibly.

| SQLSTATE or condition | Toasty error |
|---|---|
| `40001` *(serialization_failure)* | `Error::SerializationFailure` — retryable. The transaction lost an optimistic conflict and should be retried by the caller. |
| `25006` *(read_only_sql_transaction)* | `Error::ReadOnlyTransaction` — the connection is read-only. |
| Other server errors with a SQLSTATE | `Error::DriverOperationFailed` |
| Socket / protocol errors (closed connection, broken pipe, end-of-stream during handshake) | `Error::ConnectionLost` |

A `ConnectionLost` error tells the pool to evict the failed
connection. The next acquire pings idle connections, drops the ones
that fail, and opens a fresh slot if needed — so a backend restart
typically costs one failed user query rather than one per pooled
connection. See [Database Setup](./database-setup.md#connection-pool)
for the pool knobs (`max_pool_size`, `pool_pre_ping`,
`pool_health_check_interval`, …) and what they do.

The driver caches prepared statements and enum type OIDs per
connection. Both caches are invalidated automatically when a
connection is dropped, so they do not cause stale-state issues after
an eviction.
