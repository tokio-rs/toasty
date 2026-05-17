# Static SQL Values

## Summary

`stmt::Expr::Value` carries a render mode: `Bound` (the default)
extracts the value as a bind parameter; `Static` inlines it as a
literal in the SQL text.  Today, every scalar value is extracted
unconditionally; schema-known constants (`LIMIT 10`, enum
discriminants, `#[auto]` defaults) waste bind slots even when the
value is fixed across every call to that query shape.  This design
lets the engine mark a leaf `Static` at construction time so it
survives `extract_params` and lands inline in the rendered SQL.

Tagging is structural: the engine and the model derive set `Static`
for the small set of leaves it makes sense for; user-supplied values
keep the default `Bound` and pass through the bind path unchanged.
There is no new user-facing surface.

## Motivation

[#237] asks whether Toasty can distinguish values that should be
escaped from values that should not.  The framing that survives
discussion is narrower: drop the trust axis entirely (every value
reaches the driver through bind parameters today, so escaping is the
driver's concern), and keep the orthogonal question of *rendering*.
"Render this as part of the SQL text" versus "render this as a bind
parameter" is a real choice the engine makes implicitly today;
making it explicit at the AST level fixes three concrete things.

1. **Bind-parameter inflation.**
   `crates/toasty/src/engine/extract_params.rs:91`'s `extract_values`
   replaces every scalar `Expr::Value` with `Expr::Arg(n)`.  `LIMIT 10`
   on the user-facing API becomes `LIMIT $1` with `[I64(10)]` as a
   bind, not `LIMIT 10`.  The same applies to schema-derived enum
   discriminants and `#[auto]` defaults.  Plan caches in PostgreSQL,
   MySQL, and SQLite key on SQL text; baking these schema-fixed values
   into the SQL text expands the universe of query shapes the cache
   recognizes as the same.

2. **Runtime-only literal invariants.**
   `crates/toasty/src/engine/verify.rs:165` panics if `LIMIT` or
   `OFFSET` is anything other than `Expr::Value(Value::I64(_))`.  The
   simplifier and lowering passes hold this invariant by hand;
   nothing at the type level prevents a future contributor from
   threading a user-supplied `Expr::Arg` into the cursor pagination
   path.  PR #703 ("gate simplification rules on expression
   stability") is a recent example of this hand-rolled invariant
   work.

3. **DDL inline-literal contexts.**
   `crates/toasty-sql/src/serializer/value.rs:30` inlines `Value`
   nodes directly into SQL text for DDL (CHECK constraints, column
   DEFAULTs).  Today the inline path assumes the value reached it
   from a code path that excluded user input; the assumption is held
   by the call structure, not by the type.  An explicit `Static` tag
   on `Expr::Value` lets the inline path require `Static` and reject
   anything else.

[#237]: https://github.com/tokio-rs/toasty/issues/237

## User-facing API

This design adds no new user-facing API.  Existing queries compile
and execute unchanged.

The only observable change is in the rendered SQL.  A query like
`User::all().limit(10).exec(&mut db)` that today emits

```sql
SELECT id, name FROM users LIMIT ?1
-- params: [I64(10)]
```

emits

```sql
SELECT id, name FROM users LIMIT 10
-- params: []
```

after this design lands.  Users who inspect SQL through driver hooks
or who care about prepared-statement plan caching see the change;
every other user sees nothing.

## Behavior

- **Default render mode.**  Every `Expr::Value` constructed through
  the user-facing builder API (`.filter`, `.eq`, `.update`, etc.) is
  `Bound`.  Every leaf the macro-generated model code or the engine
  synthesizes from schema-fixed information (enum discriminants,
  sentinel records, variant-encoded `None` markers, `#[auto]`
  defaults, `LIMIT n` and `OFFSET n` arguments from the builder) is
  `Static`.

- **Tag representation.**  `Expr::Value` becomes
  `Expr::Value { value: Value, render: ValueRender }`, with
  `pub enum ValueRender { Bound, Static }`.  Existing matches on
  `Expr::Value(v)` switch to `Expr::Value { value: v, .. }` (a one-
  character refactor per arm).  Toasty's house style allows public
  fields on AST structs; `ValueRender` is a public enum with public
  variants, so call sites read naturally without constructor
  ceremony.

- **Engine passes.**  `Simplify`, `fold`, and `lower` preserve the
  render mode through rewrites.  When two leaves combine into a new
  leaf (e.g. constant folding `Value(I64(2)) + Value(I64(3))` into
  `Value(I64(5))`), the result is `Static` iff both inputs were
  `Static`.  Mixed-mode operations produce `Bound`.  The lattice
  matches PR #703's stability gate, so the simplifier's rule
  predicate composes cleanly: a rewrite that requires a literal
  input now requires a `Static` literal input.

- **Extract-params behavior.**  `extract_values` extracts `Bound`
  scalars to `Expr::Arg(n)` placeholders and leaves `Static` scalars
  inline.  The serializer's `Expr::Value` arm emits the static
  leaves as SQL literals through the existing `value::to_sql` path
  (which already escapes `String` correctly).

- **Verify-pass behavior.**  `LIMIT` and `OFFSET` checks tighten
  from "is `Value::I64`" to "is `Static` `Value::I64`".  A future
  contributor who threads a user-supplied value into pagination
  fails the verify pass at the AST level rather than escaping into
  the serializer.

- **Driver interface.**  Unchanged.  Drivers continue to receive a
  SQL string and `Vec<TypedValue>`; the render mode is consumed
  entirely upstream of the driver call.

## Edge cases

- **Mixed-mode binary ops.**  `Static(I64(10)) + Bound(I64(x))`
  evaluates to a `Bound` result.  The simplifier already cannot
  constant-fold across the bind boundary, so the existing "rewrite
  only if both children are stable" rule subsumes this.  The
  lattice rule is documented explicitly so a future contributor
  does not invert it.

- **Records and lists.**  A record leaf carries a per-field render
  mode, not a record-level mode, because the same record can mix
  schema-supplied keys with user-supplied values (a primary-key
  record where the table-name discriminant is `Static` but the
  user-supplied row id is `Bound`).  The list case is symmetric.

- **DDL paths.**  Inline-literal serialization in
  `toasty-sql/src/serializer/value.rs` continues to escape `String`
  literals defensively even when the leaf is `Static`.  The render
  mode changes which paths reach the inline serializer (`Bound`
  leaves are rejected upstream), not how the inline serializer
  behaves once it is reached.

- **Round-trip values from `RETURNING`.**  Values read back from the
  database via `RETURNING` and re-fed into a follow-on statement
  (the PR-#812 lower-sub-statement path) are classified `Bound` in
  iteration 1.  An aggressive optimization that classifies
  `Bool`/integer/`Uuid` returns as `Static` is deferred so the
  type-tag plumbing can land first.

- **Custom user types.**  If a user implements `IntoExpr` for a
  custom type, the resulting `Value` is constructed `Bound`.
  Iteration 1 has no opt-in API for marking a user-supplied value
  `Static`.

## Driver integration

For SQL drivers, **nothing changes**.  Drivers receive a SQL string
and a `Vec<TypedValue>`.  A driver that previously received
`("SELECT ... LIMIT ?1", [TypedValue { value: I64(10), ty: I64 }])`
now receives `("SELECT ... LIMIT 10", [])`.  Every supported SQL
driver already accepts inline literals in `LIMIT`/`OFFSET` and CHECK
positions.

For the DynamoDB driver, the render mode is invisible: DynamoDB's
`AttributeValue` encoding has no SQL-text-vs-bind distinction, and
the driver continues to receive each value through the same
`TypedValue` channel as today.

Out-of-tree drivers see no API break: `ValueRender` is not exposed
through the `Driver` trait or the `Operation` enum.  A driver that
wants to take advantage of the tag (a driver that emits raw SQL for
migrations and could safely inline more shapes, for example) reads
it from `Expr::Value` directly.

**Plan-cache stability.**  Inlining schema-fixed leaves does not
pollute prepared-statement caches the way Entity Framework Core's
pre-9 inlining heuristic did (see "Prior art" below).  Toasty's
`Static` set is a structural subset of "constants known at query
construction time, fixed across calls to the same query shape"
(`LIMIT`/`OFFSET` literals from the builder, schema-derived enum
discriminants, `#[auto]` defaults).  These do not vary per call for
a given query shape, so PostgreSQL's plan cache, MySQL's prepared-
statement cache, and SQLite's statement cache continue to hit.
`Bound` leaves remain bind parameters and continue to vary the
parameter list, not the SQL text.

## Alternatives considered

- **Tag on `Expr::Value` (recommended).**  One new field on the
  existing `Value` variant.  Minimal AST surface change; existing
  matches keep compiling with a `, ..` addition.  Specified above.

- **Tag inside `stmt::Value` itself.**  Bloats every `Value` use
  site, including the `TypedValue` records sent to drivers and the
  arithmetic the simplifier runs on `Value` directly.  Rejected:
  the render decision is meaningful at the expression layer, not
  at the value layer.

- **Parallel `Expr` variants (`Expr::StaticValue`).**  Most
  explicit at call sites; worst for migration churn.  Every
  existing `Expr::Value` match arm has to decide which variant it
  cares about.  Rejected: the ergonomic loss for drive-by readers
  is not worth the marginal extra compile-time discrimination over
  the field-tag approach.

- **Trust/untrust framing.**  An earlier draft of this doc framed
  the tag as `Trusted`/`Untrusted` and proposed inlining trusted
  leaves while binding untrusted leaves.  This makes a category
  error: every value Toasty emits today reaches the driver through
  a bind parameter (escaping is the driver's concern, not
  Toasty's), and even "trusted" values benefit from being bound
  when the query shape is reused under a prepared-statement plan
  cache.  The decision is not about whether the value is safe; it
  is about whether the value belongs in the SQL text.  Trustedness
  stays implicit and structural (every `Static` value Toasty
  constructs is, by construction, schema-known), but the tag
  encodes the rendering decision, not the provenance.

- **Provenance on `TypedValue` only.**  Keeps the AST unchanged and
  attaches a tag to bind-parameter records emitted by extract-
  params.  Useful for query-log middleware but does not unlock
  inlining (the value has already been extracted by the time the
  tag exists).  Rejected as the primary mechanism; could ship
  alongside the AST tag as a downstream-driver convenience.

- **Status quo (no tag).**  Every value continues to be a bind
  parameter.  Cost is real but bounded; the type-level verify-pass
  tightening cannot land.  This is the null hypothesis the design
  has to beat.

### Prior art

**JOOQ (Java).**  JOOQ exposes the rendering decision explicitly:
`DSL.val(x)` creates a bind parameter, `DSL.inline(x)` creates an
inlined SQL literal, and `DSL.inlined(condition|field|query)`
inlines a sub-tree.  A global `Settings.statementType =
STATIC_STATEMENT` inlines everything.  JOOQ's documentation notes
that "all inlined bind values [are] properly escaped to avoid SQL
syntax errors and SQL injection," confirming that inlining stays
responsible for hygiene; `inline` does not mean "trusted to be
safe."  The polarity (default-bind, opt-in-inline) matches the
direction Toasty takes, with the difference that Toasty sets the
tag automatically from schema knowledge rather than expecting the
user to call `inline()`.

**Slick (Scala).**  `Rep[T]` is a query-time expression;
`ConstColumn[T]` is a literal value or a compiled-query parameter.
`ConstColumn` exists because compiled queries distinguish runtime
parameters from literal constants at plan time; mixing the two
arbitrarily defeats the compiled-query plan cache.  Structural
distinction in the same family as JOOQ's, motivated by plan-cache
soundness rather than security.

**Entity Framework Core (C#).**  EF Core's expression-tree compiler
treats `ConstantExpression` (a literal in the C# tree) differently
from captured-variable references (funcletized into constants but
tagged for parameterization).  Historically only captured variables
became `@p0`/`@p1` binds; literal constants inlined directly.  Each
distinct inlined constant produced a distinct query plan on SQL
Server, polluting the plan cache.  EF Core 9 (2024) rolls out
"Better Query Parameterization" to address this by parameterizing
more `ConstantExpression` cases.  Toasty avoids the failure mode
because the `Static` set is a structural subset of "constants
fixed across calls to the same query shape"; schema-derived
literals do not vary per call.

**Ecto (Elixir).**  Ecto inverts the polarity of every other ORM
in this survey: literals embedded directly in a query expression
inline into the generated SQL, and external or dynamic values
must be pinned with `^` to be interpolated as bind parameters.
The literals Ecto inlines without a pin are a closed set:
integers, floats, booleans, binaries, strings, atoms (excluding
`nil` and the boolean atoms), and arrays of those.  Anything
else, or any value that is not a literal at the source-code
level, must be pinned: `from u in User, where: u.age > ^age`.
The pin doubles as a place to attach type information via
`type/2`.

Ecto is the closest prior art at the user-facing API level; the
others either hide the distinction (Diesel, SeaORM, EF Core) or
expose it as a lower-level affordance for advanced users (JOOQ,
Slick).  The Elixir community has lived with `^` for a decade
with mostly positive reception: the pin makes the parameterization
boundary visually obvious, which helps both code review and SQL-
injection auditing.  The downside cited most frequently is the
additional ceremony for cases where the value is in fact a
constant at the call site (`where: u.status == ^@active_status`
instead of `where: u.status == @active_status`); Ecto contributors
have considered an "auto-pin module attributes" relaxation but
have not landed it.  Toasty's "default-bind, engine-set static
for schema-derived literals" direction avoids this ceremony
entirely because the engine and the macro know which leaves are
schema-derived without help from the user.

## Open questions

- **Audit of simplifier rules that synthesize new leaves.**  The
  lattice rule ("`Static` combines to `Static` iff both inputs are
  `Static`") needs to apply to every site in `engine/simplify.rs`
  and `engine/fold.rs` that builds an `Expr::Value` from one or
  more inputs.  PR #703's stability-gate audit covered most of
  these, but a fresh sweep is needed before the verify-pass
  tightening lands.  *Blocking implementation.*

- **Macro-generated model code surface.**  Whether `toasty-macros`
  emits `Expr::Value { value, render: ValueRender::Static }`
  literals inline, or routes through a `pub(crate)` helper such as
  `stmt::Expr::static_value(value)`.  The helper keeps generated
  code shorter; the inline form keeps generated code transparent
  for `cargo expand` consumers.  *Deferrable; resolves during
  implementation.*

- **Compatibility with the per-call column-projection design
  (#811).**  `.select(p)` lowers `p.into_expr().untyped` into the
  parent's `Returning::Project`; the projection expression is
  built from schema-known field handles, so the resulting
  `Expr::Value` leaves (if any are introduced by future projection-
  rewrite rules) should default to `Static`.  Confirm during
  implementation that the `IntoExpr<T>` impls on field handles
  thread `Static` through their generated `Expr::Value`
  constructions.  *Deferrable.*

## Out of scope

- **Aggressive RETURNING classification.**  A per-shape allow-list
  (`Bool`, integer, `Uuid` are `Static` on round-trip;
  `String`/`Bytes`/list/record are `Bound`) is deferred to a
  follow-on iteration so the type-tag plumbing can land under its
  own review focus.

- **Public opt-in for `Static`.**  An API surface that lets a user
  mark an external constant as `Static` at the call site (rare;
  a constant in an external configuration crate, for example).
  Deferred until a real use case emerges; the iteration-1 design
  has no public construction site for `Static` values outside the
  engine and the model derive.

- **Raw SQL string fragments.**  Toasty's typed builder remains the
  only entry point; a future raw-SQL escape hatch is an orthogonal
  feature.

- **Migrating CHECK-constraint emission to require `Static`.**  The
  inline-literal serializer in `toasty-sql/src/serializer/value.rs`
  continues to escape defensively, regardless of render mode.  This
  design unlocks the option of requiring `Static` on inline-only
  paths but does not exercise it.

- **Cross-driver SQL-text caching.**  A render-aware AST makes
  literal inlining stable for cache lookup, but the cache itself is
  a separate feature.

- **Wholesale newtype-ID introduction (`TableId`, `ColumnId`,
  etc.).**  Out of scope per the project's existing house-style
  decision.
