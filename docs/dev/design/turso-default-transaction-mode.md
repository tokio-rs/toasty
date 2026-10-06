# Turso default transaction mode

## Summary

`Turso::with_transaction_mode(TransactionMode)` configures the driver's default
transaction mode, including transactions Toasty starts implicitly.

## Motivation

Turso Cloud hosts both Turso-engine and libSQL databases. The Turso engine
supports `BEGIN CONCURRENT`; libSQL rejects it. Deleting a model with a
has-many relation can require several database operations, so Toasty starts a
transaction even when the caller does not request one. Configuring the driver
with `Deferred` lets these operations run against libSQL.

## User-facing API

Set the mode on the existing driver builder before passing it to `Db::builder`:

```rust
use toasty_core::driver::operation::TransactionMode;
use toasty_driver_turso::Turso;

let driver = Turso::new(std::env::var("TURSO_DATABASE_URL")?)?
    .with_auth_token(std::env::var("TURSO_AUTH_TOKEN")?)
    .with_transaction_mode(TransactionMode::Deferred);
let db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .build(driver)
    .await?;
```

## Behavior

With `Default`, the driver keeps its natural default: serverless connections
and embedded connections configured with `concurrent_writes()` use
`BEGIN CONCURRENT`; other embedded connections use `BEGIN`.

With `Deferred`, `Immediate`, or `Exclusive`, default transaction starts use
`BEGIN DEFERRED`, `BEGIN IMMEDIATE`, or `BEGIN EXCLUSIVE`, respectively. This
includes `db.transaction()`, multi-operation plans, and read-modify-write
operations. Backend errors retain their existing classification.

## Edge cases

An explicit mode on `db.transaction_builder().mode(...)` overrides the
driver-wide mode. `with_transaction_mode(Default)` restores the natural
default. The option does not enable or disable the embedded MVCC journal;
`concurrent_writes()` still controls that. Configuration order does not affect
the precedence between these two options.

## Driver integration

Only the Turso driver changes. No new capabilities or operations are required;
other drivers and explicit transaction modes keep their existing contracts.
Schema pushes and migrations retain their existing transactional batches.

## Alternatives considered

Wrapping each operation in an explicit deferred transaction leaves implicit
transactions unconfigured. Inferring the engine from the URL scheme is
unreliable because both Cloud engines accept the same connection schemes.

## Out of scope

Engine detection and libSQL replication are separate capabilities. There are
no open questions for the default-mode option.
