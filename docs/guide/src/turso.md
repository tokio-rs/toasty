# Turso

[Turso] is a SQLite-compatible database engine with native async I/O.
Toasty's Turso driver speaks the same SQL dialect as the
[SQLite driver](./sqlite.md) and supports the same Rust types and
queries. The differences from SQLite cover four areas: connection
pooling against an in-memory database, an opt-in concurrent-writes
mode that uses Turso's MVCC journal, toggles for Turso's experimental
engine features, and optional remote sync (Turso Cloud or
`tursodb --sync-server`).

[Turso]: https://turso.tech/

## Enabling the driver

Add the `turso` feature to Toasty in `Cargo.toml`:

```toml
[dependencies]
toasty = { version = "{{toasty_version}}", features = ["turso"] }
```

Then pass a `turso:` URL to `Db::builder`:

```rust,ignore
let db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .connect("turso::memory:")
    .await?;
```

A file-backed database uses the same URL scheme with a path:

```rust,ignore
let db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .connect("turso:./app.db")
    .await?;
```

You can also construct the driver directly and pass it to `build()`
instead of `connect()`. This is the form to use when you want to set
Turso-specific options that don't fit in a URL:

```rust,ignore
let driver = toasty_driver_turso::Turso::in_memory().concurrent_writes();
let db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .build(driver)
    .await?;
```

## Connection URL options

| URL | Meaning |
|---|---|
| `turso::memory:` | An in-memory database, shared across every connection in the pool (see below). |
| `turso:<path>` | A file-backed database at `<path>`. Relative paths resolve against the process's working directory. |

The driver does not parse query parameters from the URL. To set
options like `concurrent_writes()`, remote sync, or any of the
`experimental_*` flags, construct the driver directly.

## Type mapping and SQL behavior

The Turso driver uses the same type mapping and SQL serializer as the
[SQLite driver](./sqlite.md) — see that chapter for the column-type
table and notes on UUIDs as `BLOB`, ISO 8601 temporal types, decimals
stored as `TEXT`, and targeted [`upsert_by_*`](./upserting-records.md).
The list of Turso-specific behaviors below is everything that differs from the
SQLite chapter.

## In-memory databases share state across the pool

Unlike the SQLite driver — which caps the pool at one connection
because each `Connection::open(":memory:")` returns a private,
disjoint database — the Turso driver caches a `turso::Database` at
construction time and hands every pool slot a connection backed by
the same database. An in-memory Turso database supports the full
`max_pool_size` like a file-backed one: writes from one connection
are immediately visible to readers on another.

This means the connection-pool tests in Toasty's integration suite
run against in-memory Turso, and you can use `turso::memory:` for
multi-connection tests where in-memory SQLite would not work.

## Remote sync

Remote sync keeps a local Turso database in sync with Turso Cloud or a
local `tursodb --sync-server`.

Sync lives on `toasty-driver-turso`, not on `Db`: enable the feature,
build the driver with a remote URL, pass it to `build()`, and call
`push()` / `pull()` when you want to sync.

Add the driver's `sync` feature in `Cargo.toml`. Keep Toasty's `turso`
feature as well if you connect through `toasty::Db`:

```toml
[dependencies]
toasty = { version = "{{toasty_version}}", features = ["turso"] }
toasty-driver-turso = { version = "{{toasty_version}}", features = ["sync"] }
```

Construct the driver directly — sync options do not fit in a `turso:`
URL — then pass it to `build()`:

```rust,ignore
use toasty_driver_turso::Turso;

let driver = Turso::file("./app.db")
    .with_remote_url("https://your-db.turso.io")
    .with_auth_token(std::env::var("TURSO_AUTH_TOKEN")?);

let db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .build(driver.clone())
    .await?;

// Pull remote changes into the local file, then push local writes back.
driver.pull().await?;
// … use `db` as usual …
driver.push().await?;
```

Local development against `tursodb --sync-server` omits the token:

```rust,ignore
// Started with: tursodb :memory: --sync-server 127.0.0.1:8080
let driver = Turso::file("./app.db")
    .with_remote_url("http://127.0.0.1:8080");
```

`with_remote_url` accepts `https://`, `http://` and `libsql://`
(`libsql://` becomes `https://`). On a file-backed database that was
synced before, omitting the URL loads it from on-disk metadata
(`./app.db-info` and related files).

### Authentication

`with_auth_token` sets a static auth token. Pass the token **without**
a `Bearer ` prefix — the driver adds that for you.

| Environment | Token |
|---|---|
| Local `tursodb --sync-server` (no auth) | Omit `with_auth_token` |
| Turso Cloud | Database token from the Turso dashboard or `turso db tokens create` |

For rotating tokens (secrets manager, OAuth refresh), use
`with_auth_token_fn`. The callback runs before every HTTP request; if
it fails, that sync call fails.

### Bootstrap

When the local database is empty, Turso downloads schema and data from
the remote on first open. That is Turso's default. Call
`bootstrap_if_empty(false)` to skip it — for example when opening an
existing local file that should not be overwritten.

### Sync operations

`push`, `pull`, `checkpoint`, and `stats` are methods on the `Turso`
driver, not on `Db`. They use the driver's shared database handle, so
every connection in the pool sees the same pending changes.

| Method | Behavior |
|---|---|
| `push()` | Send local changes to the remote |
| `pull()` | Fetch remote changes and apply them locally; returns `true` if anything changed |
| `checkpoint()` | Checkpoint the local WAL |
| `stats()` | Return sync stats (`DatabaseSyncStats`) |

Call `push()` and `pull()` yourself — sync is not automatic.

### Sync builder options

| Builder method | Purpose |
|---|---|
| `with_remote_url(url)` | Remote URL |
| `with_auth_token(token)` | Static auth token (no `Bearer ` prefix) |
| `with_auth_token_fn(f)` | Callback that returns a token for each request |
| `with_client_name(name)` | Client name (default `turso-sync-rust`) |
| `with_long_poll_timeout(duration)` | How long to wait when polling for remote changes |
| `bootstrap_if_empty(bool)` | Download schema and data when the local database is empty (default `true`) |
| `with_remote_encryption(key, cipher)` | Base64 key and cipher for an encrypted remote |
| `with_remote_encryption_key(key)` | Base64 key only (prefer `with_remote_encryption` when bootstrapping) |
| `experimental_with_partial_sync_opts(opts)` | Experimental partial sync (`PartialSyncOpts`) |
| `experimental_index_method(true)` | Experimental index methods (works with or without sync) |

`PartialSyncOpts`, `RemoteEncryptionCipher`, and `DatabaseSyncStats`
come from `turso::sync` and are re-exported by `toasty_driver_turso`.

## Concurrent writes

A classic SQLite database serializes all writers on a single
write lock: a second writer's `BEGIN` succeeds but its first write
waits for the first writer to commit. Turso also supports an MVCC
journal mode in which multiple writers can run concurrently and
conflicting commits surface at write or commit time — `BEGIN
CONCURRENT`. Toasty exposes this through the driver builder:

```rust,ignore
let driver = toasty_driver_turso::Turso::file("app.db").concurrent_writes();
```

When `concurrent_writes()` is enabled, the driver issues
`PRAGMA journal_mode = 'mvcc'` on each new connection and a
transaction started with `TransactionMode::Default` — the form you
get from `db.transaction()` — uses `BEGIN CONCURRENT`. A write-write
conflict surfaces as `Error::SerializationFailure`, the same retry
signal PostgreSQL emits for `40001` and MySQL for `1213`:

```rust,ignore
let mut tx = db.transaction().await?;
Counter::filter_by_id(1).update().tally(7).exec(&mut tx).await?;
match tx.commit().await {
    Ok(()) => {}
    Err(err) if err.is_serialization_failure() => {
        // Another writer touched the same row — retry.
    }
    Err(err) => return Err(err),
}
```

### Overriding the default per-transaction

The other [`TransactionMode`](./transactions.md#lock-acquisition-modes)
variants are an opt-out from MVCC: a transaction can request classic
locking even when the driver is configured with `concurrent_writes()`.

```rust,ignore
use toasty_core::driver::operation::TransactionMode;

// Plain BEGIN, classic deferred locking, no MVCC concurrency.
let mut tx = db
    .transaction_builder()
    .mode(TransactionMode::Deferred)
    .begin()
    .await?;

// BEGIN IMMEDIATE — take the write lock at begin time so later writes
// don't fail with BUSY. A second IMMEDIATE transaction on a separate
// connection will fail at its own BEGIN.
let mut tx = db
    .transaction_builder()
    .mode(TransactionMode::Immediate)
    .begin()
    .await?;

// BEGIN EXCLUSIVE — reserve the database. Another writer's BEGIN
// EXCLUSIVE fails until this transaction commits.
let mut tx = db
    .transaction_builder()
    .mode(TransactionMode::Exclusive)
    .begin()
    .await?;
```

Without `concurrent_writes()`, all four `TransactionMode` variants
behave the same as on the SQLite driver — `Default` and `Deferred`
both emit `BEGIN`.

## Experimental features

Turso ships several engine features behind opt-in flags. The driver
mirrors each one as a builder method on `Turso`:

```rust,ignore
use toasty_driver_turso::Turso;

let driver = Turso::file("app.db")
    .experimental_attach(true)
    .experimental_materialized_views(true)
    .experimental_vacuum(true);
```

| Builder method | Turso feature |
|---|---|
| `experimental_encryption(opts)` | At-rest page encryption with a cipher + key |
| `experimental_attach(true)` | `ATTACH DATABASE` |
| `experimental_custom_types(true)` | User-defined types |
| `experimental_generated_columns(true)` | `GENERATED ALWAYS AS` columns |
| `experimental_index_method(true)` | Alternative index methods |
| `experimental_materialized_views(true)` | Materialized views |
| `experimental_vacuum(true)` | `VACUUM` |
| `experimental_multiprocess_wal(true)` | Multi-process WAL access |
| `experimental_without_rowid(true)` | `WITHOUT ROWID` tables |

These are passthrough toggles: Toasty does not validate or use the
features itself. Turso enforces and implements them. As Turso adds or
stabilizes features, the toggles may change name or move out of the
`experimental_*` family — track the [turso] release notes.

### Encryption

Encryption is the one toggle that needs configuration beyond a bool.
Construct an `EncryptionOpts` with a cipher name and a hex-encoded
key:

```rust,ignore
use toasty_driver_turso::{EncryptionOpts, Turso};

let driver = Turso::file("encrypted.db")
    .experimental_encryption(EncryptionOpts {
        cipher: "aes256gcm".into(),
        hexkey: "<64-hex-character-key>".into(),
    });
```

Supplying `EncryptionOpts` enables encryption — the driver bundles
the two upstream calls (`experimental_encryption(true)` and
`with_encryption(opts)`) so you can't enable encryption without
supplying a key. Cipher names Turso accepts include `aes128gcm`,
`aes256gcm`, and the AEGIS family; see Turso's engine documentation
for the current list. Key management — storage, rotation,
provisioning — is the caller's responsibility.

## Errors and the connection pool

The driver classifies a few specific Turso error variants into
Toasty's typed retry variants; everything else surfaces as
`Error::DriverOperationFailed`:

| Turso error | Toasty error |
|---|---|
| `Busy`, `BusySnapshot` | `Error::SerializationFailure` |
| `Error(msg)` containing `"conflict"` | `Error::SerializationFailure` |
| `Readonly` | `Error::ReadOnlyTransaction` |
| `IoError` | `Error::ConnectionLost` |

The `"conflict"` substring check is a workaround for the upstream
Turso 0.6 API: write-write conflicts under MVCC sometimes surface as
the generic `Error` variant rather than as `Busy*`. The check matches
the same heuristic Turso's own
[`examples/concurrent_writes.rs`][example] uses; expect it to go away
once the upstream API gives every retryable conflict a dedicated
variant.

[example]: https://docs.rs/turso/0.6/turso/examples/concurrent_writes/index.html

## Migrations

`apply_migration` wraps each migration in `BEGIN` / `COMMIT`; a
statement failure rolls the migration back. The migration generator
emits the same SQLite-compatible DDL as the SQLite driver, including
the same six-step rebuild for unsupported `ALTER COLUMN`s. See
[SQLite migrations](./sqlite.md#migrations) for the mechanics.
