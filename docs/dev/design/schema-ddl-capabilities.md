# Schema DDL capabilities

## Summary

Let drivers describe schema execution separately from their query dialect and
application transaction capabilities. Schema push and generated migrations
honor DDL transaction restrictions, use driver-provided index creation syntax,
and wait for asynchronous schema work to become ready before reporting success.
Users opt into a constrained backend profile; ordinary PostgreSQL and SQLite
keep their existing behavior. The contract is generic, with Aurora DSQL as one
motivating backend. This is a proposed design, not an implemented API.

## Motivation

A PostgreSQL-compatible connection can support application queries while
rejecting the DDL used to create those queries' tables and indexes.
[#1329][proposal] describes this gap. Aurora DSQL [requires `CREATE INDEX
ASYNC`][dsql-index], [allows one DDL statement per transaction and forbids mixing
DDL with DML][dsql-ddl], and [returns an index-build job before the index is
ready][dsql-async]. This includes unique indexes, where a failed build must not
be mistaken for a successfully installed constraint.

At upstream commit `45ac82a0ba9af5efb9086ca0638b0103c9566e08`, the
[index serializer][current-index] emits `CREATE [UNIQUE] INDEX` without an async
mode, and [PostgreSQL migration generation][current-generation] uses that
serializer with the ordinary PostgreSQL capabilities. The current
[`push_schema` implementation][current-push] calls
[`create_table`][current-table], which submits table creation and secondary
indexes separately. It does not introduce a multi-DDL transaction wrapper; its
immediate index blocker is the syntax. [Migration application][current-apply]
starts a transaction, executes every migration statement, inserts the history
row, and commits. Both multiple DDL statements and a single DDL followed by
that bookkeeping insert violate the DSQL transaction contract.

Hand-written SQL can work around this, but users then maintain another schema
path and must solve asynchronous readiness and partial application themselves.
Recognizing those properties as schema capabilities lets other constrained
SQL backends use the same migration machinery.

## User-facing API

### Choose the target before creating or generating a schema

Continue to use the ordinary driver for ordinary PostgreSQL. Opt into a
backend's documented schema profile before constructing the `Db`. Profile
selection is explicit; Toasty does not infer a hosted product from a hostname,
probe by executing DDL, or silently retry rejected syntax with another dialect.

The following names are illustrative. An ergonomic PostgreSQL-driver preset
might look like this:

```rust
let driver = PostgreSQL::new(url)?
    .schema_profile(PostgreSQLSchemaProfile::aurora_dsql())?;
let db = Db::builder().build(driver).await?;
db.push_schema().await?;
```

The preset belongs to the PostgreSQL driver, not to a core enum of cloud
products. It selects generic schema capabilities and the matching rendering
and completion adapter. Those adapters use the driver's existing SQL
connection; no AWS SDK or Amazon-specific dependency is needed in core crates.
A profile also declares its supported types and schema mutations accurately;
selecting it is not a claim of complete PostgreSQL compatibility.

Before this change, an ordinary `PostgreSQL::new(url)?` cannot create a
secondary index on this target. After the change, selecting a supported
profile makes the same model definition and `push_schema` call use the
target's schema contract. Code targeting ordinary PostgreSQL needs no change.

### Apply migrations for the same target

Select the same schema profile when generating and applying migrations,
including through the CLI or an embedded `MigrationSet`. Generation may run
offline: the target and its capabilities must come from configuration, rather
than being discovered only after connecting during application.

For constrained targets, the generator produces SQL plus a versioned plan
manifest. Keep both files in version control. The manifest records explicit
step boundaries, DDL/DML/read classification, transaction groups, target
requirements, asynchronous completion requests, and a content digest. The
CLI and embedding macro preserve and consume it, rather than reconstructing a
plan from SQL text at startup. The application's model snapshot stays a schema
snapshot; it is not evidence of which remote steps have committed.

A new plan-aware migration API is opt-in. The existing SQL-only `Migration`
constructors and files keep their legacy path on existing profiles. A
constrained profile rejects a SQL-only migration before execution, even if
its SQL appears to contain just one statement. Breakpoint comments identify
text chunks today, not a safe execution or readiness contract. Do not split
on semicolons or rewrite saved SQL when changing the target profile. Generate
a plan for that target, or explicitly author a plan through the new API and
have the driver validate its steps.

If the configured target cannot satisfy the artifact's requirements, application
fails before bootstrapping history tables or executing migration steps. The
manifest carries a versioned adapter identifier and required semantics, not a
product name as an authorization shortcut. A driver must understand both the
syntax adapter and the requested semantics; merely accepting PostgreSQL wire
protocol is insufficient. Editing SQL without updating its manifest fails the
content check. Older binaries reject unsupported manifest versions rather than
applying the SQL while discarding its metadata.

### Understand success and partial failure

`push_schema().await` succeeds when every requested schema object is ready,
including all asynchronous indexes. A migration is recorded as applied only
after the same condition holds. An index submission is never treated as
completion. Dependent steps wait for readiness; this proposal deliberately
runs the steps in order rather than building independent indexes in parallel.

On a backend that cannot commit the whole migration atomically, application
requires explicit acceptance of partial schema changes in the migration
configuration. The CLI shows that property before applying; noninteractive
application must set the same acknowledgement in configuration. There is no
prompt that an embedded application unexpectedly has to answer.

If step 3 fails after steps 1 and 2 committed, the error identifies step 3,
reports steps 1 and 2 as confirmed, and leaves the migration incomplete. It
does not claim that rollback restored the old schema. Later migrations stop.
An uncertain commit or an index build still running is reported separately
from a confirmed failed operation. Inspect and reconcile incomplete work
before applying again; the default is not to replay the whole file.

## Behavior

### Capabilities, rather than a product taxonomy

Propose a separate `SchemaCapabilities` value, exposed by an additive,
defaulted driver method. It is independent of `Capability::sql`, the existing
`SchemaMutations`, and `Capability::interactive_transactions`.
`SchemaMutations` still decides which changes can be expressed; the new
contract decides how supported changes are executed and completed.

The initial schema execution properties are:

| Property | Meaning |
| --- | --- |
| Transactional DDL | Declares whether DDL can actually roll back. |
| DDL grouping limit | An optional positive maximum DDL count per transaction; unlimited by default where supported. |
| DDL with DML | Whether a DDL transaction may also contain DML, including migration bookkeeping. |
| Transaction delivery | Driver-owned interactive transaction or autocommit delivery; implicit-commit DDL must run outside an existing transaction. |
| Index creation | Synchronous, or asynchronous submission requiring a completion adapter. |

These are mechanisms and limits, not promises of full backend compatibility.
Invalid combinations are rejected when constructing a profile: a zero
statement limit, an async mode without a completion adapter, or transactional
rollback claimed for an implicit-commit backend. A boolean named
`async_index_creation` alone cannot select arbitrary SQL syntax. The adapter
renders that mode, captures any job token, and proves completion.

For example, the DSQL profile permits transactional DDL with a limit of one,
forbids DDL with DML, and uses asynchronous index creation. It may support
interactive application transactions at the same time. Another backend may
need autocommit DDL without asynchronous indexes. Do not infer one property
from the other.

### Planning and transaction boundaries

Preflight the entire supported plan, including bootstrap operations and index
completion requirements, before making schema changes. Every constrained
plan step contains exactly one physical statement or one driver-native
schema request. A logical change may expand into multiple physical steps,
each counted independently. The driver validates this property before effects;
user-supplied metadata is not permission to send a multi-statement batch.
Generated typed statements can establish boundaries before rendering. A
driver unable to validate an authored opaque SQL step rejects it; a marker
comment or declared statement count is not sufficient validation.

Choose only transaction groups the target can execute with the stated
rollback semantics. Preserve the existing all-or-nothing migration and
history insert where ordinary PostgreSQL or SQLite support them. If DDL is
limited to one per transaction, commit that statement before the next. If DDL
and DML cannot mix, all history creation DDL and history updates obey the same
separation. An empty generated migration performs no schema steps.

A constrained migration proceeds as follows:

1. Validate the saved target requirements, content digest, every physical step,
   completion adapter, and acknowledgement of non-atomic application.
2. Bootstrap required history/journal tables using legal, separate schema
   steps. Verify existing tables' definitions; existence alone is not proof
   that the tracking schema is usable.
3. Create an in-progress migration record in a separate DML transaction, before
   the first user schema step. Record migration identity and plan digest.
4. Before each schema step, durably record its intent. Commit the DDL in its
   permitted transaction or autocommit context. Capture its response, then
   wait for confirmed readiness outside that DDL transaction. Record confirmed
   progress in a separate DML transaction.
5. Only after all steps are confirmed ready, record the migration as applied
   and clear its in-progress state together in a DML transaction.

The intent journal detects interrupted application; it does not make DDL and
history atomic. A process can die after a DDL commit and before storing its
job token or success. That is an unresolved outcome, requiring inspection of
the actual catalog/object state. History-table bootstrap can fail before any
journal exists; such an error reports that phase and must be reconciled too.

An existing in-progress record, a different digest for the same migration ID,
or an unresolved bootstrap schema stops application before new user steps.
An applied record can be skipped only when its digest agrees. Keep existing
legacy history readable; do not present old rows lacking digests as verified
new-plan application. Upgrading tracking tables is itself planned DDL subject
to the same limits. This design requires deployment coordination with a
single active migrator per database. A unique in-progress record protects
against duplicate application of the same migration, but is not a lock
against different migrations or external DDL.

`push_schema` shares target validation, statement isolation, and completion
handling. It remains a schema-creation helper, without migration history or
automatic resume. On partial failure it reports confirmed objects and the
unresolved step. Calling it again does not imply idempotent reconciliation.

### Async completion, timeouts, and cancellation

For an asynchronous index, submission and readiness are separate observable
states. The DSQL adapter captures the returned job ID and can use the
[documented job status and catalog validity][dsql-async] to distinguish
pending, ready, and failed. Commit the submission before waiting. A `false`
wait result, a missing response, or an expired job-history entry is not
success. Object definition and catalog validity may establish readiness when
job history is no longer available; matching only an object's name cannot.

The public options include a finite schema completion timeout and a bounded
poll interval. Proposed defaults are five minutes and one second, with
backoff/jitter for repeated status requests; users can raise the timeout for
large indexes. Timeout and cancellation stop the caller's wait, not the remote
job. A connection with an unresolved DDL transaction must be rolled back or
discarded before returning to the pool; cancellation must not let a later
caller inherit it. No automatic `DROP INDEX`, compensating migration, or
background task owned by Toasty continues after the caller stops. A subsequent application
encounters the incomplete record and requires reconciliation.

Errors preserve the backend cause and expose the migration/step identity,
confirmed progress, and outcome class (failed, pending, or unknown). A
preflight inability uses `unsupported_feature`; async build failure remains a
driver operation failure with context. New typed schema timeout,
partial-application, and unknown-outcome errors carry the underlying cause,
including an existing serialization-failure classification where applicable.
They must not collapse into an undifferentiated success or a promise of
rollback. The precise Rust error wrapper is an implementation review item.

### Conflicts and retry policy

The default remains no automatic retry. An opt-in schema retry policy may
repeat a bounded number of attempts, with backoff and jitter, only when the
driver proves the attempted transaction aborted without effects. Retry the
current legal transaction, not already committed steps or the whole migration.
After exhausting the bound, report its last error and confirmed progress.

DSQL documents schema conflicts as SQLSTATE `40001` with OCC detail `OC001`;
`OC001` is not a second SQLSTATE. The current PostgreSQL driver already
[classifies `40001` as a serialization failure][current-errors]. A classified
error during a known aborted submission transaction may be retryable. The
same error during a post-commit readiness check does not mean the index
submission should be replayed. A lost connection during commit is an unknown
outcome, not proof of an abort. Never retry arbitrary user SQL, assume
`IF NOT EXISTS` verifies an object, or recreate a possibly running index job.

## Edge cases

A schema with no secondary indexes still needs legal DDL/history separation
when migrations apply. Both unique and non-unique async indexes must reach
confirmed validity. Existing duplicate rows can make a unique build fail;
Toasty reports that failure and leaves repair to the user. It must not drop
or relax uniqueness to finish a migration.

Dependencies retain order: a table must exist before its indexes, an index
must be ready before a dependent step, and schema changes needed by later
statements must be visible after commit. SQLite table-rebuild plans contain
DDL and data-copy steps; these classifications and atomic groups must survive
generation. A constrained profile rejects an unsupported rebuild before
starting it instead of splitting a change whose correctness needs atomicity.
An unsupported named enum, type change, or foreign-key operation is likewise
rejected where the target's capabilities do not support it. This proposal does
not promise DSQL support for every PostgreSQL schema or operation.

Reject schema operations inside a caller-owned transaction when the target
requires a commit boundary or cannot preserve that transaction's guarantees.
Do not insert a hidden commit, downgrade a transaction to no-ops, or use a
savepoint as a substitute for separate committed DDL transactions. A pending
index observed by another session does not satisfy migration readiness.

Crash windows include a committed table without a progress record, a submitted
job without a saved token, a completed index with expired job history, and all
DDL ready but the final history insert uncertain. The safe default is to stop
and inspect. A future resume API must validate the plan digest and the object's
actual definition and readiness; automatic resume and repair are not supplied
by this design. Migration identity alone is insufficient.

## Driver integration

### Declare and carry the schema profile

Driver implementors opt into a defaulted schema-capability method and
plan-generation/application hooks. Prefer returning metadata borrowed from the
driver instance rather than extending the existing `&'static Capability`
query contract to hold runtime configuration. Every connection created by the
driver receives the same immutable schema profile, including replacement
connections. Offline generation and runtime execution must see identical
requirements and adapter versions.

Keep existing required trait methods and SQL-only constructors usable for
legacy drivers. The default reports legacy/unrecognized schema support and
leaves that driver's existing path intact; it must not guess transactional
DDL from `sql()` or `interactive_transactions`. New plan hooks default to
`unsupported_feature`. Use a separate `MigrationPlan` type rather than adding
a variant to the exhaustively matchable `Migration` enum or changing its
constructors. Plan-aware embedded artifacts use an additive entry point; old
`MigrationSet` values retain their SQL-only contract. Legacy drivers compile
without implementing the new hooks and refuse new structured plans before
effects. Avoid adding required
fields to the exhaustively constructible `Capability` or new `Operation`
variants just to dispatch schema work. Existing connection schema methods
remain the entry points, with additive opt-in plan hooks. This does not
promise source compatibility if an implementation instead changes those
required signatures; such a change needs explicit breaking-change review.

### Render, execute, and prove completion

A supporting SQL driver supplies a renderer for its schema operations and a
schema executor that can return an immediate result or an opaque async token.
Tokens are retained in plan progress but are not interpreted by core code.
The driver's completion adapter reports pending, ready, failed, or unknown,
with a stable object identity and backend cause. Provide transaction abort
classification if opting into safe retries. Validate all declared physical
step boundaries before execution, including authored plans.

The generator uses the driver's configured schema capabilities for both
`push_schema` and migration plans. It must not select ordinary PostgreSQL
capabilities again during serialization. The CLI/embedded path preserves
transaction groups, completion requests, and artifact validation instead of
flattening the plan back to unannotated SQL. Do not render vendor syntax by
post-processing arbitrary SQL strings. Adding `ASYNC` to a supported index
statement preserves its quoting, uniqueness, key order, and expressions.

| Backend | Contract |
| --- | --- |
| PostgreSQL | Default synchronous indexes; preserve atomic generated migrations and history insertion where the operations permit it. Special nontransactional operations need explicit per-operation requirements. |
| SQLite | Default synchronous indexes and transactional DDL; preserve rebuild/copy dependencies and atomic grouping. |
| MySQL/MariaDB | Preserve native index behavior. Model implicit commits for affected DDL operations honestly; a transaction wrapper cannot promise rollback. Plan-aware migrations require non-atomic acknowledgement when appropriate. Legacy files keep their path; this design does not repair past partial migrations. |
| Constrained PostgreSQL-compatible SQL | Declare actual schema mutation/type support, grouping and DML limits, syntax adapter, and readiness semantics. DSQL is one possible driver-owned preset. |
| DynamoDB | Keep native table/index provisioning and its existing readiness behavior. No SQL rewriting, SQL transaction wrapping, or new attribute encoding is introduced. SQL migration plans remain unsupported; migration panic/error handling is tracked separately in [#1162][dynamo-migrations]. |

[#1128][transaction-issue] and [merged #1302][transaction-pr] concern query
plans on backends without interactive transactions. This design neither sets
that flag to false just because DDL is constrained nor changes query reads,
write atomicity, or explicit transaction APIs. The existing
[atomic-batches design](atomic-batches.md) governs application write delivery;
a migration split into committed schema steps is not an atomic write batch.

The migration system in [#190][migration-issue] and the
[standalone CLI design](standalone-cli.md) provide the generation/application
workflow. They must preserve this schema plan's target and execution contract.
[#1269][driver-issue] discusses driver registration, out-of-tree rendering, and
CLI reuse. These hooks should compose with its driver-owned renderer boundary;
a product-per-variant core dialect extension or new registry is not required
by this proposal.

## Alternatives considered

**A core `Dialect::AuroraDsql` variant.** This makes a first example compact,
but ties schema syntax, limits, readiness, and unrelated query behavior to a
closed product list. Compatible engines can share a query dialect while
having different schema contracts, and backend features can evolve
independently. Keep generic capabilities and driver-owned presets/adapters;
a dialect still identifies the actual SQL family.

**Only boolean flags.** Separate capabilities are better than product
branching, but two booleans do not express rollback, DDL/DML separation,
completion hooks, or physical statement boundaries. Use typed modes and
limits for related semantics; simple independent facts can remain flags.

**Rewrite generated SQL and remove `BEGIN`.** It cannot safely identify SQL
statements containing quoted semicolons or procedural bodies, does not wait
for jobs, and may drop guarantees needed by data-copy migrations. Explicit
plans and native driver rendering let unsupported work fail before effects.

**Always use one statement per transaction.** This weakens atomic PostgreSQL
and SQLite migrations and still mixes DDL with history DML unless tracked
separately. Apply restrictions only where the target requires them.

**Automatic migration replay or `IF NOT EXISTS` everywhere.** A completed
object can have a different definition, an async job can still be running,
and the history insert can fail after schema changes commit. A durable intent
record plus explicit reconciliation is safer than claiming idempotence.

**Keep hand-written DDL outside Toasty.** It remains an escape hatch, but
users lose model-to-schema generation and have to implement the same
transaction/readiness contract themselves. This design makes that contract
reviewable without making the core depend on a hosted service's SDK.

## Open questions

- **Blocking acceptance:** accept the separate schema contract and explicit
  partial-application acknowledgement, rather than promise atomic schema
  migrations on every SQL backend. This proposal takes that direction.
- **Blocking implementation:** finalize additive trait methods, driver
  profile builders, error wrappers, manifest version/digest layout, and
  tracking-table upgrade behavior. Preserve the legacy path and fail closed
  for unsupported new plans as described here.
- **Blocking implementation:** select the first driver's exact completion
  and catalog validation strategy, and demonstrate recovery after the
  submission/token crash window. Stopping for reconciliation is the default.
- **Deferrable:** expose a general profile-registration builder or begin with
  driver-owned presets plus implementor hooks. Prefer presets for user
  ergonomics while keeping the shared contract generic.
- **Deferrable:** tune the five-minute wait timeout and one-second polling
  defaults with backend experience; both remain finite and configurable.

## Out of scope

- Implementation in this PR; it follows acceptance of this roadmap/design.
- AWS authentication, token refresh, SDK integrations, and hostname detection;
  they do not define schema semantics.
- New query operators, schema type support, or complete compatibility with a
  particular PostgreSQL-compatible product; declare unsupported work honestly.
- Distributed migration locking, automatic repair/resume, rollback by
  compensation, and parallel async job scheduling; they need separate
  contracts, and deployments coordinate one active migrator today.
- General transaction retry and application DML batching; those must not
  inherit schema-only relaxations.
- SQL migration support for DynamoDB or a general driver registry; existing
  backend-specific provisioning and separate proposals own those topics.

[proposal]: https://github.com/tokio-rs/toasty/issues/1329
[transaction-issue]: https://github.com/tokio-rs/toasty/issues/1128
[transaction-pr]: https://github.com/tokio-rs/toasty/pull/1302
[migration-issue]: https://github.com/tokio-rs/toasty/issues/190
[driver-issue]: https://github.com/tokio-rs/toasty/issues/1269
[dynamo-migrations]: https://github.com/tokio-rs/toasty/issues/1162
[dsql-index]: https://docs.aws.amazon.com/aurora-dsql/latest/userguide/create-index-syntax-support.html
[dsql-ddl]: https://docs.aws.amazon.com/aurora-dsql/latest/userguide/working-with-ddl.html
[dsql-async]: https://docs.aws.amazon.com/aurora-dsql/latest/userguide/working-with-create-index-async.html
[current-index]: https://github.com/tokio-rs/toasty/blob/45ac82a0ba9af5efb9086ca0638b0103c9566e08/crates/toasty-sql/src/serializer/statement.rs#L61-L76
[current-generation]: https://github.com/tokio-rs/toasty/blob/45ac82a0ba9af5efb9086ca0638b0103c9566e08/crates/toasty-driver-postgresql/src/lib.rs#L250-L259
[current-push]: https://github.com/tokio-rs/toasty/blob/45ac82a0ba9af5efb9086ca0638b0103c9566e08/crates/toasty-driver-postgresql/src/lib.rs#L543-L569
[current-table]: https://github.com/tokio-rs/toasty/blob/45ac82a0ba9af5efb9086ca0638b0103c9566e08/crates/toasty-driver-postgresql/src/lib.rs#L453-L478
[current-apply]: https://github.com/tokio-rs/toasty/blob/45ac82a0ba9af5efb9086ca0638b0103c9566e08/crates/toasty-driver-postgresql/src/lib.rs#L627-L656
[current-errors]: https://github.com/tokio-rs/toasty/blob/45ac82a0ba9af5efb9086ca0638b0103c9566e08/crates/toasty-driver-postgresql/src/lib.rs#L60-L68
