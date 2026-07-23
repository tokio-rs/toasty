# Retry-safe transparent recovery from connection loss

## Summary

Classify every statement as `ReadOnly` or `Mutating` at exec time
by walking its AST.  When `Connection::exec` returns
`Error::connection_lost`, the engine retries `ReadOnly` statements
on a fresh connection (bounded) and propagates everything else.
Callers stop seeing one `connection_lost` per pool-restart event
for the common read path.

Same retry plumbing also retries the first statement of an
explicit transaction (a separate rule from classification: the
transaction has issued no other statements and no `COMMIT` has
been sent, so the server-side rollback leaves nothing to undo).
The classifier is also the foundation for any future API that
wants to know whether a statement mutates (e.g. [#981]'s
read-only handle).

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

The classifier also unblocks two adjacent items:

1. **First-statement-of-transaction retry** ([#863], same issue).
   The transaction has issued no other statements and no `COMMIT`
   has been sent, so a server-side rollback on connection drop
   leaves nothing to undo.  Different rule from the read-only one,
   but uses the same retry plumbing.
2. **Read-only API surfaces** ([#981]).  A runtime-checked
   `DbReader` handle that rejects mutating statements is a natural
   consumer of the same classifier.  No separate walk required.

CTE-with-mutation queries (Postgres `WITH ins AS (INSERT ...
RETURNING *) SELECT * FROM ins`) make a static-only classifier
unsound: a `Statement::Query` can carry an `ExprSet::Insert` in its
`WITH` clause, and mutations can appear as `Expr::Stmt` elsewhere in
the tree.  The walk is the load-bearing piece; everything else is
policy.

[PR #861]: https://github.com/tokio-rs/toasty/pull/861
[#863]: https://github.com/tokio-rs/toasty/issues/863
[#981]: https://github.com/tokio-rs/toasty/issues/981

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

- **Classification.**  A statement is `ReadOnly` if it is a
  `Statement::Query` and contains no `Insert`, `Update`, or `Delete`
  statement anywhere in its tree (filter, returning, CTE bindings,
  set-op operands, embedded subqueries).  Otherwise `Mutating`.

- **When the classifier runs.**  At exec time, on the statement
  the engine is about to hand to `Connection::exec`.  One pass per
  top-level statement; cost is linear in the statement tree size
  and runs once even when no retry occurs.

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

- **CTE with mutation.**  `WITH ins AS (INSERT INTO t ... RETURNING
  *) SELECT * FROM ins` parses as `Statement::Query` whose `with`
  carries a CTE with an `ExprSet::Insert` body.  The classifier walks
  `with` and classifies the whole statement as `Mutating`.

- **Mutation sub-statements in expressions.**  Same handling — any
  `Expr::Stmt(Insert | Update | Delete)` reached during the tree
  walk forces `Mutating`.  Today the verify pass already encounters
  these via `visit_expr_stmt` (`engine/verify.rs`); the classifier
  is a separate walk with simpler logic but lives next to it
  conceptually.

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

Nothing changes.  Drivers continue to surface
`Error::connection_lost` from `Connection::exec` on connection
drop; the engine handles the retry on the engine side using the
existing pool checkout machinery.  No new `Driver` or `Connection`
methods.

The classifier reads only `stmt::Statement` / `stmt::Expr`, which
is shared across all drivers, so the same retry policy applies
uniformly to SQL drivers and to DynamoDB.

## Open questions

- **Classifier placement.**  Three options: a free function on
  `&stmt::Statement` (read by the pool-retry wrapper), a method on
  `Engine`, or a field cached on the `Operation` enum produced by
  the planner.  The pool's retry wrapper wants a `bool`-shaped
  answer; the simplest shape is a free function called once at
  exec entry.

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
  the classifier; tracked in #981.
- **Configurable retry backoff.**  Iteration 1 retries immediately
  on a fresh connection.  Exponential backoff is a follow-on if
  needed.
