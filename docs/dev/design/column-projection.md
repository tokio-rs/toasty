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

Tuple impls cover sizes 1 through 12, matching the standard library's
tuple trait conventions.  For larger projections or named-field
results, see "Open questions" below.

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

### What `.select(...)` does not accept

`.select(...)` rejects relation fields at compile time.  Relation
loading already has a dedicated surface (`.include(...)`); allowing
relations on `.select(...)` would force the result type to bridge two
shapes (a tuple for projections, a model+children record for relations)
and would duplicate the include machinery.  The compile error points
the user at `.include(...)`.

`.select(())` (empty tuple) is also a compile error.  At least one
field is required; an "empty projection" reduces to `.count()` for
collections or a no-op for single-row queries.

`.update(...)` and `.delete(...)` reject `.select(...)`.  Mutations
return either nothing or, in a future iteration, a `RETURNING`
projection covered by a separate design.

## Behavior

**Default projection.**  A query without `.select(...)` loads every
non-deferred field, exactly as today.  `.select(P)` replaces the
default column list with `P`'s columns.

**Result type.**  The mapping is mechanical:

| Builder call                                         | Result of `.exec()`        |
|------------------------------------------------------|----------------------------|
| (none)                                               | `Vec<Model>`               |
| `.select(F)`                                         | `Vec<F::Type>`             |
| `.select((F1, F2, ..., Fn))`                         | `Vec<(F1::Type, ..., Fn::Type)>` |

`.first()` lifts the outer container to `Option<_>` and `.get()` lifts
to a single value, identical to today's mapping for `Select<M>`.
`.count()` ignores the projection entirely; it returns the row count.

**Type inference.**  The result element type follows from the
projection's `Project::Output` associated type, so the user can let
inference do the work or annotate at the binding site as the examples
above do.

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
Rejected at compile time with the same message as a direct relation
selection: relations are loaded via `.include(...)`, not `.select(...)`.

**Selection on a streamed query** (when streaming lands, [#324]).
Streams yield the projected element type per item.  The projection is
established before the stream is opened; no new behavior to design
here.

**Boundary: tuple of size 13 or more.**  Compile error from a missing
trait impl.  The diagnostic message points the user at the size-12
ceiling and the future work item.

**Concurrent modification.**  No new concurrency surface.  The query
returns whatever rows match at execution time; projection is purely a
column-list change.

## Driver integration

The feature is a column-list change on existing operations.  Most of
the work lives in the macro and the engine's lowering phase; drivers
see operations they already handle, with a different column list.

### Macro

`#[derive(Model)]` generates a single new method on the query builder:

- `Select<M>::select<P>(self, projection: P) -> Select<M, P::Output>`
  where `P: Project<M>`.

The `Project` trait is doc-hidden codegen support:

```rust
#[doc(hidden)]
pub trait Project<M: Model> {
    type Output: stmt::Decode;
    fn columns(&self) -> Vec<stmt::ExprReference>;
}
```

Implementations are generated for:

1. Every field handle on the model (one impl per field, with
   `Output = FieldType`).
2. Tuple impls for sizes 1 through 12 (composition over the
   per-field impls).

Compile-time rejection of relation handles is enforced by *not*
generating a `Project` impl for them.  The diagnostic uses
`#[diagnostic::on_unimplemented]` to point users at `.include(...)`.

The decode path on the result side reuses the `stmt::Decode` machinery
that already underlies model record decoding; tuples and primitives
are existing `Decode` shapes.

### Engine

Lowering already walks the field list and emits a column projection
in `Returning::Project`.  The change is to consult one extra input
from the user-facing query:

- **`select_override: Option<Vec<ExprReference>>`**, when present,
  replaces the default model projection wholesale.

When `select_override` is present, the lowering bypasses the
model-shape decode at exec time: the result is decoded as the
projected types, not as a model record.  When absent, lowering
proceeds exactly as today.

No change to the planner.  No change to the operation graph.  No new
MIR variant.

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

- **Tuple arity ceiling.**  The proposal sets the ceiling at 12.
  Diesel goes higher (16 or 32 depending on feature flag).  Higher
  ceilings cost compile time on every consumer.  The 12-impl ceiling
  is consistent with the standard library's own conventions.
  *Deferrable; can be raised by adding more impls in a follow-on PR.*

- **`#[derive(Project)]` for named-field results.**  Once tuples
  ship, a derived projection struct gives ergonomic named access for
  repeated projections.  *Deferrable; not part of this design.*

- **Aggregates inside `.select(...)`.**  `.select(count(F), avg(G))`
  would let projection cover aggregate queries (#421).  Out of scope
  here, but the `Project` trait should leave room in its
  `columns()` return for aggregate expressions so the future
  expansion does not require a breaking change.
  *Blocking acceptance: confirm the trait shape leaves the door open
  for aggregate-expression columns.*

- **`.exclude(...)` shorthand via macro.**  If users push back on
  `.select((a, b, c, d, e))` for "everything except f" projections, a
  `select_except!` macro that expands at the call site to the explicit
  tuple is the cheapest answer.  *Deferrable; reopen if real users
  hit it.*

- **`RETURNING` projections on writes.**  `.update(...).select(...)`
  on PostgreSQL or `ReturnValues = ALL_NEW` on DynamoDB could echo a
  projected shape back from a mutation.  *Deferrable; covered by a
  separate write-projection design.*

- **Field-path projections.**  `.select(Document::fields().metadata().author())`
  selects an embed sub-field.  The proposal supports this, but the
  trait shape that allows arbitrary field paths versus only top-level
  fields needs one more pass.  *Blocking implementation: confirm the
  field-path-chain typing.*

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
