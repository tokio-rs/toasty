# Grouped counts and extrema

## Summary

Add typed grouped queries for `COUNT(*)`, `MIN`, and `MAX`, with `GROUP BY`
and `HAVING`. A dashboard over a job queue can request counts by kind and
state, or the earliest pending deadline by kind, without raw SQL or loading
all jobs. SQL drivers execute these queries natively; DynamoDB rejects them
explicitly. This document proposes behavior, not a shipped API.

## Motivation

[#421](https://github.com/tokio-rs/toasty/issues/421) tracks aggregate queries.
Its statement that there is no public count API is stale: current main has
`Query<List<M>>::count() -> Query<u64>` and generated builders expose it.
For example, `Job::all().count().exec(&mut db).await?` already returns a
scalar count. Field and tuple projections through `.select(...)` also exist.
This proposal preserves those APIs and addresses the missing grouped
count/min/max surface, grouping, and group predicates. It does not complete
all the aggregates requested by #421.

A job queue dashboard needs these two queries:

```sql
SELECT kind, state, COUNT(*) FROM jobs GROUP BY kind, state;
SELECT kind, MIN(run_after) FROM jobs
WHERE state = 'pending' GROUP BY kind;
```

Fetching every job transfers and retains far more data than the summary
requires. Raw SQL can express the queries, but gives up the model builder's
field and result typing.

## User-facing API

### Count rows in each group

Call `.group_by(...)` on a model list query. Pass one direct scalar field or
a nonempty tuple of direct scalar fields, then use `.select(...)` to choose
the group keys and aggregates to return. Examples below are proposed syntax;
assume `kind` and `state` are `String`, and `run_after` is `i64`.

```rust
use toasty::stmt::{count, max, min};

let counts: Vec<(String, String, u64)> = Job::all()
    .group_by((Job::fields().kind(), Job::fields().state()))
    .select((Job::fields().kind(), Job::fields().state(), count()))
    .exec(&mut db)
    .await?;
```

`count()` constructs an aggregate expression for `COUNT(*)`; it differs
from the existing terminal `.count()` that counts matching model rows.
The grouped builder has no terminal `.count()`, avoiding ambiguity between
counting source rows and counting result groups.

Call `.filter(...)` before `.group_by(...)` to restrict source rows:

```rust
let deadlines: Vec<(String, Option<i64>, Option<i64>)> = Job::all()
    .filter(Job::fields().state().eq("pending"))
    .group_by(Job::fields().kind())
    .select((
        Job::fields().kind(),
        min(Job::fields().run_after()),
        max(Job::fields().run_after()),
    ))
    .exec(&mut db)
    .await?;
```

`min(field)` and `max(field)` return `Expr<Option<T>>` for a field of type
`T` or `Option<T>`. They ignore absent values and return `None` when a
group has no present values. An optional input contributes one `Option`
layer, not `Option<Option<T>>`. Even required inputs have optional aggregate
results, keeping a consistent type as nullable fields are introduced. The
constructor bound must describe a direct scalar field with an associated
nonoptional value type, rather than infer an arbitrary `T` from `IntoExpr<T>`;
this keeps optional-input inference unambiguous and excludes relation paths.

### Filter and order groups

Use `.having(...)` for predicates over group keys and aggregates. Each call
ANDs its predicate with earlier calls. It is available before or after
`.select(...)`; selection changes the result shape, not the group context.

```rust
let busy: Vec<(String, u64)> = Job::all()
    .group_by(Job::fields().kind())
    .having(count().gt(10_u64))
    .select((Job::fields().kind(), count()))
    .order_by(count().desc())
    .order_by(Job::fields().kind().asc())
    .limit(20)
    .exec(&mut db)
    .await?;
```

Aggregates use the existing typed expression comparisons. This proposal
also adds `.asc()`/`.desc()` to typed scalar expressions, producing the
same ordering-expression type as field paths. Repeated `.order_by(...)`
calls append tie-breakers, as on existing queries. Repeating `count()` denotes
the same aggregate over the same source group; users do not need SQL aliases. Group
ordering and limits
apply after HAVING. Supply an order when which groups make the limit
matters. `.offset(...)` requires `.limit(...)`, as for existing queries.
There is no implicit model primary-key tie-breaker: a group is not a model.

### Builder and expression boundaries

The new `GroupedQuery<M, R = ()>` retains its source model and group context
separately from its result type. `.group_by(...)` yields the unselected
`GroupedQuery<M>`; `.select(...)` yields `GroupedQuery<M, List<T>>`, whose
`.exec(...)` returns `Vec<T>` using the existing executor and tuple decoding.
The unselected builder cannot execute. Tuple arities follow the existing
`IntoExpr` support. Grouped results are read-only and cannot be converted
implicitly back into model queries.

Aggregate constructors produce ordinary typed expressions, so existing
comparison and tuple composition stay useful. Execution validates aggregate
placement and group membership, including expressions created through raw
AST access. A typed expression alone does not prove that it is valid SQL.
An aggregate used in an ordinary row projection or WHERE clause is rejected;
this release does not add ungrouped aggregate expressions to `.select(...)`.

Only model list builders gain `.group_by(...)`, including generated
`all()` and filtered finders. Single-row, already projected, and scalar
count builders do not gain it. Grouped builders expose HAVING, ordering,
limit/offset, selection, and execution, but no row `.filter(...)`,
`.include(...)`, `.update(...)`, `.delete(...)`, `.paginate(...)`, or second
`.group_by(...)`. Existing callers require no migration.

## Behavior

Execution filters source rows, forms groups, computes aggregates, filters
groups with HAVING, orders the remaining groups, and applies limit/offset.
A selected expression may be a declared group key, an aggregate over a
direct field of the source model, or a literal. HAVING and ordering may
compose those expressions using supported scalar operators. Every field
reference outside an aggregate must be a declared group key. Aggregates
need not appear in the selected result to be used in HAVING or ordering.

The initial supported field domain is required or singly optional scalar
strings, integers, and supported date/time values; boolean fields can also
be group keys. `MIN`/`MAX` accept the ordered members of that domain, not
booleans. Each field's storage mapping must support its database operation
and decoding to the declared Rust type. Unsupported mappings return
`Error::unsupported_feature`; they are never converted to strings to make
an aggregate succeed. Aggregate comparisons involving absence follow the
[Option design](option-semantics.md), including `is_none()`/`is_some()`.

Invalid grouping or expression placement returns `Error::invalid_statement`
before dispatch: empty or repeated group keys, a bare ungrouped field,
nested aggregates, aggregates in WHERE or group keys, and keys from another
model.
Type-incompatible expressions fail to compile where existing expression
bounds can enforce them; malformed raw AST statements are validated too.
Unsupported backend or field shapes return `Error::unsupported_feature`.
Database execution errors retain their existing error categories. A count
that cannot be represented as `u64`, or an extremum that cannot decode as
its field type, returns `Error::invalid_type_conversion`, never a truncated
or wrapped value. Queries fail as a whole on a decoding error.

A source query carrying ordering, a limit/offset, cursor pagination, or
includes is invalid for grouping, even if its builder type allows the call.
Return `invalid_statement` at execution rather than discard or reinterpret
those clauses. To bound result groups, put ordering and limits on the
grouped builder. Aggregating a bounded source subset needs an explicit
subquery design and is deferred. Relation-traversing source builders and
relation paths in keys or aggregates are unsupported in this first release;
ordinary filters on the source model's own fields remain supported when
they can be evaluated entirely in the single SQL statement. A predicate
requiring client-side post-filtering is unsupported: grouping must never
run before a remaining row predicate.

A grouped query is one read-only SQL statement, not a sequence of reads
per group. It participates in the existing transaction and executor
interfaces. Visibility and isolation are those of the backend and the
caller's transaction; the builder adds no stronger snapshot guarantee.

## Edge cases

No matching rows produce an empty `Vec`, not a synthetic group containing
zero. An existing group always has a positive `COUNT(*)` before HAVING,
even when all its aggregate field values are absent. `MIN`/`MAX` then return
`None`; zero, empty strings, and other present values remain present.
`COUNT(*)` includes rows whose keys or aggregate inputs are absent.

Absent scalar keys form a single group for each combination of other keys.
The returned key retains its `Option<T>` type. SQL grouping equivalence
and extrema for present values use native storage-type and collation rules,
not Rust hashing or `Ord`. Case-insensitive collations may combine distinct
Rust strings; the representative key spelling is backend-selected. Extrema
may also select any database-equivalent tied spelling. No normalization is
promised across databases.

Without ordering, result order is unspecified. Native collation ties in
ordered keys can also leave group order nonunique. Limits do not bound
source rows examined, and multiple queries with offset pagination may
observe changes from concurrent writers. Cursor pagination is deferred
until it can encode a complete, unique group identity; the model key is not
that identity. `MIN(run_after)` returns a value, not the complete job with
that deadline, and ties do not identify a winning job.

Embedded values, enums, relation keys, document/collection fields, nested
options, floating-point and decimal fields, arbitrary computed group keys,
and aggregates over relation paths are outside the initial field domain.
Reject these with `unsupported_feature`; they need separate decisions about
mapping, presence, equality, and ordering before being advertised.

## Driver integration

### Capability and operation contract

Use the existing `Capability::sql()` distinction for this initial surface:
all currently supported SQL dialects provide GROUP BY, HAVING, COUNT,
MIN, and MAX. No new capability field, required `Driver` method, or
`Operation` variant is proposed. SQL drivers continue to receive
`Operation::QuerySql` and return streamed projected rows. The shared
serializer gains the grouped clauses and extrema expressions; drivers
must decode each column in projection order, with counts as checked `u64`
values and extrema using the input field's mapping and optional result.

The public grouped-query additions preserve existing model builders.
Out-of-tree drivers using a supported SQL dialect receive native grouped
SQL; those without SQL capability reject grouped statements before any
operation is dispatched. This does not grant grouping support to a future
SQL dialect automatically; that dialect must establish serialization and
field support. A SQL-compatible backend with restricted aggregate support
must reject unsupported queries rather than emulate them.

No existing trait implementation or `Capability` struct literal needs new
members. Extending public statement AST structs or function enums can affect
out-of-tree struct literals and exhaustive matches. The implementation PR
must identify those source compatibility changes and follow the project's
versioning policy; this design does not claim all public AST consumers will
be unaffected. No AST layout is fixed by this guide-level proposal.

### SQL serialization and backend behavior

The observable statement shape is:

```sql
SELECT <keys and aggregates> FROM <source>
WHERE <row predicate>
GROUP BY <declared keys>
HAVING <group predicate>
ORDER BY <group ordering> LIMIT <limit> OFFSET <offset>;
```

Omit optional clauses when absent. Emit each nonaggregate projected field
explicitly in GROUP BY; do not rely on functional dependencies or permissive
bare-column behavior. Refer to actual key or aggregate expressions in
HAVING and ordering rather than SELECT aliases or positional ordinals.
Keep the backend's identifier quoting and value binding; user values in
WHERE/HAVING remain parameters. No user-controlled SQL fragments are added.

| Driver/backend | Grouped-query contract |
| --- | --- |
| PostgreSQL | Native SELECT; typed extrema and checked integer counts. |
| SQLite | Native SELECT; bare ungrouped columns are rejected. |
| MySQL / MariaDB | Native SELECT valid with `ONLY_FULL_GROUP_BY`; native collation. |
| Turso | SQLite-dialect contract for embedded, sync, and serverless modes. |
| DynamoDB | `unsupported_feature` before any data operation; no aggregate encoding. |

DynamoDB cannot compute these groups server-side. Its scalar count
facilities do not produce counts or extrema per arbitrary key tuple. An
implicit paginated scan would change cost, memory, collation, and consistency
properties. This design deliberately offers no such fallback, even when
a source predicate selects one partition. Applications needing DynamoDB
summaries can maintain summary items or explicitly aggregate a bounded
read themselves. This proposal makes no claim to extend existing scalar
`.count()` support on DynamoDB.

The implementation's conformance coverage must establish WHERE/HAVING
separation, empty inputs, multiple keys, all-absent extrema, typed tuple
results, collation-equivalent keys, strict MySQL grouping, invalid clauses,
count conversion failures, and unchanged scalar count behavior. DynamoDB
coverage must prove rejection with zero data requests, not merely an error
after reading items. These are acceptance requirements for implementation,
not executable examples supplied by this design-only PR.

## Alternatives considered

**Reuse model `Query` for every builder state.** This makes mutations,
includes, model pagination, and terminal `.count()` easy to inherit with
ambiguous or invalid group semantics. A distinct grouped builder shares
expression and result decoding conventions while keeping those boundaries
explicit.

**Aggregate in memory on every backend.** It could serve DynamoDB, but
silently turns a small dashboard response into an unbounded read. Native
collation and transaction visibility also cannot be reproduced by Rust
hashing and sorting. Explicit rejection keeps that cost decision with users.

**Require a result struct or closure-bound group expression DSL.** Both
could strengthen typing or name outputs, but introduce another projection
vocabulary before the existing tuple form is exhausted. Runtime validation
of field membership is consistent with raw AST escape hatches; a future
struct projection can compose with the grouped builder.

**Implement all of #421 at once.** SUM/AVG and distinct or filtered counts
need additional numeric widening, overflow, and nullability decisions.
Count/min/max solve the concrete dashboard cases without deciding those
unrelated contracts.

## Open questions

- **Blocking acceptance:** Do maintainers prefer explicit DynamoDB rejection
  or an opt-in client aggregate API? The proposal chooses rejection; an
  opt-in API would need separate cost and consistency bounds.
- **Blocking acceptance:** Is a distinct grouped builder with runtime
  membership validation the preferred surface? The proposal chooses it;
  stricter group-bound typing would change the illustrative API.
- **Blocking implementation:** Which public AST extensions fit the next
  release's compatibility policy? Keep the guide-level behavior above and
  document any source breaks for out-of-tree AST users before implementation.

## Out of scope

- Implementation: it follows only after this design and roadmap PR merges.
- SUM/AVG, COUNT(field), DISTINCT/FILTER aggregates, and ungrouped extrema:
  these require separate expression and result-type contracts under #421.
- Latest or oldest complete row per group, DISTINCT ON, and window functions:
  selecting the extreme value does not select a row; see
  [#425](https://github.com/tokio-rs/toasty/issues/425).
- Joins, relation traversal, embedded/document grouping, and computed keys:
  they require explicit cardinality and value-mapping contracts.
- Rollups, cubes, grouping sets, materialized summaries, and client-side
  DynamoDB aggregation: these are different grouping and cost surfaces.
- Group cursor pagination: it needs group identity and ordering semantics.
