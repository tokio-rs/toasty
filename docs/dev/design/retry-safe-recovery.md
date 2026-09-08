# Retry-safe transparent recovery from connection loss

## Summary

When `Connection::exec` returns `Error::connection_lost`, retry the
statement on a fresh connection (bounded) if the engine's statement
classifier reports `ReadOnly`; propagate everything else.  Callers stop
seeing one `connection_lost` per pool-restart event for the common read
path.

The same retry plumbing also retries the first statement of an explicit
transaction.  That is a separate rule from classification: the
transaction has issued no other statements and no `COMMIT` has been
sent, so the server-side rollback leaves nothing to undo.

## Motivation

The connection-pool resilience design ([PR #861]) recovers the pool
after a backend restart, but every in-flight caller still sees one
`Error::connection_lost`.  For read queries that's pure noise: the
statement has no side effects, retrying produces the same result,
and a transparent retry on a fresh connection turns the failure into
nothing.

Most production deployments hit this every time the database
restarts (planned failover, maintenance window, autoscaling event).
Today the user sees a flurry of one-shot errors that disappear by
themselves; cleaning them up requires hand-rolling a retry layer
on top of `is_connection_lost()`.

First-statement-of-transaction retry ([#863], same issue) is a
different rule, but it uses the same retry plumbing, so both land
together.

[PR #861]: https://github.com/tokio-rs/toasty/pull/861
[#863]: https://github.com/tokio-rs/toasty/issues/863

## User-facing API

No new public types in iteration 1.  The observable change is the
absence of spurious errors during pool recovery:

```rust
// Before this design: a backend restart while this is in flight
// surfaces one Error::connection_lost per concurrent caller, even
// though every call could be retried safely.
let users = User::all().exec(&mut db).await?;

// After: the engine retries on a fresh connection (bounded);
// callers see Ok on success or Error::connection_lost only if every
// retry attempt also failed.
let users = User::all().exec(&mut db).await?;
```

A `Builder::disable_transparent_retry()` knob (default off, i.e.
retries enabled) lets callers who want to surface the raw error opt
out.  Useful for tests that assert recovery behavior; not needed by
ordinary application code.

```rust
let db = Db::builder(driver)
    .disable_transparent_retry()
    .build(schema)
    .await?;
```

## Behavior

- **When the statement is classified.**  At exec time, on the statement
  the engine is about to hand to `Connection::exec`.  One pass per
  top-level statement, whether or not a retry occurs.

- **Retry trigger.**  `Connection::exec` returns
  `Error::connection_lost`.  No other error variant triggers retry.

- **Retry policy.**  Up to one retry attempt on a fresh
  connection.  Matches Go's `database/sql` (two total attempts).
  If the second attempt also returns `connection_lost`, the error
  surfaces.

- **Transaction-first-statement retry.**  Same retry plumbing,
  orthogonal rule: track per-transaction whether any statement has
  reached the server.  If the failing statement is the first, retry
  regardless of classification.  The transaction has issued no
  other statements and no `COMMIT` has been sent, so the server-
  side rollback on connection drop leaves nothing to undo.  Ships
  in iteration 1 alongside read-only retry; both rules fan into
  the same retry wrapper.

- **Idempotent-write retry.**  Out of scope.  An `UPDATE ... WHERE
  id = N SET x = constant_value` is idempotent in principle, but
  proving idempotence requires analyzing every assignment and
  every filter predicate; the conservative classifier rejects all
  autocommit writes for now.  A follow-up can land that work
  behind the same retry plumbing.

## Edge cases

- **Lowering-generated sub-statements.**  `INCLUDE` subqueries and
  the other lowering-synthesized statements (recursive lower per
  PR #812) are all `Query`/`Select` shapes.  Classification on the
  pre-lowering AST is sufficient for retry decisions; lowered
  structure cannot introduce mutations that were not visible in
  the input.

- **Read against a side-effecting database function.**  `SELECT *
  FROM some_function()` where `some_function` writes is classified
  `ReadOnly` (Toasty has no way to know).  This is the same
  limitation as every other ORM and as `database/sql` itself; users
  who rely on side-effecting functions accept the risk.  Stored-
  procedure support ([#833]) would add an explicit mutation
  declaration if/when it lands.

- **Multi-statement transactions.**  A connection drop mid-
  transaction is fatal: retrying just the failing statement on a
  fresh connection produces inconsistent state.  The engine
  propagates the error and lets the caller restart the transaction.

- **All retry attempts fail.**  The user sees `Error::connection_lost`
  exactly once, regardless of how many internal attempts the engine
  made.  Matches today's surface for callers who already handle the
  error.

[#833]: https://github.com/tokio-rs/toasty/issues/833

## Driver integration

No new `Driver` or `Connection` methods.  Drivers signal a lost
connection by returning `Error::connection_lost` from
`Connection::exec`; the engine performs the retry using the existing
pool checkout machinery.

Only the PostgreSQL, MySQL, and Turso drivers construct
`Error::connection_lost` today.  SQLite is embedded and has no
connection to lose.  DynamoDB reaches a remote endpoint over HTTP but
reports transport failures as other error variants, so transparent
retry is a no-op there until that driver classifies them as
`connection_lost`.

## Open questions

- **Where classification runs relative to lowering.**  Classifying the
  statement handed to `Connection::exec` means classifying lowered
  statements; the argument in "Lowering-generated sub-statements" is
  about the pre-lowering AST.  Both placements should agree, but
  nothing verifies that lowering cannot introduce a mutation the input
  did not carry.  Blocking implementation.

- **`Builder::disable_transparent_retry` placement.**  On `Builder`
  (per-`Db`) is the proposed default.  Per-call disable
  (`.exec_no_retry`) is also conceivable but adds API surface for
  the rare case.

- **Bounded retry count.**  One retry (matching Go) is the
  proposal.  Higher counts (Postgres's `pgx` allows configurable)
  are a follow-on if anyone reports needing them.

- **Idempotent-write classification path.**  Whether to ship it in
  iteration 2 (after the read-only path proves the retry plumbing)
  or defer indefinitely until driver-level
  "definitely-pre-send" classification (#863's third alternative)
  lands.  Argument for iteration 2: a non-trivial fraction of
  ORM-generated writes are key-equality updates with constant RHS
  values, and classifying them retryable cleans up a second class
  of spurious errors.

## Out of scope

- **Idempotent-write classification.**  Deferred per the open
  question above.
- **Autocommit-write retry via driver-level "didn't reach server"
  signaling.**  Per #863's alternatives section; needs per-driver
  classification work to surface a richer error variant.
- **Mid-transaction retry.**  Server-side state divergence makes
  this unsound; left to the caller.
- **Read-only API surface (`DbReader`).**  Separate consumer of
  the classifier; tracked in [#981].
- **Configurable retry backoff.**  Iteration 1 retries immediately
  on a fresh connection.  Exponential backoff is a follow-on if
  needed.

[#981]: https://github.com/tokio-rs/toasty/issues/981
