# Static SQL Values

## Summary

Add a new `Expr::Static(Value)` AST variant alongside `Expr::Value`.
`Expr::Value` is extracted as a bind parameter (today's behavior);
`Expr::Static` survives `extract_params` and renders inline as a SQL
literal.  The engine and the model derive emit `Expr::Static` for
schema-fixed leaves (`LIMIT n` from the builder, enum discriminants,
`#[auto]` defaults).  User-supplied values stay `Expr::Value` and
flow through the bind path unchanged.

## Motivation

[#237] asks how Toasty should distinguish values that need
escaping from values that do not.  The framing that survives is
narrower: every value reaches the driver through a bind parameter
today, so escaping is the driver's concern.  The remaining
question is *rendering* — whether a leaf belongs in the SQL text
or in the parameter list.  Three concrete payoffs:

1. **Bind-parameter inflation.**  `extract_values`
   (`crates/toasty/src/engine/extract_params.rs:91`) replaces every
   `Expr::Value` with `Expr::Arg(n)`.  `LIMIT 10` becomes
   `LIMIT $1` with `[I64(10)]` as a bind.  Plan caches in
   PostgreSQL, MySQL, and SQLite key on SQL text; inlining
   schema-fixed leaves widens the universe of query shapes the
   cache recognizes as the same.

2. **Runtime-only literal invariants.**  `verify.rs:165` panics
   if `LIMIT`/`OFFSET` is anything other than `Expr::Value(I64(_))`.
   With `Expr::Static`, the check tightens to "is `Expr::Static`
   carrying `Value::I64`," and a future contributor who threads a
   user-supplied bind into pagination fails at the AST level.

3. **DDL inline-literal contexts.**
   `crates/toasty-sql/src/serializer/value.rs:30` already inlines
   `Value` for CHECK constraints and column DEFAULTs.  The
   inline-only paths can require `Expr::Static` once the variant
   exists.

[#237]: https://github.com/tokio-rs/toasty/issues/237

## User-facing API

No new user-facing API.  Existing queries compile and execute
unchanged.

The observable change is in the rendered SQL.  A query like
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

after this lands.

## Proposed changes

### AST

Add `Expr::Static(Value)` next to `Expr::Value` in
`crates/toasty-core/src/stmt/expr.rs`:

```rust
pub enum Expr {
    // ... existing variants ...

    /// Constant value rendered as a bind parameter.  The default
    /// for user-supplied leaves.
    Value(Value),

    /// Constant value rendered inline as a SQL literal.  Set by
    /// the engine and the model derive for schema-fixed leaves
    /// (`LIMIT n` from the builder, enum discriminants, `#[auto]`
    /// defaults).  Survives `extract_params` and reaches the
    /// serializer unchanged.
    Static(Value),
}
```

### Extract-params

`extract_values` matches `Expr::Value` and replaces with
`Expr::Arg(n)`.  `Expr::Static` is not matched and passes through:

```rust
// In crates/toasty/src/engine/extract_params.rs::extract_values
match expr {
    stmt::Expr::Value(value) if is_extractable_scalar(value) => {
        // ... replace with Expr::Arg(n), as today ...
    }
    stmt::Expr::Static(_) => {
        // Pass through.  Rendered inline by the serializer.
    }
    // ... other arms unchanged ...
}
```

### Serializer

The SQL serializer renders `Expr::Static` through the existing
`Value::to_sql` inline path:

```rust
// In crates/toasty-sql/src/serializer/expr.rs::ToSql for &stmt::Expr
stmt::Expr::Static(value) => value.to_sql(cx, f),
```

`Value::to_sql` already escapes `String` correctly for the inline
DDL path and is reused here.

### Engine and macro hookup

Sites that synthesize schema-fixed leaves switch from `Expr::Value`
to `Expr::Static`.  The set for iteration 1:

- `LIMIT n` / `OFFSET n` from the builder
  (`crates/toasty/src/stmt/query.rs`).
- Enum discriminants emitted during lowering
  (`crates/toasty/src/engine/lower/`).
- `#[auto]` and literal defaults emitted by the model derive
  (`crates/toasty-macros/src/expand/`).
- Variant-encoded `None` markers in embedded enums (same call
  sites as discriminants).

Every other `Expr::Value` construction stays `Value`.

### Engine passes

`Simplify`, `fold`, and `lower` preserve the variant through
rewrites.  Constant folding `Static + Static → Static`,
`Value + Value → Value`, mixed `Value + Static → Value`.  PR #703's
stability gate already enforces "rewrite only if both children are
stable"; mixed-mode folding cannot happen, so the lattice rule
does not need a new visitor.

## Edge cases

- **`Expr::Value` matches across the codebase.**  Every existing
  `match` on `Expr::Value(v)` that should also handle the inline
  case adds an `Expr::Static(v)` arm or `Expr::Value(v) |
  Expr::Static(v)`.  The compiler enumerates the call sites.

- **Records and lists.**  A record or list with mixed-provenance
  fields stays an `Expr::Record` / `Expr::List` whose leaves
  individually are `Value` or `Static`.  There is no record-level
  render mode.

- **Round-trip values from `RETURNING`.**  Values read back from
  the database and re-fed into a follow-on statement
  (the PR-#812 lower-sub-statement path) are classified `Value` in
  iteration 1.  Classifying scalar returns as `Static` is deferred.

- **Custom `IntoExpr` impls.**  User implementations of `IntoExpr`
  for custom types construct `Expr::Value`.  Iteration 1 has no
  public path to construct `Expr::Static`.

## Driver integration

Nothing changes for drivers.  SQL drivers receive a SQL string and
`Vec<TypedValue>`; the render decision is consumed entirely
upstream of the driver call.  A driver that previously received
`("SELECT ... LIMIT ?1", [TypedValue { value: I64(10), ty: I64 }])`
now receives `("SELECT ... LIMIT 10", [])`.

DynamoDB is unaffected: the driver has no SQL-text-vs-bind
distinction, and every `Expr::Static` leaf flows through the same
`TypedValue` channel as an `Expr::Value` would.

## Out of scope

- **Aggressive RETURNING classification.**  Per-shape allow-list
  (`Bool`/integer/`Uuid` returns become `Static`; `String`/`Bytes`/
  list/record stay `Value`) deferred to a follow-up.
- **Public opt-in for `Static`.**  No call-site API for marking a
  user-supplied constant `Static` in iteration 1.
- **Migrating CHECK-constraint emission to require `Static`.**
  The inline-literal serializer keeps escaping defensively
  regardless of variant.
- **Raw-SQL string fragments.**  Orthogonal feature.
