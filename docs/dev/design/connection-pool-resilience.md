# Connection pool resilience

## Summary

Toasty's connection pool detects broken connections and replaces them
without surfacing the failure to the caller. Drivers report connection
loss two ways: through a typed `Error::connection_lost` returned by
`Connection::exec`, and through a sync `Connection::is_valid` flag that
recycle inspects before a connection is handed out.

By default the pool runs a low-cost background sweep: every minute it
pings the longest-idle connection. A successful ping returns it as
most-recently-used, so the next interval picks a different connection.
A failing ping — or any user query returning `Error::connection_lost`
— escalates: the pool pings every idle connection and drops the ones
that fail. One bad result triggers a full purge; one good result costs
roughly one round-trip per minute.

The sweep is configurable. Disable it for fully passive behavior, or
combine it with per-acquire pre-ping for paranoid deployments. An
optional connection lifetime cap covers the remaining hole — sockets
that an LB closes silently while the database is healthy enough to
satisfy a ping. After this lands, an application whose database restarts
sees at most one failed query — usually zero — before the pool returns
to a healthy steady state.

## Motivation

Issue [#678] reports that after a PostgreSQL restart, every query
through a `toasty::Db` keeps failing with `connection error: db error`
indefinitely. The pool hands out the same dead connection it held
before the restart, and nothing in Toasty replaces it.

The pattern is not specific to PostgreSQL or to a hard restart. Any
backend with long-lived TCP connections is affected: a Postgres failover,
an `idle_in_transaction_session_timeout`, MySQL's `wait_timeout`, or a
network appliance closing an idle socket all leave the pool full of
sockets that look fine until you write to them. A symptom common in
production: requests succeed for hours, the team deploys a database
upgrade, and afterward every request from every replica fails until the
process is restarted.

PR [#681] is the first attempt at a fix. It adds a sync `is_valid()`
method on the `Connection` trait, lets the connection task exit when
`is_valid()` returns false after an op, and lets `Manager::recycle`
catch that via the existing `join_handle.is_finished()` check. That
addresses "the pool never recovers" but leaves three problems:

- The MySQL, SQLite, and DynamoDB drivers all return `true`
  unconditionally — recovery only works for PostgreSQL.
- The first query after the restart still fails. Recovery happens
  between queries, not before them.
- There is no way to evict a silently rotted connection (closed by an
  LB or proxy while idle) without issuing a query that fails first.

The wider Rust and database ecosystem has converged on a small set of
mechanisms for this: a passive "is this connection known dead" check, an
active "ping before handing it out" probe, lifetime caps, and
background validation of idle connections. sqlx and bb8 default to
active per-acquire pings; deadpool-postgres is configurable; HikariCP
runs a `keepaliveTime` background check on top of active validation;
SQLAlchemy defaults toward active validation; Go `database/sql` and
pgxpool prefer passive detection plus periodic background sweeps. None
of them combine a single-connection sweep with eager escalation on
failure — the standard background sweep validates every idle
connection on every interval, which doubles the steady-state cost. The
escalation pattern proposed here costs roughly one round-trip per
minute when healthy and pays for a full pool sweep only after a real
failure.

[#678]: https://github.com/tokio-rs/toasty/issues/678
[#681]: https://github.com/tokio-rs/toasty/pull/681

## User-facing API

Pool resilience is controlled through four optional builder methods.
The defaults — opportunistic background sweep, no per-acquire ping, no
lifetime cap — match a typical production deployment with a remote
database that occasionally restarts.

### Tolerating a database restart

Toasty drivers classify backend errors. When the underlying client
reports that a connection is broken — `tokio_postgres::Error` with a
closed socket, `mysql_async::Error::Io`, etc. — the driver returns
`Error::connection_lost`. The pool sees that error, marks the
connection as bad, and the next call to `db` opens a fresh one. The
background sweep usually catches a restart before the application
sees any error at all; in the worst case the application sees one
failed query and recovers on retry.

```rust
match User::filter_by_id(1).exec(&mut db).await {
    Ok(user) => /* … */,
    Err(err) if err.is_connection_lost() => {
        // Transient. The pool has already discarded the dead
        // connection; retrying will pick a fresh one.
        User::filter_by_id(1).exec(&mut db).await?
    }
    Err(err) => return Err(err),
}
```

### The background health-check sweep

Once per `pool_health_check_interval`, the pool grabs the longest-idle
connection, sends it a lightweight ping (`SELECT 1` for SQL drivers,
`COM_PING` for MySQL, no-op for SQLite and DynamoDB), and returns it.
The successful ping touches the connection, so it is no longer the
longest-idle and the next interval naturally picks a different one. A
failing ping triggers an eager sweep — the pool drains the rest of the
idle connections, pings each, and drops the ones that fail. After one
bad ping, the entire pool is purged of dead connections within seconds.

The eager sweep also fires when any user query returns
`Error::connection_lost`. A real connection-lost error usually means
more than one connection is affected (database restart, network event),
so the pool does not wait for the next sweep interval to discover the
rest.

In the healthy case this costs one round-trip per minute regardless of
pool size. In the failure case it costs one round-trip per idle
connection, paid once when the failure is detected.

The default interval is one minute. Disable the sweep entirely with
`None`:

```rust
let db = toasty::Db::builder()
    .models(toasty::models!(User))
    // turn the sweep off; rely on errors only
    .pool_health_check_interval(None)
    .connect(&url)
    .await?;
```

Tighten it for a shorter recovery window:

```rust
let db = toasty::Db::builder()
    .models(toasty::models!(User))
    .pool_health_check_interval(Some(Duration::from_secs(15)))
    .connect(&url)
    .await?;
```

### Validating connections on every acquire

For deployments that cannot tolerate even one failed query — a public
API behind a flaky network, an idempotent worker that does not
implement retry — enable per-acquire pre-ping:

```rust
let db = toasty::Db::builder()
    .models(toasty::models!(User))
    .pool_pre_ping(true)
    .connect(&url)
    .await?;
```

With `pool_pre_ping(true)`, every checkout from the pool runs the same
ping the sweep uses, before returning the connection to the caller. A
failing ping evicts the connection and the pool reuses another idle
one or opens a fresh one. The caller sees either a healthy connection
or a clean `Error::connection_pool` if no connection can be opened
within `pool_create_timeout`.

The trade-off is one round-trip per checkout. Combine it with a
larger `max_pool_size` if the extra latency starts queueing requests.
`pool_pre_ping` is independent of the background sweep — most
deployments want one or the other, but enabling both is safe.

### Capping connection age

Some failures never produce an observable error on the connection
until the next write — a load balancer that silently closes idle
sockets is the classic example. The ping is itself a write, so the
sweep does catch this case, but only after the next interval fires.
To bound how long a connection can be alive or idle before reuse,
regardless of ping success:

```rust
let db = toasty::Db::builder()
    .models(toasty::models!(User))
    .pool_max_connection_lifetime(Some(Duration::from_secs(30 * 60)))
    .pool_max_connection_idle_time(Some(Duration::from_secs(10 * 60)))
    .connect(&url)
    .await?;
```

`pool_max_connection_lifetime` evicts connections older than the
limit. `pool_max_connection_idle_time` evicts connections that have
been sitting unused for longer than the limit. Both are checked in
`recycle` (when the pool considers handing the connection back out);
neither runs in the background. Both default to `None` (no cap).

Recommended for any deployment that talks to a remote database:
`pool_max_connection_lifetime` shorter than every idle timeout in the
path (server, load balancer, NAT). 30 minutes works for most clouds.

### Detecting connection loss in user code

`Error::is_connection_lost()` is a predicate on the existing
`toasty::Error`. It returns `true` when a query fails because the
underlying connection is broken — independent of whether the
connection had been used before, or whether retry is safe. The pool
already evicts the connection by the time the error reaches the user,
so a retry on the same `Db` will get a fresh one.

The error is **not** retried automatically. A connection that breaks
mid-query may have applied an `INSERT` whose response was lost; Toasty
cannot tell. Users who want automatic retry should wrap their query
calls and check `is_connection_lost()` (or use a backoff crate) at a
layer where they know the operation is idempotent.

## Behavior

**Passive detection (always on).** When `Connection::exec` returns
`Error::connection_lost`, the connection task records the failure and
exits its receive loop. `Manager::recycle` then sees
`join_handle.is_finished()` (or, equivalently, a sync `is_valid()`
returning false), discards the slot, and deadpool retries on another
slot or creates a new connection.

**Background sweep (default on).** A single tokio task spawned by the
pool sleeps for `pool_health_check_interval`, then runs one iteration:

1. Grab the longest-idle connection from the pool. If no idle
   connection is available (every connection is in use, the pool is
   empty, or the pool size is zero), skip the iteration.
2. Send a `Ping` operation to that connection's task and await the
   result, bounded by `pool_recycle_timeout`.
3. On success, return the connection to the pool. End the iteration.
4. On failure (the ping returned `Error::connection_lost`, or timed
   out), drop the connection — its task has already exited or will on
   timeout, and the pool's existing eviction logic discards it on next
   recycle. Then escalate.

The escalation pings every remaining idle connection: pop, ping, hold
healthy ones aside; drop failed ones; loop until the pool reports zero
idle. Healthy connections held during the escalation are returned to
the pool when the loop finishes. This drains all dead connections in
one pass without forcing a real query to discover them.

The next iteration starts fresh from step 1. Because a successful ping
returns the connection as most-recently-used, the longest-idle selector
naturally picks a different connection each interval — no extra state
needed to rotate. Over an idle pool of size `N`, every connection is
pinged once every `N × pool_health_check_interval`.

The sweep task is canceled when the `Pool` is dropped.

**Eager sweep on observed connection loss.** When `Connection::exec`
returns `Error::connection_lost`, passive detection (above) evicts that
connection. The pool also signals the sweep task to escalate
immediately — pinging every other idle connection and dropping those
that fail — rather than waiting for the next periodic tick. Eager
escalation cuts the recovery window after a database restart from
`pool_health_check_interval` to one round-trip per idle connection,
starting as soon as the first failure surfaces. Eager escalation runs
only when the background sweep is enabled; with
`pool_health_check_interval(None)` the pool relies on per-query
discovery only.

**Per-acquire validation (opt-in).** With `pool_pre_ping(true)`,
`Manager::recycle` sends the same `Ping` operation and awaits the
result, in addition to the passive check. A failing ping is treated
identically to a connection-lost error from `exec`. The check runs
inside the existing `recycle_timeout` budget. Default: off.

**Lifetime caps (opt-in).** When `pool_max_connection_lifetime` is set,
`recycle` rejects any connection whose `Metrics::created` is older than
the limit. `pool_max_connection_idle_time` rejects any connection
whose `Metrics::last_used` is older than the limit. Default: both
`None`.

**Order in `recycle`.** Lifetime check → idle check → passive
`is_valid` → optional active ping (when `pool_pre_ping` is true).
Cheap checks first; the round-trip runs only when the connection
survives all three local tests.

**Pool retry budget.** Deadpool already loops indefinitely on a failing
recycle, popping the next slot or creating a fresh connection. Toasty
inherits that behavior — the only failure surfaced to the caller is
exhausting `pool_create_timeout` or `pool_wait_timeout`.

**Build-time probe.** `Db::builder().build` already acquires one
connection to verify the database is reachable. With pre-ping enabled,
that acquire also pings the connection — startup fails fast if the
database is wrong, not on the first user query.

## Edge cases

- **Mid-query failure during a write.** Toasty cannot tell whether an
  `INSERT` reached the server before the socket died. The error
  surfaces to the user as `Error::connection_lost`; the pool evicts the
  connection but does not retry. Users running non-idempotent writes
  must handle this explicitly.

- **Mid-transaction failure.** A connection that breaks during a
  multi-statement transaction loses every uncommitted statement and the
  transaction handle. The connection is evicted and the user sees
  `Error::connection_lost`. Resuming the transaction is impossible —
  the user must restart it on a fresh connection.

- **In-memory SQLite.** The driver caps the pool at one connection.
  Connection loss on an in-memory database means the database itself
  is gone; eviction would create a fresh, empty database. Drivers that
  cap `max_connections()` at 1 implicitly opt out of automatic
  reconnection — see Driver integration below for the contract.

- **DynamoDB.** The AWS SDK manages its own HTTP connection pool and
  retry policy beneath Toasty's `Connection`. The driver's
  `is_valid()` always returns `true`, `ping()` is a no-op, and
  `Operation::exec` does not produce `Error::connection_lost`. Toasty's
  pool degenerates to a thin async-task wrapper for DynamoDB; nothing
  in this design changes that.

- **Pre-ping under heavy contention.** Each pre-ping consumes its
  connection while it runs. With `max_pool_size` set close to peak
  concurrency, the extra round-trip per acquire can become a tail
  latency bottleneck. The doc on `pool_pre_ping` will note that
  enabling it usually warrants raising the pool size.

- **Sweep under heavy contention.** When every connection is checked
  out the sweep skips its iteration; busy connections will surface
  failures organically through user queries. The sweep does not block
  user acquires — it competes for an idle connection on equal terms
  and yields if none is available within a brief poll window.

- **Sweep during a database restart.** The sweep cannot reconnect to
  a database that is fully down — its `Driver::connect` calls
  triggered by recycle failures will themselves fail until the
  database returns. While the database is unreachable, the pool
  empties as recycles fail and is rebuilt as `connect` succeeds again.
  No special handling: a failed sweep iteration is indistinguishable
  from a transient idle-pool state and the next interval retries.

- **Sweep escalation cost.** After a real failure, the escalation
  pings every idle connection in the pool. With a 100-connection
  pool, that is up to 100 round-trips concentrated within a few
  hundred milliseconds. This is bounded and one-shot — once dead
  connections are evicted the sweep returns to its single-ping
  cadence. Network amplification during this burst is acceptable;
  the alternative is letting user queries discover each dead
  connection one by one.

- **Lifetime cap and long checkouts.** Lifetime is checked at acquire
  time only. A query that holds a connection for an hour past
  `pool_max_connection_lifetime` is allowed to finish; the connection
  is evicted on its next return.

- **Clock jumps.** Lifetime and idle caps use `tokio::time::Instant`
  via `deadpool::managed::Metrics`, which is monotonic. Wall-clock
  jumps do not affect eviction.

## Driver integration

Out-of-tree drivers see two new trait additions and one new error
constructor. All have defaults so existing drivers compile unchanged;
they just stay on the passive-only path until they wire up the new
hooks.

### `Connection::is_valid`

```rust
trait Connection {
    /// Returns `false` if this connection is known to be unusable
    /// (the underlying socket is closed, the session was killed,
    /// the driver observed a fatal error). Sync, must not block.
    /// Default: `true`.
    fn is_valid(&self) -> bool { true }
    // … existing methods
}
```

Per-driver mapping:
- **PostgreSQL.** `!self.client.is_closed()`.
- **MySQL.** `mysql_async`'s `Conn` does not expose a passive flag.
  The driver tracks an internal `Cell<bool>` set to `false` whenever
  `exec` returns an `Error::connection_lost`.
- **SQLite.** Always `true`. SQLite connections do not silently break
  — corruption surfaces synchronously on the next call.
- **DynamoDB.** Always `true`. The SDK manages its own connections.

### `Connection::ping`

```rust
trait Connection {
    /// Active liveness probe. Issued by the pool when pre-ping is
    /// enabled; should be the cheapest round-trip the backend
    /// supports. Default: `Ok(())` (no round-trip).
    async fn ping(&mut self) -> crate::Result<()> { Ok(()) }
}
```

Per-driver implementation:
- **PostgreSQL.** `self.client.simple_query("").await`. The empty
  query is the lightest sync round-trip in the protocol; it does not
  invoke the parser.
- **MySQL.** `self.conn.ping().await` — `COM_PING`.
- **SQLite.** Default no-op.
- **DynamoDB.** Default no-op.

A failing `ping` must return `Error::connection_lost` (not a generic
`driver_operation_failed`) so the pool can apply its eviction logic.

### `Error::connection_lost` and `Error::is_connection_lost`

A new variant is added to `ErrorKind`. The constructor accepts a cause:

```rust
Error::connection_lost(underlying_io_error)
```

Drivers must map their backend's "connection is gone" errors to this
constructor. Concretely:

- **PostgreSQL.** `tokio_postgres::Error::source` is an
  `std::io::Error`, *or* `Client::is_closed()` is true after the call,
  *or* the error string is `"connection closed"`. Conservative rule:
  any `tokio_postgres::Error` whose `as_db_error()` returns `None` is
  treated as a connection-lost error.
- **MySQL.** `mysql_async::Error::Io` and `mysql_async::Error::Driver`
  variants whose underlying source is an `io::Error`. Server-side
  errors (the `Server` variant) are not connection-lost.
- **SQLite.** Never. SQLite has no connection layer that can break in
  isolation; if the file becomes unreadable that is a different error.
- **DynamoDB.** Never.

For the `serialization_failure` and `read_only_transaction` mappings
that already exist on the PostgreSQL and MySQL drivers, this design
adds one more match arm before the catch-all `driver_operation_failed`.

### Drivers with `max_connections() == 1`

A driver that caps the pool at one connection (in-memory SQLite, the
existing case) implicitly opts out of automatic reconnection: there is
no second connection to fall back to. The pool still evicts a broken
connection on `is_valid` returning false and creates a new one via
`Driver::connect`, which for in-memory SQLite means a fresh empty
database. Drivers in this category should document the failure mode;
no extra trait support is needed.

### Backward compatibility

`is_valid` and `ping` are added with defaults, so out-of-tree drivers
that do not implement them continue to compile and behave like the
SQLite/DynamoDB drivers — passive detection only, no active probe. The
new `Error::connection_lost` is constructible only by drivers; user
code can match on `is_connection_lost()` regardless of which driver is
in use.

## Alternatives considered

**Fully passive default (no sweep, no pre-ping).** Considered. This
matches Go `database/sql` and deadpool-postgres' `Fast`. Rejected
because issue [#678] is explicitly about the failure mode this
default cannot solve: a database restart leaves dead connections in
the pool until each one is discovered by a failing user query. The
sweep is cheap enough at one round-trip per minute that the
"zero-overhead" framing does not justify the worse user experience.
Users who do want fully passive can set
`pool_health_check_interval(None)`.

**Active pre-ping always-on (sqlx-style default).** Considered.
Rejected as the default because it adds a round-trip to every
checkout, including the common case where the database is healthy.
The background sweep gets nearly all of pre-ping's resilience benefit
at a tiny fraction of the overhead — a successful sweep recently
proves every connection's liveness, and the eager escalation handles
the rest. `pool_pre_ping` remains available as an explicit opt-in.

**Background sweep that pings every connection per interval
(HikariCP/pgxpool style).** Considered. Steady-state cost is
`pool_size` round-trips per interval, which is a real cost for large
pools and shorter intervals. Rejected in favor of the
"one-then-escalate" pattern: in the healthy case the cost is
constant in pool size; in the failure case the cost is the same as
the per-interval-everyone approach but paid only once.

**Transparent retry on `Error::connection_lost`.** Considered;
deferred. Go's `database/sql` retries up to two times on `ErrBadConn`,
which is elegant but rests on the driver guaranteeing the operation
never reached the server. None of Toasty's underlying drivers expose
that guarantee directly — a `tokio_postgres::Error` after a partial
write is indistinguishable from one before the write. Adding
transparent retry safely requires either (a) richer per-driver error
classification ("definitely-pre-send" vs "ambiguous"), or (b)
restricting retry to operations the engine knows are idempotent
(read-only queries). Both deserve their own design doc. This design
hands the user the predicate (`is_connection_lost()`) so they can
implement retry where they know it is safe.

**Per-driver classification via `classify_error` callback.** An
alternative to adding a new error variant: keep
`driver_operation_failed` as the single bucket and ask drivers to
implement a `fn classify(err: &Error) -> ErrorKind`. Rejected because
it forces every consumer (the pool, user code, future retry layers)
to call the classifier instead of pattern-matching the error. A typed
variant on the existing `Error` enum is cheaper at every call site.

**Sweep via deadpool's `Pool::retain` (sync) instead of acquire+drop.**
`Pool::retain` walks idle slots synchronously and drops those failing
a predicate. It is unsuitable for active validation because the
predicate cannot await. Used internally for the sync lifetime/idle
checks where it is a clean fit.

## Open questions

**Default value of `pool_health_check_interval`.** Proposed: 60
seconds. Trade-offs:

- Shorter (15–30s) detects a restart faster but pays more pings per
  minute and slightly more LB-keepalive churn.
- Longer (5 min) is cheaper but means up to 5 minutes of failing
  queries before recovery if a user query is the first to hit a dead
  connection.

60 seconds matches pgxpool's `HealthCheckPeriod` default. **Deferrable**
— easy to revisit once we have real deployment feedback.

**Default value of `pool_max_connection_lifetime`.** None means
"unbounded" and matches sqlx's behavior. HikariCP defaults to 30
minutes. Argument for unbounded: simpler model, users behind an LB
know their idle timeout and can set it explicitly. Argument for a
default: the failure mode is invisible in development and bites in
production. **Deferrable** — the design works either way.
Recommendation: ship with `None` and document the recommended setting
in the user guide.

**Should the sweep also check `pool_max_connection_lifetime`?** The
current proposal checks lifetime/idle caps only on `recycle`. The
sweep could also evict aged connections preemptively rather than
waiting for the next acquire. **Deferrable** — for short-lived pools
the difference is small; for long-lived idle pools it bounds eviction
latency. Easiest answer: yes, fold an age check into each sweep
iteration before the ping.

**`pool_pre_ping` default for the in-memory SQLite case.** With
`max_connections == 1`, pre-ping is wasted work. Should the builder
silently force `pool_pre_ping(false)` when the driver caps at one
connection? **Deferrable** — the round-trip is a no-op for SQLite, so
the cost is negligible. Leave the user setting alone.

**Skipping the sweep when `Connection::ping` is the default no-op.**
SQLite and DynamoDB return `Ok(())` from the default `ping`. Running
the sweep against them is harmless but useless. The pool could check
`Driver::capability()` (or a new flag) and skip spawning the sweep
task entirely. **Deferrable** — the cost of a no-op sweep is one
`tokio::sleep` per minute. Add a capability bit if a real workload
shows the noise.

**Should `is_valid` and `ping` collapse into a single async method?**
Two methods because they have different cost models — `is_valid` must
be cheap and sync (it is checked on every recycle), and `ping` is
async by nature. Combining them would force every recycle to await,
which makes the lifetime/idle checks pointless (they exist precisely
to short-circuit the round-trip). **Resolved** in favor of two methods.

## Out of scope

- **Engine-level transparent retry of idempotent operations.** Deferred
  to a separate design.
- **Per-query retry attributes.** A model-level "this query is
  idempotent, please retry" annotation is a richer feature than the
  pool needs and belongs in the engine work above.
- **Reconnect with backoff on initial connect failure.**
  `pool_create_timeout` already bounds startup; deadpool retries
  internally until it expires. Adding explicit exponential backoff has
  no clear caller today.
- **Connection warm-up / minimum pool size.** Out of scope; deadpool
  does not support a minimum size. Track separately if asked for.
- **Sweep jitter.** Multiple processes hitting the same database with
  synchronized 60-second sweeps could create a pingstorm. Acceptable
  at one ping per process per minute; revisit if benchmarks say
  otherwise.
