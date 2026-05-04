# Per-Call Column Projection

## Summary

A new `.select(...)` method on the query builder narrows a query's result
to a chosen subset of fields.  `.select(field)` returns `Vec<T>` for that
field's type; `.select((f1, f2, ...))` returns `Vec` of a tuple.  The
method composes with `.filter`, `.order_by`, `.limit`, `.first`, `.get`,
and `.count`.  It is the per-call companion to schema-level
`#[deferred]`: deferred sets a default for a column the model rarely
needs; `.select` shapes one call site.  Compilation rides the engine's
existing `Returning::Project` path; no new driver capability is required.

## Motivation

A query without projection control loads every non-deferred column.
That is the right default but the wrong cost shape for several common
patterns:

- A list view that needs `(id, name)` to render thousands of rows.
- A search-results page that wants the full record for the matched row
  but only a thumbnail for siblings.
- A GraphQL or JSON-API resolver that maps requested fields onto a
  tight projection.
- A migration script that wants every record minus a deprecated
  column, before dropping it.

`#[deferred]` ([design](deferred-fields.md), PR #793) addresses the
case where a column is *always* heavy.  It is a schema-level decision.
The cases above are call-site decisions on otherwise-eager columns, and
moving every such column to `Deferred<T>` flips the schema's polarity in
the wrong direction (unloaded by default, loaded on opt-in) for the
many call sites that do not need to skip it.

The deferred-fields design carved this gap out as a future feature in
its "Alternatives considered" and "Out of scope" sections.  This
document is that follow-on.  The pattern is well-established elsewhere:
Diesel's `select(...)`, ActiveRecord's `select`, Sequel's `select`,
JPQL constructor expressions, Prisma's `select`, SQLAlchemy's
`load_only`.  Toasty already has the engine plumbing
(`Returning::Project` accepts arbitrary column expressions, renamed in
PR #790); the missing piece is the user-facing surface and the
type-level shape of the result.

## User-facing API

### Selecting a single field

Pass a field handle.  The query yields a `Vec<T>` where `T` is that
field's Rust type:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key] #[auto] id: u64,
#     name: String,
#     email: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let names: Vec<String> = User::all()
    .select(User::fields().name())
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

Single-row terminators adapt to the projected type.  `.first()` returns
`Option<T>` and `.get()` returns `T`:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key] #[auto] id: u64,
#     name: String,
#     email: String,
# }
# async fn __example(mut db: toasty::Db, id: u64) -> toasty::Result<()> {
let name: String = User::filter_by_id(id)
    .select(User::fields().name())
    .get(&mut db)
    .await?;
# Ok(())
# }
```

### Selecting several fields

Pass a tuple to project several fields at once.  The result element
type is a tuple whose element types match in order:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key] #[auto] id: u64,
#     name: String,
#     email: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let pairs: Vec<(u64, String)> = User::all()
    .select((User::fields().id(), User::fields().name()))
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

Tuple impls cover sizes 1 through 10, matching the existing
`impl_into_expr_for_tuple!` ceiling in `toasty/src/stmt/into_expr.rs`.
For larger projections or named-field results, see "Open questions"
below.

### Composing with other clauses

`.select(...)` is orthogonal to `.filter(...)`, `.order_by(...)`,
`.limit(...)`, `.first()`, `.get()`, and `.count()`.  Filters reference
any field on the model, regardless of whether it appears in the
projection.  The compiled SQL ends up the natural shape, with the
projection in the SELECT list and the predicate in the WHERE clause:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key] #[auto] id: u64,
#     name: String,
#     email: String,
#     #[index] active: bool,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let active_emails: Vec<String> = User::all()
    .filter(User::fields().active().eq(true))
    .order_by(User::fields().name())
    .select(User::fields().email())
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

### Selecting deferred and embedded fields

A `#[deferred]` field selected through `.select(...)` is loaded
eagerly, exactly as if it had been included.  An embedded type field
selected through `.select(...)` returns the embed value:

```rust
# use toasty::{Model, Embed};
# #[derive(Debug, toasty::Embed)]
# struct Metadata { author: String, notes: String }
# #[derive(Debug, toasty::Model)]
# struct Document {
#     #[key] #[auto] id: u64,
#     title: String,
#     #[deferred] body: toasty::Deferred<String>,
#     metadata: Metadata,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let bodies: Vec<String> = Document::all()
    .select(Document::fields().body())
    .exec(&mut db)
    .await?;

let metas: Vec<Metadata> = Document::all()
    .select(Document::fields().metadata())
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

### Selecting relation fields

A relation field is a valid projection target.  Selecting one
projects to the relation's value shape: a `BelongsTo<T>` or `HasOne<T>`
field yields one element per parent row (`T`, or `Option<T>` for a
nullable association); a `HasMany<T>` field yields a `Vec<T>` per
parent row.  The engine reuses the include-subquery machinery already
used by `.include(...)`; the difference is the surrounding shape.
`.include(...)` returns a model record with the relation attached;
`.select(...)` returns the relation value directly.

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Author { #[key] #[auto] id: u64, name: String }
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key] #[auto] id: u64,
#     title: String,
#     #[belongs_to(key = author_id, references = id)]
#     author: toasty::BelongsTo<Author>,
#     author_id: u64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let authors: Vec<Author> = Post::all()
    .select(Post::fields().author())
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

Field paths through a relation are also supported.  Following a
`HasMany` adds a `Vec` layer; following a `BelongsTo` or `HasOne`
adds none (or `Option` if the association is nullable):

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct Author { #[key] #[auto] id: u64, name: String }
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key] #[auto] id: u64,
#     title: String,
#     #[belongs_to(key = author_id, references = id)]
#     author: toasty::BelongsTo<Author>,
#     author_id: u64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let author_names: Vec<String> = Post::all()
    .select(Post::fields().author().name())
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

Relations compose with tuple projections: `.select((Post::fields().
title(), Post::fields().author().name()))` yields `Vec<(String,
String)>`.  A `HasMany` traversal inside a tuple position contributes
its `Vec` layer at that position only; the outer projection stays a
flat tuple per parent row.

### What `.select(...)` does not accept

`.update(...)` and `.delete(...)` reject `.select(...)`.  Mutations
return either nothing or, in a future iteration, a `RETURNING`
projection covered by a separate design.

`.select(())` (empty tuple) is permitted and yields `Vec<()>`: one
unit value per matching row.  This degenerates to row counting, and
`.count()` is the more natural call site, but the empty tuple is not
specially rejected.

## Behavior

**Default projection.**  A query without `.select(...)` loads every
non-deferred field, exactly as today.  `.select(P)` replaces the
default model projection with the expression `P` reduces to.

**Result type.**  The mapping is mechanical: where the projection
expression `P` implements `IntoExpr<T>`, the call returns `Select<M,
T>` and `.exec()` yields `Vec<T>`.  Concrete shapes:

| Builder call                            | Projection `IntoExpr<T>` | Result of `.exec()`     |
|-----------------------------------------|--------------------------|-------------------------|
| (none)                                   | (n/a)                    | `Vec<Model>`            |
| `.select(field)`                         | `T = FieldType`          | `Vec<FieldType>`        |
| `.select((f1, ..., fn))`                 | `T = (T1, ..., Tn)`      | `Vec<(T1, ..., Tn)>`    |
| `.select(belongs_to_field)`              | `T = Related`            | `Vec<Related>`          |
| `.select(has_many_field)`                | `T = Vec<Related>`       | `Vec<Vec<Related>>`     |
| `.select(belongs_to_field.sub())`        | `T = SubFieldType`       | `Vec<SubFieldType>`     |
| `.select(has_many_field.sub())`          | `T = Vec<SubFieldType>`  | `Vec<Vec<SubFieldType>>` |

Field paths reduce by composition: a `HasMany` step lifts the eventual
element type into a `Vec`; a `BelongsTo` / `HasOne` step does not (or
adds `Option` if nullable).

`.first()` lifts the outer container to `Option<_>` and `.get()` lifts
to a single value, identical to today's mapping for `Select<M>`.
`.count()` ignores the projection entirely; it returns the row count.

**Type inference.**  The result element type follows from the `T`
parameter of the `IntoExpr<T>` impl chosen at the call site.  The
user can let inference do the work or annotate at the binding site as
the examples above do.

**Deferred fields.**  Selecting a `#[deferred]` field eagerly loads
it.  The result element type is `T`, not `Deferred<T>`; the unloaded
state has no place in a projection result.

**Nullable fields.**  `Option<T>` carries through unchanged.  A
`select(field_of_optional_string)` yields `Vec<Option<String>>`.

**Empty result.**  An empty projection result is `Vec` of length zero;
a single-row `.get()` on a missing record returns the existing
`record_not_found` error.

**Interactions.**

- *Embedded types.*  An embedded field maps to a single projection
  element of the embed type.  Selecting one field of an embed via the
  field-path chain (`Model::fields().embed().sub_field()`) is also
  permitted and yields the sub-field's type.
- *Relations.*  Relation fields and field paths through relations are
  valid projections; cardinality follows the rules described in
  "Selecting relation fields" above.  `.select(...)` and `.include(...)`
  are mutually exclusive on a single query.  The builder enforces this
  at the type level: `.include(...)` is exposed on the pre-projection
  `Select<M>` (which carries `Returning::Model`), and `.select(...)` is
  only callable on a `Select<M>` with no includes attached.  Once
  `.select(...)` returns `Select<M, T>`, neither method is exposed
  again.  A user who wants both a model-with-relations record and a
  separate projection should issue two queries.
- *`#[version]`.*  The version column is a normal field for
  projection purposes; selecting it returns the version number.  The
  returned tuple does not drive an instance update; only loaded model
  records do.
- *Pagination.*  Cursor encoding uses key fields, which the engine
  pulls separately from the projection.  The user is not required to
  include the key in the projection.
- *`#[sensitive]` (when that lands).*  Identical handling to other
  primitives; redaction is orthogonal to projection.

## Edge cases

**Selecting the same field twice.**  `.select((F, F))` is allowed; the
column is read once and the value is duplicated into the tuple
positions.  This matches SQL `SELECT col, col FROM ...` semantics and
costs nothing extra to support.

**Selecting a field on a model without it.**  Compile error;
`Model::fields()` only exposes what the model declares.

**Selecting through a relation traversal (`.todos().title()`).**
Supported.  See "Selecting relation fields" above for the cardinality
rules: each `HasMany` step adds a `Vec` layer, each `BelongsTo` /
`HasOne` step adds none (or `Option` for a nullable association).

**Selection on a streamed query** (when streaming lands, [#324]).
Streams yield the projected element type per item.  The projection is
established before the stream is opened; no new behavior to design
here.

**Boundary: tuple of size 11 or more.**  Compile error from a missing
`IntoExpr` impl.  The diagnostic message points the user at the
size-10 ceiling and the future work item.

**Concurrent modification.**  No new concurrency surface.  The query
returns whatever rows match at execution time; projection is purely a
column-list change.

## Driver integration

The feature is a column-list change on existing operations.  Most of
the work lives in the macro and the engine's lowering phase; drivers
see operations they already handle, with a different column list.

### Macro and trait surface

`#[derive(Model)]` generates a single new method on the query builder:

```rust
impl<M: Model> Select<M> {
    pub fn select<E, T>(self, projection: E) -> Select<M, T>
    where
        E: stmt::IntoExpr<T>,
        T: stmt::Decode,
    {
        // sets Returning::Project(projection.into_expr().untyped)
        // on the underlying statement
        ...
    }
}
```

There is no new `Project` trait.  `stmt::IntoExpr<T>` already exists
in `toasty/src/stmt/into_expr.rs` and already covers everything the
projection surface needs:

- Field handles for primitive, embedded, and deferred fields impl
  `IntoExpr<FieldType>` (or get the impl through their `stmt::Expr<T>`
  return value, which has a blanket `IntoExpr<T> for Expr<T>`).
- Tuples of arities 1-10 already impl `IntoExpr<(T0, ..., Tn)>`
  whenever each component does (`impl_into_expr_for_tuple!` macro
  invocation in `into_expr.rs`).  An impl for `()` is added so the
  empty-tuple projection compiles.
- `Option<T>` already lifts; nullable field handles flow through
  unchanged.

What the macro must add for projection to work end-to-end:

1. `IntoExpr<T>` impls for relation field handles, where `T` is the
   relation's value shape (`Author` for a `BelongsTo<Author>`,
   `Vec<Comment>` for a `HasMany<Comment>`, `Option<T>` for nullable
   associations).  These reduce to the same subquery expression
   `.include(...)` already constructs, wrapped in a typed `Expr<T>`.
2. `IntoExpr<T>` impls for relation-traversal field paths
   (`Post::fields().author().name()`), where `T` follows the
   cardinality rules above.
3. `stmt::Decode` impls for any new shape that does not already have
   one.  Primitives, tuples, `Option<T>`, and `Vec<T>` are existing
   `Decode` shapes; relation values decode through the same path
   `.include(...)` already uses for nested model records.

The 10-arity ceiling matches the existing `impl_into_expr_for_tuple!`
expansion and `IntoExpr`'s own conventions.  Raising it is a one-line
change to the macro invocation if a use case justifies the compile-time
cost.

Why a new trait was considered and rejected: an earlier draft proposed
a `Project<M>` trait with `type Output: Decode; fn columns(&self) ->
Vec<stmt::ExprReference>`.  Reusing `IntoExpr<T>` is strictly better:
it is the same trait already used everywhere else expressions enter
the API (filters, setters, comparisons), the tuple impls are already
in place, and there is no duplicated typing surface for users or
docs to cross-reference.  The only concrete cost is that
`stmt::IntoExpr` must be in scope at the call site, which it already
is for any user touching `.filter(...)` or `.eq(...)`.

### Engine

`Returning::Project(Expr)` and `Returning::Model { include }` already
exist (see `crates/toasty-core/src/stmt/returning.rs`).  Today, a
default `Select<M>` carries `Returning::Model { include: vec![] }` and
lowering expands that into the model's field list.  `.include(...)`
appends to the include vec; `.select(p)` replaces the entire
`Returning` with `Returning::Project(p.into_expr().untyped)`, using
the existing `Statement::set_returning_project` helper.

Lowering already handles `Returning::Project` (see PR #790's
`Returning` rename and PR #793's deferred-field path).  Decode already
follows whatever the typed expression resolves to.  Combining
`.select(...)` with `.include(...)` becomes a compile-time error in
the builder layer, since `Select<M>` and `Select<M, T>` (post-select)
are different types and `.include(...)` is only defined on the former.

No new builder field.  No `select_override`.  No change to the
planner.  No change to the operation graph.  No new MIR variant.

### SQL drivers

No change.  The shorter column list flows through `SELECT col1, col2,
... FROM ...` identically to how `#[deferred]` already produces a
shorter list.  No new dialect coverage; no new capability flag; no SQL
serializer changes beyond accepting the existing `Returning::Project`
shape.

### DynamoDB

No new `Operation` variant.  The DDB driver translates the engine's
column list into a `ProjectionExpression`.  `.select(F)` reduces that
expression to one attribute; `.select((F1, F2))` to two.  Non-key
selections on Query and Scan operations remain valid; key-field
selections continue to ride the existing key encoding.

### Backward compatibility for out-of-tree drivers

A driver that ignored projection lists in the past keeps working: the
engine continues to decode the columns it requested, and the
user-visible behavior degrades to "the projection returned more
columns than asked for, which the engine then discards."  Identical
compat outcome to the `#[deferred]` rollout.

## Alternatives considered

**Custom struct results via `#[derive(Project)]`.**  Define a struct
listing the projected fields and call `.select::<MyProjection>()`; the
macro generates the column list and decode.  Rejected as v1 because
it adds ceremony for the common ad-hoc case, where a tuple is named
at every call site by its element types.  Left in scope for a v2 once
the tuple form is stable; see open questions.

**Keyword-argument projection (`.select(name: true, email: true)`).**
Mimics Prisma via a `select!` macro.  Rejected because Toasty's
established field-handle convention (`Model::fields().name()`) carries
the value at every call site; a parallel naming approach would
fragment the API.

**`.pluck(F)` for the single-field case.**  An ActiveRecord-style
specialized form for "give me a `Vec<T>` of one field".  Rejected as
redundant: the size-1 tuple impl produces the same shape with one
method instead of two.

**`.exclude(F)` as the inverse of `.select(F)`.**  The deferred-fields
design names `.exclude(...)` alongside `.select(...)` as a future
feature.  After analysis, `.exclude(...)` does not fit Toasty's typed
model without a representational change:

- A non-deferred field of declared type `T` has no runtime "unloaded"
  state.  Returning a model record with such a field marked unloaded
  requires either a new `Lazy<T>` wrapper at every field site (a
  representational change to every model), a generated lite-struct
  type per call site (`UserExcludeName`, `UserExcludeEmail`,
  combinatorially many), or a runtime "stale" bit accessed through a
  non-default API path.
- A `#[deferred] Deferred<T>` field is already unloaded by default;
  `.exclude(...)` of it is a no-op against the existing schema-level
  semantics.
- A relation field is already lazy; `.exclude(...)` of it is also a
  no-op against the existing `.include(...)` semantics.

The conclusion is that the per-call complement to `#[deferred]` is
`.select(...)` alone in Toasty's typed-model setting.  Where Rails
or Sequel use `.exclude(...)` for an everything-but-X projection,
toasty's analog is `.select((all, fields, except, the, one))`, which
the user can construct explicitly.  Open questions below propose a
macro shorthand if call-site verbosity becomes a real complaint.

**`Select<M>` carries a phantom `Loaded` set in the type system.**
Encode the projection as a type-level set (`Select<M, {Name, Id}>`).
Rejected because the deferred-fields design already chose runtime
loaded-ness over phantom-set encoding for the same kind of state, and
introducing it here for projections would duplicate that decision in
incompatible ways.

**Reusing `.include(...)` for primitive fields.**  Make
`.include(field)` mean "ensure this field is loaded" for both
relations and deferred primitives.  Already designed: the
deferred-fields document specifies `.include(...)` for deferred
primitives.  No conflict with `.select(...)`, which constructs a
different result shape.

## Open questions

- **Tuple arity ceiling.**  The proposal inherits the existing
  10-tuple ceiling from `impl_into_expr_for_tuple!`.  Diesel goes
  higher (16 or 32 depending on feature flag).  Higher ceilings cost
  compile time on every consumer.  *Deferrable; raised by extending
  the macro invocation in `into_expr.rs` and is independent of this
  design.*

- **`#[derive(Project)]` for named-field results.**  Once tuples
  ship, a derived projection struct gives ergonomic named access for
  repeated projections.  Implementation-wise this is just an
  `IntoExpr<MyStruct> for MyStruct` impl emitted by a derive macro,
  composing the field-handle expressions into a record.  *Deferrable;
  not part of this design.*

- **Aggregates inside `.select(...)`.**  `.select((count(F), avg(G)))`
  would let projection cover aggregate queries (#421).  Out of scope
  here.  Compatibility with this design is automatic: aggregate
  expressions already produce typed `Expr<T>` values, which already
  satisfy `IntoExpr<T>` via the blanket `IntoExpr<T> for Expr<T>`
  impl.  No trait-shape change needed when aggregates land.

- **`.exclude(...)` shorthand via macro.**  If users push back on
  `.select((a, b, c, d, e))` for "everything except f" projections, a
  `select_except!` macro that expands at the call site to the explicit
  tuple is the cheapest answer.  *Deferrable; reopen if real users
  hit it.*

- **`RETURNING` projections on writes.**  `.update(...).select(...)`
  on PostgreSQL or `ReturnValues = ALL_NEW` on DynamoDB could echo a
  projected shape back from a mutation.  *Deferrable; covered by a
  separate write-projection design.*

- **Relation field-handle return type.**  Relation field handles
  currently return relation-specific builder types
  (`BelongsToBuilder`, `HasManyBuilder`, etc.).  Adding `IntoExpr<T>`
  impls for them is straightforward but the `T` choice for `HasMany`
  warrants a closer look during implementation: `Vec<T>` is the
  natural shape, but `List<T>` (toasty's wire-typed list) may compose
  better with the existing decode machinery.  *Blocking
  implementation: pick one and confirm `Decode` lines up.*

[#324]: https://github.com/tokio-rs/toasty/issues/324
[#421]: https://github.com/tokio-rs/toasty/issues/421

## Out of scope

- **`.exclude(...)` as a primary surface.**  Discussed in
  "Alternatives considered" above.  The conclusion is that the
  per-call complement to `#[deferred]` is `.select(...)` alone in
  Toasty's typed-model setting.  Reopen as a separate design if
  users argue it back in.

- **Named-field projection structs (`#[derive(Project)]`).**  Listed
  as open question; not in v1.

- **Aggregates and grouping.**  Covered separately by the existing
  roadmap entry for `COUNT` / `SUM` / `AVG` / `GROUP BY` (#421).

- **Subquery-as-column projections.**  Selecting a correlated
  subquery as one of the result fields, for example a child-count
  attached to each parent.  Covered by the future relation-aggregate
  surface; not part of column projection.

- **Mutation projections (`RETURNING` shaping).**  Useful but
  separable; the present design keeps reads and writes on disjoint
  surfaces.

- **Cross-row column reshaping.**  Pivot, unpivot, transpose: SQL
  features that have no natural ORM surface.  Stays an escape hatch
  for raw SQL (#93).

- **Selection on streaming results in this design.**  Covered as a
  no-op consequence of streaming once that design lands; tracked
  under #324, not here.
