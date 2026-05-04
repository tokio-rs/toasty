# Trusted vs Untrusted Values

> **Status: Draft for review.**  Reflects the current code state at
> `main` (`10322f23`) and the design discussion on [#237].  Iteration-1
> scope and remaining open questions are listed at the end.

## Summary

Toasty's statement AST does not distinguish between a value that the
user wrote (`.filter(name.eq(input))`) and a value that the engine or
schema synthesized (`LIMIT 10`, an enum discriminant, an `#[auto]`
default).  Both reach the simplifier, lowering, and extract-params
phases as `stmt::Expr::Value(stmt::Value)`, and the extract-params
phase parameterizes both indiscriminately.  This proposal threads a
*trust* tag through `Expr::Value` so trusted leaves stay inline as
SQL literals while untrusted leaves continue to be extracted as bind
parameters.

The primary win is correctness: paths that today rely on a runtime
`matches!(.., Expr::Value(Value::I64(_)))` invariant (`LIMIT`,
`OFFSET`, schema discriminants) get a type-level guarantee that the
leaf is schema-known.  The secondary win is performance: hot-path
queries with constant `LIMIT`s and bool/discriminant filters emit
fewer bind placeholders, which gives the database planner more
constant-folding room and shrinks the `Vec<TypedValue>` that drivers
move per call.

**Iteration-1 scope.**  This design covers the type-tag plumbing on
`Expr::Value`, selective behavior in `extract_params`, the
verify-pass tightening for `LIMIT` and `OFFSET`, and adjustments to
the simplifier's constant-folding rules so the trust tag propagates
correctly.  RETURNING-derived values stay `Untrusted` (always re-bind
through extract-params); a per-shape allow-list that classifies
`Bool`/integer/`Uuid` returns as `Trusted` is deferred to a follow-on
iteration.

## Motivation

[#237] (Carl, A-engine, C-feature) frames the problem as: "we assume
that every value in the statement could be untrusted and should be
escaped.  In practice, this is not necessarily true."  In the current
code, the literal "escaping" path is narrower than the issue text
suggests, but the underlying observation generalizes.  Concretely:

1. **Bind-parameter inflation in DML.**
   `crates/toasty/src/engine/extract_params.rs:91`'s `extract_values`
   replaces every scalar `Expr::Value` with an `Expr::Arg(n)`
   placeholder, regardless of source.  A query like
   `User::all().limit(10)` emits `LIMIT $1` with `[I64(10)]` as a
   bind, not `LIMIT 10`.  Multiplied across nested includes, batched
   updates, and the synthesized child statements PR #812 just added,
   schema-known literals account for a measurable fraction of every
   driver round-trip's bind list.

2. **Runtime-only literal invariants.**
   `crates/toasty/src/engine/verify.rs:165` panics if `LIMIT` or
   `OFFSET` is anything other than `Expr::Value(Value::I64(_))`.  The
   simplifier and lowering passes hold this invariant by hand;
   nothing at the type level prevents a future contributor from
   threading a user-controlled `Expr::Arg` into the cursor pagination
   path.  PR #703 ("gate simplification rules on expression
   stability") is a recent example of this hand-rolled invariant
   work.

3. **DDL inline-literal escaping.**
   `crates/toasty-sql/src/serializer/value.rs:30` inlines `Value`
   nodes directly into SQL text for DDL contexts (CHECK constraints,
   column DEFAULTs).  Single quotes in `String` are doubled; many
   variants `todo!()`.  This path assumes the value came from the
   schema (and so cannot contain attacker-controlled bytes), but
   nothing in the type guarantees it.  A future feature that lets a
   user supply a CHECK expression would silently break the
   assumption.

4. **Audit clarity for the AST.**
   Anyone reading a `stmt::Statement` at the boundary, e.g. a future
   query optimizer, a metrics middleware, or a driver implementor,
   currently has no signal about which leaves are user-controlled.
   A trust tag turns a runtime invariant into a documentation
   surface.

[#237]: https://github.com/tokio-rs/toasty/issues/237

## User-facing API

The trust tag lives on `stmt::Expr::Value` and is set by the engine
and the model derive.  User code in the guide does not change.
Schema-defined literals (`#[auto]` defaults, enum discriminants,
`LIMIT n` and `OFFSET n` arguments from the builder) are constructed
`Trusted`.  Everything that flows in through the typed builder API
(`.filter`, `.eq`, `.update`, etc.) is `Untrusted`.

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key] #[auto] id: u64,
#     name: String,
# }
# async fn __example(mut db: toasty::Db, input: String) -> toasty::Result<()> {
// `LIMIT 10` is trusted; `name == input` is untrusted.
let users: Vec<User> = User::all()
    .filter(User::fields().name().eq(input))
    .limit(10)
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

The user sees no new method, no new trait, no new macro.  The engine
emits `LIMIT 10` inline and `name = $1` with `[String(input)]` as a
bind.

A future opt-in escape hatch (a `trusted!` macro that lets a user
mark an external constant as trusted at the call site) is sketched
in [Out of scope](#out-of-scope) and reserved for a follow-on if a
real use case emerges.

## Behavior

- **Default trust state.**  Every leaf the macro-generated model
  code emits as a constant (enum discriminants, sentinel records,
  variant-encoded `None` markers, `#[auto]` defaults, schema-supplied
  builder arguments like `LIMIT n` and `OFFSET n`) is constructed
  `Trusted`.  Every leaf that originates from a user `.eq`/`.filter`
  call, or from a custom `IntoExpr` impl, is `Untrusted`.

- **Tag representation.**  `Expr::Value` becomes
  `Expr::Value { value: Value, trust: Trust }`, where
  `pub enum Trust { Trusted, Untrusted }`.  This is one new field on
  the existing variant; existing `match Expr::Value(v) =>` arms keep
  compiling once they destructure as `match Expr::Value { value: v, .. } =>`.
  Toasty's house style allows public fields on AST structs and the
  `Trust` enum is a public type with public variants, so call sites
  read naturally without constructor ceremony.

- **Engine passes.**  `Simplify`, `fold`, and `lower` preserve the
  trust tag through rewrites.  When two leaves combine into a new
  leaf (e.g. constant folding `Expr::Value(I64(2)) + Expr::Value(I64(3))`
  into `Expr::Value(I64(5))`), the result is `Trusted` iff both
  inputs were `Trusted`.  Mixed-trust ops produce `Untrusted`.  This
  matches PR #703's stability gate, so the simplifier's existing
  rule predicate composes cleanly: a rewrite that requires a literal
  input now requires a `Trusted` literal input.

- **Extract-params behavior.**  `extract_values` becomes selective:
  it extracts `Untrusted` scalars to `Expr::Arg(n)` placeholders and
  leaves `Trusted` scalars inline.  The serializer's `Expr::Value`
  arm emits the trusted leaves as SQL literals using the existing
  `value::to_sql` path (which already escapes `String` correctly).

- **Verify-pass behavior.**  `LIMIT` and `OFFSET` checks tighten
  from "is `Value::I64`" to "is `Trusted` `Value::I64`".  A future
  contributor who threads a user-controlled value into pagination
  fails the verify pass at the AST level rather than escaping into
  the serializer.

- **No change to the `Driver` interface.**  Drivers continue to
  receive `Vec<TypedValue>` for binds; the trusted leaves never
  reach the driver because they are inlined upstream.

## Edge cases

- **Mixed-trust binary ops.**  `Trusted(I64(10)) + Untrusted(I64(x))`
  evaluates to an untrusted result.  The simplifier already cannot
  constant-fold across the bind boundary, so the existing rule
  ("rewrite only if both children are stable") subsumes this.  The
  lattice rule is documented explicitly so a future contributor does
  not invert it.

- **Records and lists.**  A record leaf carries a per-field trust
  tag, not a record-level tag, because the same record can mix
  schema-supplied keys with user-supplied values (e.g. a primary-key
  record where the table-name discriminant is trusted but the user
  supplied the row id).  The list case is symmetric: each element
  carries its own trust state.

- **DDL paths.**  Inline-literal serialization in
  `toasty-sql/src/serializer/value.rs` continues to escape `String`
  literals defensively even when the leaf is trusted.  The trust
  tag changes which paths reach the inline serializer (untrusted
  leaves are rejected upstream), not how the inline serializer
  behaves once it is reached.  Defense in depth.

- **Round-trip values from RETURNING.**  Values read back from the
  database via `RETURNING` and re-fed into a follow-on statement
  (the PR-#812 lower-sub-statement path) are classified
  `Untrusted` in iteration 1.  This forces them through
  `extract_params` and re-binds them as parameters in the child
  statement.  The aggressive optimization (inline `Bool`, integer,
  `Uuid` returns; re-bind `String`/`Bytes`) is deferred to a
  follow-on so the type-tag plumbing can land first under its own
  review focus.

- **Custom user types.**  If a user implements `IntoExpr` for a
  custom type, the resulting `Value` is constructed `Untrusted`.
  There is no opt-in escape hatch in iteration 1; users who want a
  custom-type leaf inlined route through the existing typed builder
  surface, where engine-emitted literals are already trusted.

## Driver integration

For SQL drivers, **nothing changes**.  Drivers receive a SQL string
and a `Vec<TypedValue>`; the trust tag is consumed entirely upstream
of the driver call.  A driver that previously received
`("SELECT ... LIMIT ?1", [TypedValue { value: I64(10), ty: I64 }])`
now receives `("SELECT ... LIMIT 10", [])`, which every supported
SQL driver already accepts.

For the DynamoDB driver, the trust tag is also invisible: DynamoDB's
`AttributeValue` encoding does not have a SQL-style injection
surface, and the driver continues to receive each value through the
same `TypedValue` channel as today.

Out-of-tree drivers see no API break: trust is not exposed in the
`Driver` trait or the `Operation` enum.  A driver that wants to take
advantage of the distinction (for example, a driver that emits raw
SQL for migrations and could safely inline more shapes) reads the
tag from `Expr::Value` directly.

**Plan-cache stability.**  Inlining schema-known literals does not
pollute prepared-statement caches the way EF Core's pre-9 inlining
did (see "Prior art" below).  Toasty's trusted set is a structural
subset of "constants known at query construction time, fixed across
calls"; the same query shape produces the same SQL text on every
call, so PostgreSQL's plan cache, MySQL's prepared-statement cache,
and SQLite's statement cache continue to hit.  Untrusted leaves
remain bind parameters and continue to vary the parameter list, not
the SQL text.

## Alternatives considered

- **Tag at the `Expr::Value` level (recommended).**  Adds one field
  to the `Value` arm of `Expr`.  Minimal AST surface change.
  Existing `match Expr::Value(v)` arms keep compiling with a
  one-character refactor (`{ value: v, .. }`).  This is the design
  this document specifies.

- **Tag inside `stmt::Value` itself.**  Bloats every `Value` use
  site, including the `TypedValue` records sent to drivers and the
  arithmetic the simplifier runs on `Value` directly.  Rejected:
  the trust distinction is meaningful at the AST/expression layer,
  not at the value layer; pushing it into `Value` forces every
  consumer to thread it whether they care or not.

- **Parallel variants on `Expr` (`Expr::Literal` for trusted,
  `Expr::Value` for untrusted).**  Most explicit at call sites;
  worst for migration churn.  Every existing `Expr::Value` match
  arm has to decide which variant it cares about.  Rejected: the
  ergonomic loss for drive-by readers is not justified by the
  marginal extra compile-time discrimination over the field-tag
  approach.

- **Newtype wrappers `Trusted<T>` / `Untrusted<T>` on the
  user-facing builder API, lowering to a single internal
  representation.**  This is the literal reading of the issue
  title.  Adds the most type-driven discipline at the API
  boundary but does not survive past the lowering step, so the
  engine still has to reconstruct the distinction on the AST or
  lose it.  Rejected as a primary representation; potentially
  used as the spelling for a future opt-in `trusted!` macro.

- **Provenance on `TypedValue` only.**  Keeps the AST unchanged and
  attaches trust to bind-parameter records emitted by extract-params.
  Useful for query-log middleware but does not unlock literal
  inlining (because the value has already been extracted by the
  time the tag exists).  Rejected as the primary mechanism; could
  ship alongside the AST tag as a downstream-driver convenience.

- **Status quo (no distinction).**  Every value continues to be a
  bind parameter.  Cost is real but bounded; correctness wins are
  forfeited.  This is the null hypothesis the design has to beat.

### Prior art

**Diesel (Rust).**  Diesel routes user values through the
`AsExpression<T>` trait, which has three behaviors: return self for
built-in expression types (column references, `now`), perform
implicit coercion (e.g., `now` as both `Timestamp` and
`Timestamptz`), or wrap the value as a bind parameter via the type's
`ToSql` impl.  The trust distinction is structural, not nominal:
built-in expression types are always trusted, and types that bottom
out in `ToSql` always become bind parameters.  Diesel does not
expose a runtime-tagged trust state; the choice is made at compile
time by which `AsExpression` impl gets selected.  Toasty borrows the
structural-rather-than-nominal posture but adds a runtime tag
because Toasty's AST is dynamically typed (`stmt::Value` is an enum)
where Diesel's is statically typed (per-column types in the schema).

**SeaORM (Rust).**  SeaORM uses a `sea_orm::Value` enum analogous
to Toasty's `stmt::Value`, and its query builder methods (`filter`,
`having`, etc.) accept anything that converts to `Value`.  No trust
tag; every value reaches the database driver as a bind parameter
via the underlying `sea_query` translator.  This is the do-nothing
baseline.

**JOOQ (Java).**  JOOQ exposes the trust distinction explicitly at
the API level: `DSL.val(x)` creates a bind parameter (`?`
placeholder) and `DSL.inline(x)` creates an inlined SQL literal,
with both methods available for every supported type.  Users can
also flip the global `Settings.statementType` to `STATIC_STATEMENT`
to inline all bind values, or apply
`DSL.inlined(condition/field/query)` to inline a sub-tree.  JOOQ's
documentation notes that "all inlined bind values [are] properly
escaped to avoid SQL syntax errors and SQL injection," confirming
that the inlining path remains responsible for hygiene; `inline`
does not mean "trusted to be safe," it means "rendered inline with
the same escape rules the inline-literal serializer uses for any
value."  This polarity (default-bind, opt-in-inline) matches the
recommended Toasty direction, with the difference that Toasty sets
the tag automatically from schema knowledge rather than expecting
the user to call `inline()`.

**Slick (Scala).**  Slick distinguishes `Rep[T]` (a query-time
expression that may bind to columns or other expressions) from
`ConstColumn[T]` (a literal value or a parameter of a compiled
query).  `ConstColumn` exists because compiled queries need to know
which leaves are runtime parameters and which are literal
constants; mixing the two arbitrarily would defeat the
compiled-query plan cache.  This is a structural distinction in the
same family as JOOQ's `val`/`inline`, motivated by query-plan
caching rather than by security.

**Entity Framework Core (C#).**  EF Core's expression-tree compiler
distinguishes `ConstantExpression` (a literal value baked into the
C# expression tree) from captured-variable references (which are
funcletized into constants but tagged for parameterization).
Historically, only captured variables became `@p0`/`@p1` bind
parameters; literal constants in the tree inlined directly into the
SQL.  This caused real problems on SQL Server: each distinct
inlined constant produced a distinct query plan, polluting the plan
cache and degrading performance under load.  EF Core 9 (2024) is
rolling out "Better Query Parameterization" to address this by
parameterizing more `ConstantExpression` cases automatically.  The
Toasty design avoids this failure mode because Toasty's trusted set
is structurally a subset of "constants known at query construction
time, fixed across calls" (LIMIT and OFFSET literals from the
builder, schema-derived enum discriminants, `#[auto]` defaults).
These do not vary per call for a given query shape, so plan caches
still see stable SQL text.

**Ecto (Elixir).**  Ecto inverts the polarity of every other ORM
in this list: literals embedded directly in a query expression are
inlined into the generated SQL, and external or dynamic values
must be marked with the `^` (pin) operator to be interpolated as
bind parameters.  The literals Ecto inlines without a pin are
restricted to a closed set: integers, floats, booleans, binaries,
strings, atoms (excluding `nil` and the boolean atoms), and arrays
of those.  Anything else, or any value that is not a literal at
the source-code level, must be pinned: `from u in User, where: u.age > ^age`.
The pin is also where users supply type information via the
`type/2` cast: `type(^age, :integer)`.

For Toasty, Ecto is the most directly relevant prior art because
it is the only listed ORM that surfaces the trust distinction at
the user-facing API level; the others either hide it (Diesel,
SeaORM, EF Core) or expose it as a lower-level affordance for
advanced users (JOOQ's `inline()`/`val()`, Slick's `ConstColumn`).
The Elixir community has lived with `^` for a decade with mostly
positive reception: the pin makes the parameterization boundary
visually obvious, which helps both code review and SQL-injection
auditing.  The downside cited most frequently is the additional
ceremony for cases where the value is in fact a constant at the
call site (`where: u.status == ^@active_status` instead of
`where: u.status == @active_status`); Ecto contributors have
considered an "auto-pin module attributes" relaxation but have not
landed it.  This downside is what a future Toasty `trusted!` macro
(see [Out of scope](#out-of-scope)) would address; with the
recommended invisible-to-the-user design the question does not
arise, because Toasty's polarity is "default-bind, with engine-set
trust for schema-derived literals," not "default-inline, with
user-set pin for parameters."

## Open questions

- **Macro-generated model code surface.**  Whether `toasty-macros`
  emits `Expr::Value { value, trust: Trust::Trusted }` literals
  inline, or routes through a `pub(crate)` helper such as
  `stmt::Expr::trusted_value(value)`.  The helper keeps generated
  code shorter; the inline form keeps generated code transparent
  for `cargo expand` consumers.  *Deferrable; resolves during
  implementation.*

- **Audit of simplifier rules that synthesize new leaves.**  The
  lattice rule ("untrusted poisons trusted") needs to be applied
  to every site in `engine/simplify.rs` and `engine/fold.rs` that
  builds an `Expr::Value` from one or more inputs.  PR #703's
  stability-gate audit covered most of these, but a fresh sweep
  is needed before the verify-pass tightening lands.  *Blocking
  implementation.*

- **Compatibility with the in-flight column-projection design
  (#811).**  `.select(p)` lowers `p.into_expr().untyped` into the
  parent's `Returning::Project`; the projection expression is built
  from schema-known field handles, so the resulting `Expr::Value`
  leaves (if any are introduced by future projection-rewrite
  rules) should default to `Trusted`.  Confirm during
  implementation that the `IntoExpr<T>` impls on field handles
  thread `Trusted` through their generated `Expr::Value`
  constructions.  *Deferrable.*

## Out of scope

- **Aggressive RETURNING classification.**  A per-shape allow-list
  (`Bool`, integer, `Uuid` are trusted on round-trip; `String`,
  `Bytes`, list, record are re-bound) is deferred to a follow-on
  iteration so the type-tag plumbing can land under its own
  review focus.  The follow-on PR will need to demonstrate that
  no existing call site relies on a string returned via
  `RETURNING` being re-bound for hygiene.

- **Public `trusted!` macro.**  An opt-in escape hatch for users
  who want to feed a schema-known constant into the builder
  without going through a model handle (rare; e.g., a constant
  in an external configuration crate).  Deferred until a real
  use case emerges; the iteration-1 design has no public
  construction site for `Trusted` values outside the model
  derive.

- **Untrusted SQL string fragments.**  This design does not let a
  user pass a raw SQL fragment with promised hygiene.  Toasty's
  position is that the typed builder is the only entry point; a
  future raw-SQL escape hatch is an orthogonal feature.

- **Migrating CHECK-constraint emission to the trust tag.**  The
  inline-literal serializer in
  `toasty-sql/src/serializer/value.rs` continues to escape
  defensively, regardless of trust.  This design unlocks the
  option of trusting that path more aggressively but does not
  exercise it.

- **Cross-driver SQL-text caching.**  A trust-aware AST makes
  literal inlining stable for cache lookup, but the cache itself
  is a separate feature.

- **Wholesale newtype-ID introduction (`TableId`, `ColumnId`,
  etc.).**  Out of scope per the project's existing house-style
  decision.
