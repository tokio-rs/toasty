# Latest row per group with DISTINCT ON

## Summary

Add `.distinct_on(...)` to model list queries to select one complete row
per group on PostgreSQL, using the caller's ordering to choose the winner.
The result keeps the usual model hydration or an explicit typed projection.
SQLite, MySQL, and DynamoDB return an explicit capability error. This is a
proposal for the latest-per-group part of [#425]; implementation follows
only after the design is accepted.

## Motivation

A results table can contain many revisions of the same subject. Reading the
newest result per subject needs the winning row's body as well as its slot
and timestamp. Fetching every revision to deduplicate in Rust transfers
unnecessary rows and makes pagination difficult. Today users write SQL:

```sql
SELECT DISTINCT ON (subject) subject, computed_at, body
FROM results
WHERE kind = $1
ORDER BY subject, slot DESC;
```

`GROUP BY subject` with `MAX(slot)` finds the largest slot, but does not
identify a single body when several rows share that slot. Selecting a whole
row is different from calculating a summary of a group.

### What already exists

Checked against upstream `45ac82a0ba9af5efb9086ca0638b0103c9566e08`.
[PR #934] added `toasty_core::stmt::Select::distinct` and SQL serialization
of plain `SELECT DISTINCT`. Multi-step relation loading uses that flag to
collapse repeated targets. Public `.select(...)`, tuple `.order_by(...)`,
limits, and scalar `.count()` also exist. Raw `toasty::sql::query(...)`
returns dynamic `Value` records and explicitly does not hydrate models.
Thus #425's statement that the AST has no distinct modifier is outdated.

The checked tip has no public `.distinct_on(...)`, DISTINCT ON AST form,
or window-function builder for this read. Plain DISTINCT compares the
entire projected row; adding a primary key normally makes every model row
unique. This proposal preserves the existing plain-distinct behavior and
does not claim to finish every use case in #425.

## User-facing API

### Read the newest result for each subject

Assume a `ReadingResult` model maps to `results` and has scalar fields
`id`, `kind`, `subject`, `slot`, `computed_at`, and `body`. Call
`.distinct_on(...)` on its model list query, then order by the group field
and the fields that choose the winner:

```rust
let latest: Vec<ReadingResult> = ReadingResult::filter(
    ReadingResult::fields().kind().eq(kind),
)
.distinct_on(ReadingResult::fields().subject())
.order_by((
    ReadingResult::fields().subject().asc(),
    ReadingResult::fields().slot().desc(),
    ReadingResult::fields().id().desc(),
))
.exec(&mut db)
.await?;
```

Here "newest" means greatest `slot`; `computed_at` is data from the same
winning row. If time defines newest instead, order by `computed_at`.
The primary key breaks ties between equal slots. No field from a losing
row is mixed into a returned model.

`.distinct_on(keys)` is available on `Query<List<M>>` where `M: Model`
and returns the same query type. Accept a scalar field path or a nonempty
flat tuple of scalar field paths belonging directly to that source model.
Paths retain their usual Rust types; heterogeneous tuples are valid.
Relationship paths, embedded values, lists, arbitrary expressions, and
fields from other models are outside the initial surface. These inputs
must be rejected, rather than interpreted as SQL text.

### Choose a smaller result shape

Call `.select(...)` after `.distinct_on(...)` to project fields from the
winning row. Selection does not change which row wins:

```rust
let latest = ReadingResult::filter(ReadingResult::fields().kind().eq(kind))
    .distinct_on(ReadingResult::fields().subject())
    .order_by((
        ReadingResult::fields().subject().asc(),
        ReadingResult::fields().slot().desc(),
        ReadingResult::fields().id().desc(),
    ))
    .select((
        ReadingResult::fields().subject(),
        ReadingResult::fields().computed_at(),
        ReadingResult::fields().body(),
    ))
    .exec(&mut db)
    .await?;
```

The result uses the existing tuple hydration. Keys and ordering fields need
not appear in the projection. The initial API does not add `.distinct_on`
to an already projected query. Direct scalar projections of the source
model are supported; relation projections are deferred.

### Group by more than one field

For one result per `(kind, subject)`, pass a tuple:

```rust
let latest = ReadingResult::all()
    .distinct_on((
        ReadingResult::fields().kind(),
        ReadingResult::fields().subject(),
    ))
    .order_by((
        ReadingResult::fields().kind().asc(),
        ReadingResult::fields().subject().asc(),
        ReadingResult::fields().slot().desc(),
        ReadingResult::fields().id().desc(),
    ))
    .limit(20)
    .exec(&mut db)
    .await?;
```

The limit counts winning rows, not revisions considered. Results follow
the full query order, so groups are ordered by kind and subject here.
This API does not independently sort winners by newest slot across groups.

## Behavior

Filtering determines the candidate rows first. Within each group,
`.order_by(...)` determines the first candidate, and DISTINCT ON retains
that entire row. The returned winners follow the same ordering. Limit
and offset apply after winner selection, regardless of the order in which
the builder methods were called. Projection reads only the winners.

An explicit order is required. Its leftmost terms must contain every
distinct key exactly once, in any permutation, before any non-key term.
Directions may differ; subsequent terms rank candidates within a group.
This follows the [PostgreSQL SELECT reference] for the key-prefix rule;
requiring an explicit order is an additional Toasty validation rule.
Chained `.order_by(...)` calls append terms as they do today. Toasty does
not silently prepend keys, change an order, or promise a unique winner
when the supplied ordering ties. Add all primary-key components as final
tie-breakers when reproducibility matters.

An empty candidate set returns an empty list. `.first()` selects the
first winning row in query order, or `None`; it does not return one row
for each group. `.one()` and `.get()` retain their ordinary single-row
behavior and missing-record error. No existing queries change behavior.

### Errors and unsupported compositions

Validation occurs before any statement in the query plan is dispatched.
Errors are returned through the existing `toasty::Error` categories:

| Condition | Error category |
| --- | --- |
| Empty or duplicate keys; repeated `.distinct_on(...)` | `invalid_statement` |
| Keys outside the supported source-model scalar paths | `invalid_statement` |
| Missing order or non-key term before all group keys | `invalid_statement` |
| Driver lacks native DISTINCT ON | `unsupported_feature` |
| Cursor pagination, aggregate projection, or `.count()` on this query | `unsupported_feature` |
| Grouped aggregates or HAVING on the same query level | `unsupported_feature` |
| Row locks, mutation conversion, or set-operation composition | `unsupported_feature` |
| Relation projection or a plan needing residual candidate filtering | `unsupported_feature` |

If several rules fail, the error category need not have a specified
precedence. No unsupported composition may silently discard DISTINCT ON,
turn a read into a scan-and-deduplicate operation, or broaden a mutation.

Existing `.include(...)` remains model loading: load relations only for
the winning root models, preserving their order and identity. Includes
must not multiply the candidate rows before selecting a winner. When a
plan cannot satisfy this, return `unsupported_feature`. Projection and
include retain their existing compatibility rules; this proposal adds no
new combination of them. Related-model filters may be used only when the
complete candidate predicate is evaluated before selection in the same
database statement. Reject plans requiring post-selection filtering;
discarding a winner would incorrectly hide an eligible runner-up.

## Edge cases

Tied candidate orderings permit an arbitrary winner among the tied rows,
following PostgreSQL. A unique tie-breaker makes the selection stable for
a fixed snapshot. Concurrent inserts or updates can change winners across
reads; this API adds no isolation, locking, or multi-read snapshot promise.
Use the existing transaction guarantees when a stable read is needed.

PostgreSQL's native DISTINCT ON comparison determines groups, including
collation and floating-point behavior. SQL NULL keys belong to the same
group; a composite key compares all components. This is not Rust hashing
or a redefinition of application Option equality. Ordering expressions
retain Toasty's ordering contract, including optional-value ordering in
[the Option design](option-semantics.md); native grouping does not force
PostgreSQL's default null placement onto `.asc()` or `.desc()`. For a
simply nullable key, the Option contract can be preserved by `ASC NULLS
FIRST` or `DESC NULLS LAST`. Any ordering rewrite must still satisfy the
DISTINCT ON key-prefix rule; reject mappings that cannot preserve both.

Offsets skip winners. Cursor pagination is rejected in the initial
feature: pushing a cursor predicate into the candidates can promote a
runner-up from an already visited group. An eventual pagination design
must place that predicate outside winner selection and define what
happens when a winner changes between page reads.

A model primary key in the selected columns does not defeat DISTINCT ON:
only the specified keys define groups. Existing internal plain DISTINCT
on included relations stays scoped to those relation queries and must
not replace, merge into, or erase the root DISTINCT ON operation.

## Driver integration

Add a proposed `Capability::native_distinct_on` flag. It describes native
support for the operator, not window functions or client-side deduplication.
The query is rejected with `unsupported_feature` before dispatch when
this flag is false, even for an empty table or a zero limit.

| Driver | Capability | Observable behavior |
| --- | --- | --- |
| PostgreSQL | `true` | Native `SELECT DISTINCT ON (keys)` |
| SQLite, including SQLite-compatible drivers | `false` | Explicit `unsupported_feature` |
| MySQL / MariaDB | `false` | Explicit `unsupported_feature` |
| DynamoDB | `false` | Explicit `unsupported_feature`; no scan or key-value emulation |

SQLite and MySQL can express many latest-per-group reads with
`ROW_NUMBER() OVER (PARTITION BY ... ORDER BY ...)` in a derived table.
This proposal does not lower `.distinct_on(...)` to that form. A separate
portable winner-selection API would need its own comparison, null,
ordering, and model-hydration contract. It must not change the meaning of
a method named after a native PostgreSQL operator.

SQL drivers continue to receive `Operation::QuerySql`; no new Operation
variant is needed. PostgreSQL serialization must keep the filter, distinct
keys, ordering, and limit at the correct query level. For the unprojected
example, the observable SQL shape is:

```sql
SELECT DISTINCT ON (subject) id, kind, subject, slot, computed_at, body
FROM results
WHERE kind = $1
ORDER BY subject ASC, slot DESC, id DESC;
```

Aliases and quoting follow the existing serializer. Filters keep existing
bind parameter handling. Distinct keys use validated field references,
never interpolated strings. Full model columns and identity remain
available for ordinary decoding; projected reads decode only their
selected fields. No row-number helper column appears in a public result.

Adding a public capability field requires updating out-of-tree drivers
that construct `Capability` literals. They should leave it false or
inherit a false backend constant unless their backend and serializer
support native DISTINCT ON. A PostgreSQL protocol alone is not proof of
support. The implementation PR must document this source compatibility
impact and preserve the existing plain DISTINCT flag and behavior.

### Relationship to aggregates

The parallel design for [#421] owns scalar and grouped aggregate APIs,
GROUP BY, HAVING, and aggregate result types. This design returns one
source row per group. It does not introduce an aggregate result type or
an `arg_max`/`arg_min` operation. In particular, `.count()` on a
distinct-on query must not accidentally become `SELECT DISTINCT ON ...
COUNT(*)`; it is rejected until counting a derived winner set has an
accepted contract. `COUNT(DISTINCT field)` remains aggregate syntax and
is separate from DISTINCT ON.

Initially reject combining grouped aggregates or aggregate projections
with DISTINCT ON at the same query level. Neither design depends on the
other merging first. Any later composition must specify query nesting
and whether aggregation occurs before or after winner selection.

## Alternatives considered

**A portable latest-per-group method with window-function lowering.**
Useful for SQLite and MySQL users, but it needs a separate backend-neutral
contract. Rejecting unsupported native operators follows Toasty's current
design philosophy and keeps this proposal small enough to evaluate.

**Document a raw window query that hydrates models.** The existing raw
SQL helper can execute backend SQL, including a window query where the
backend supports it, but returns dynamic records rather than models.
Defining model identity, relations, and typed decoding for arbitrary SQL
is broader than this feature. This proposal makes no claim that such a
model-hydrating escape already exists.

**Join a grouped MAX back to the table.** Equal maxima can yield several
rows, and selecting unrelated columns alongside MAX is not a portable
way to select a complete row. It needs an additional winner rule.

**Deduplicate in Rust.** This reads every candidate, changes comparison
semantics, and makes limits depend on execution location. It is not an
implicit fallback for unsupported drivers.

## Open questions

- **Blocking acceptance:** is native PostgreSQL `.distinct_on(...)` the
  desired first step, or should a portable winner-selection API lead?
  This proposal chooses the native method with explicit capability errors.
- **Blocking implementation:** confirm how the new capability is exposed
  to out-of-tree drivers and describe any required source migration.
  The default here is a false flag in unsupported backend constants.
- **Deferrable:** cursor pagination, independently ordering the winners,
  and counting the winner set need a derived-query contract. Until then,
  the unsupported combinations above return explicit errors.

## Out of scope

- Public plain `.distinct()`: separate projection-deduplication semantics
  cover other parts of #425; the existing internal flag remains intact.
- GROUP BY, HAVING, and COUNT(DISTINCT): owned by the aggregate proposal.
- General window functions and arbitrary raw-SQL model adapters: broader
  typed query surfaces that need separate designs.
- Top N per group, embedded keys, relation keys, and expression keys:
  extend the initial field-based API only with separately defined behavior.
- Latest-per-group writes and row locks: this proposal covers reads only.

[#425]: https://github.com/tokio-rs/toasty/issues/425
[#421]: https://github.com/tokio-rs/toasty/issues/421
[PR #934]: https://github.com/tokio-rs/toasty/pull/934
[PostgreSQL SELECT reference]: https://www.postgresql.org/docs/current/sql-select.html#SQL-DISTINCT
