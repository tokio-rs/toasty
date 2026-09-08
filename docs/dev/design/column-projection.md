# Completing Column Projection

## Summary

`.select(...)` narrows a list query's result to a chosen subset of fields:
`.select(field)` yields `Vec<T>` for that field's type, `.select((f1, f2))`
yields a `Vec` of tuples, and a relation field yields the relation's value.
Several parts of that surface are missing.  A field path through a `HasMany`
(`Post::fields().comments().body()`) type-checks but returns the wrong data.  A
nullable association projects as if it were non-null.  `.get()`, `.count()`,
and `.paginate(...)` are unreachable once `.select(...)` has changed the element
type, and `.select(())` does not compile.  `.select(...)` and `.include(...)`
are not mutually exclusive.  On DynamoDB, projection reads no fewer attributes,
and relation and embed projections are unavailable.  This design covers those
gaps.

## Motivation

- Reading one column off each child row has no working spelling.
  `Post::all().select(Post::fields().comments().body())` type-checks as
  `Vec<Vec<String>>`, then projects the whole child record and decodes column
  zero — a decode error, or a panic when the traversal sits in a tuple
  position.
- A projected lookup by key must be spelled `.select(f).one().exec(..)`,
  `.count()` must be moved ahead of `.select(...)`, and pagination cannot be
  combined with projection at all.  These are ordering rules the user has to
  learn from compiler errors rather than from the shape of the builder.
- A nullable association projects to the target type.  A row with a null
  foreign key fails at runtime (`record not found`, or `cannot convert Null to
  String` through a path) instead of yielding `None`.
- DynamoDB users project to cut read cost and do not cut it.  The driver
  fetches full items and drops attributes client-side.

## User-facing API

### Field paths through a `HasMany`

Following a relation adds that relation's cardinality layer to the result
type.  A `BelongsTo` or `HasOne` step adds nothing; a `HasMany` step adds a
`Vec`:

```rust
let comment_bodies: Vec<Vec<String>> = Post::all()
    .select(Post::fields().comments().body())
    .exec(&mut db)
    .await?;
```

The engine projects the named column of the child subquery, not the whole child
record.  In a tuple position, the traversal contributes its `Vec` layer at that
position only; the outer projection stays a flat tuple per parent row:

```rust
let rows: Vec<(String, Vec<String>)> = Post::all()
    .select((Post::fields().title(), Post::fields().comments().body()))
    .exec(&mut db)
    .await?;
```

A declared `#[has_many(via = comments.article.title)]` field is a different
construct — a schema-level path with a scalar terminal — and is unaffected.

### Nullable associations

A nullable single association projects to `Option<T>`, and so does a field path
through one.  A row whose foreign key is null yields `None`:

```rust
let owners: Vec<Option<Owner>> = Note::all()
    .select(Note::fields().owner())
    .exec(&mut db)
    .await?;

let owner_names: Vec<Option<String>> = Note::all()
    .select(Note::fields().owner().name())
    .exec(&mut db)
    .await?;
```

### Single-row terminators

`.select(...)` is available on the single-row builders, and `.get()` returns
the projected type directly:

```rust
let name: String = User::filter_by_id(id)
    .select(User::fields().name())
    .get(&mut db)
    .await?;
```

When no row matches, `.get()` returns `record_not_found` — the same error the
unprojected `.get()` returns, not a decode failure on a null column.

### Counting a projected query

`.count()` is callable after `.select(...)`.  It ignores the projection and
returns the row count:

```rust
let n: u64 = User::all()
    .select(User::fields().name())
    .count()
    .exec(&mut db)
    .await?;
```

### Pagination

`.select(...)` and `.paginate(...)` compose in either order.  Cursor encoding
uses key fields, which the engine pulls separately from the projection, so the
user is not required to include the key:

```rust
let mut pages = User::all()
    .select((User::fields().id(), User::fields().name()))
    .paginate(50);
```

### Empty projection

`.select(())` yields `Vec<()>`: one unit value per matching row.  This
degenerates to row counting, and `.count()` is the more natural call site, but
the empty tuple is not specially rejected.

### `.select(...)` and `.include(...)`

The two are mutually exclusive on a single query, enforced at the type level.
`.include(...)` is exposed on the pre-projection builder, and neither method is
exposed on the projected builder.  Both orders compile today: `.include(...)`
before `.select(...)` silently discards the include, and `.include(...)` after
`.select(...)` attaches an include that nothing reads.  A user who wants both a
model-with-relations record and a separate projection issues two queries.

## Behavior

**Result type.**  The remaining shapes, on top of those `.select(...)` already
produces:

| Builder call                            | Projection `IntoExpr<T>`      | Result of `.exec()`         |
|-----------------------------------------|-------------------------------|-----------------------------|
| `.select(has_many_field.sub())`          | `T = Vec<SubFieldType>`       | `Vec<Vec<SubFieldType>>`    |
| `.select(nullable_relation)`             | `T = Option<Related>`         | `Vec<Option<Related>>`      |
| `.select(nullable_relation.sub())`       | `T = Option<SubFieldType>`    | `Vec<Option<SubFieldType>>` |
| `.select(())`                            | `T = ()`                      | `Vec<()>`                   |

Field paths reduce by composition: a `HasMany` step lifts the eventual element
type into a `Vec`; a `BelongsTo` or `HasOne` step adds `Option` when the
association is nullable and nothing otherwise.

**Terminators.**  `.get()` lifts the container to a single value; `.count()`
replaces the projection with the row count.

**Error cases.**  `.get()` on a query matching no row returns
`record_not_found`, regardless of the projection.  A null association projects
to `None` rather than an error.

**Interactions.**

- *Relations.*  `.select(...)` and `.include(...)` are mutually exclusive, per
  the type-level rule above.
- *Pagination.*  Cursor encoding uses key fields the engine pulls separately
  from the projection.
- *`#[sensitive]` (when that lands).*  Identical handling to other primitives;
  redaction is orthogonal to projection.

## Edge cases

**Boundary: tuple of size 11 or more.**  Compile error from a missing
`IntoExpr` impl.  The diagnostic names the size-10 ceiling and the follow-up
work item, instead of surfacing a bare unsatisfied-trait-bound error.

## Driver integration

### Macro and trait surface

The projection bound is the existing `stmt::IntoExpr<T>`.  Three impls are
missing:

1. `IntoExpr<Vec<T>>` for relation-traversal field paths through a `HasMany`,
   resolving to the child subquery projected to the named column rather than to
   the whole child record.
2. `IntoExpr<Option<T>>` for nullable single associations and for field paths
   through them.  The relation field struct emits only the non-null impl;
   lowering already passes nulls through.
3. `IntoExpr<()>`, so the empty-tuple projection compiles.  The tuple macro
   expansion in `toasty/src/stmt/into_expr.rs` starts at arity 1.

The builder gains `.select(...)` on the single-row query builders, `.count()`
and `.paginate(...)` after projection, and the type-level exclusion between
`.select(...)` and `.include(...)`.

### Engine

No new operation, capability flag, or MIR variant.  The `HasMany` traversal
must project the named column of the child subquery; today it projects the
whole child record, so decode reads column zero and a traversal in a non-zero
tuple position indexes past the end of the merged row in
`toasty/src/engine/exec/nested_merge.rs`.

### SQL drivers

No change.  The traversal fix is a column-list change on a subquery the
serializer already emits.

### DynamoDB

No new `Operation` variant.  The driver translates the engine's column list
into a `ProjectionExpression` on Query and Scan, so a projected query reads
fewer attributes.  Today it builds the request with filter and attribute
setters only and picks the requested columns out of the full item after the
fetch.  `.select(F)` reduces the expression to one attribute; `.select((F1,
F2))` to two; key-field selections ride the existing key encoding.

Relation and embed projection is SQL-only.  `.select(...)` of a `HasOne`,
`HasMany`, or `via` relation, and of an embed sub-field, needs DynamoDB
coverage; the suite gates all four on `requires(sql)` today, with `BelongsTo`
the only relation case running against DynamoDB.

### Backward compatibility for out-of-tree drivers

A driver that ignores projection lists keeps working: the engine decodes the
columns it requested and discards the rest.  Identical compat outcome to the
`Deferred<T>` rollout.

## Alternatives considered

**Custom struct results via `#[derive(Project)]`.**  Define a struct listing
the projected fields and call `.select::<MyProjection>()`; the macro generates
the column list and decode.  Deferred rather than rejected: the tuple form
covers the ad-hoc case without ceremony, and the derive composes with it later.
See open questions.

**`.exclude(F)` as the inverse of `.select(F)`.**  Does not fit Toasty's typed
model without a representational change:

- A non-deferred field of declared type `T` has no runtime "unloaded" state.
  Returning a model record with such a field marked unloaded requires either a
  new `Lazy<T>` wrapper at every field site, a generated lite-struct type per
  call site (`UserExcludeName`, `UserExcludeEmail`, combinatorially many), or a
  runtime "stale" bit accessed through a non-default API path.
- A `Deferred<T>` field is already unloaded by default, and a relation field is
  already lazy.  `.exclude(...)` of either is a no-op against the existing
  semantics.

Where Rails or Sequel use `.exclude(...)` for an everything-but-X projection,
Toasty's analog is `.select((all, fields, except, the, one))`.  A macro
shorthand is proposed below if call-site verbosity becomes a real complaint.

## Open questions

- **Tuple arity ceiling.**  The projection surface inherits the 10-tuple
  ceiling from `impl_into_expr_for_tuple!`.  Diesel goes higher (16 or 32
  depending on feature flag).  Higher ceilings cost compile time on every
  consumer.  *Deferrable; raised by extending the macro invocation in
  `into_expr.rs` and independent of this design.*

- **`#[derive(Project)]` for named-field results.**  A derived projection
  struct gives ergonomic named access for repeated projections.
  Implementation-wise this is an `IntoExpr<MyStruct> for MyStruct` impl emitted
  by a derive macro, composing the field-handle expressions into a record.
  *Deferrable; not part of this design.*

- **Aggregates inside `.select(...)`.**  `.select((count(F), avg(G)))` would
  let projection cover aggregate queries ([#421]).  Out of scope here, and
  compatible by construction: aggregate expressions produce typed `Expr<T>`
  values, which satisfy `IntoExpr<T>` through the blanket impl.  No
  trait-shape change needed when aggregates land.

- **`.exclude(...)` shorthand via macro.**  If users push back on
  `.select((a, b, c, d, e))` for "everything except f" projections, a
  `select_except!` macro that expands at the call site to the explicit tuple is
  the cheapest answer.  *Deferrable; reopen if real users hit it.*

- **`RETURNING` projections on writes.**  `.update(...).select(...)` on
  PostgreSQL or `ReturnValues = ALL_NEW` on DynamoDB could echo a projected
  shape back from a mutation.  *Deferrable; covered by a separate
  write-projection design.*

[#324]: https://github.com/tokio-rs/toasty/issues/324
[#421]: https://github.com/tokio-rs/toasty/issues/421

## Out of scope

- **`.exclude(...)` as a primary surface.**  Discussed in "Alternatives
  considered" above.  Reopen as a separate design if users argue it back in.

- **Named-field projection structs (`#[derive(Project)]`).**  Listed as an open
  question; not part of this design.

- **Aggregates and grouping.**  Covered separately by the existing roadmap
  entry for `COUNT` / `SUM` / `AVG` / `GROUP BY` ([#421]).

- **Subquery-as-column projections.**  Selecting a correlated subquery as one
  of the result fields, for example a child-count attached to each parent.
  Covered by the future relation-aggregate surface.

- **Mutation projections (`RETURNING` shaping).**  Useful but separable; reads
  and writes stay on disjoint surfaces.

- **Cross-row column reshaping.**  Pivot, unpivot, transpose: SQL features with
  no natural ORM surface.  Stays an escape hatch for raw SQL (#93).

- **Projection on streaming results.**  Streams yield the projected element
  type per item, established before the stream is opened.  Tracked under
  [#324], not here.
