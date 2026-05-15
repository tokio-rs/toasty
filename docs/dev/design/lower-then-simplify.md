# Lower-then-simplify pipeline

## Summary

The pipeline reorders from `simplify → lower → simplify → …` to
`lower → simplify → plan → exec`. The pre-lowering simplify pass
goes away; what remains runs once, after lowering. Making this
work requires moving the rewrites that pattern-match on app-level
constructs — model→PK, field→FK, `IsVariant`→discriminant,
`model.via`→filter, `InSubquery` lifting over relations — into
`lower`. Going from app-level to db-level is what lowering does;
these rewrites are steps in that process. After the move, nothing
in `simplify` references `app::Schema` — the only schema it might
consult is `db::Schema`, for column properties that a few
post-lowering peepholes still need.

A new reusable transform, `fold`, contains the cheap O(n) rewrites
— constant folding, AND/OR flattening, IN-list dedup, and similar —
and is invoked from inside `lower`, inside `simplify`, and inside
`exec_statement` after bind-value substitution. `fold` is not a
pipeline stage; it's a transform reused wherever new constants
might appear. Splitting it out is what lets the pre-lowering
simplify pass disappear without breaking lowering's pattern
matches: lowering still gets folded input, but pays only for
folding rather than the full simplifier.

## Motivation

Today's `Simplify` visitor is called seven times across the
pipeline: pre-lower, post-lower, four times inline on
lowering-generated sub-statements (`lower.rs:418`, `:444`, `:930`,
`lower/relation.rs:397`), and once at exec time after bind-value
substitution (`exec_statement.rs:80`). Each call pays the full
cost of every rule the visitor contains, including its O(n²) rules
— OR factoring, OR-to-IN conversion, range-to-equality, tuple
decomposition, match elimination, contradiction detection,
OR-branch pruning. These rules only do useful work once, on the
post-lowering tree. Running them seven times pays a real cost.

The fundamental fix is to separate the rules by cost so the
expensive ones run once. Three changes follow:

1. **Cheap pattern rewrites split out as `fold`** — constant
   folding, AND/OR flattening, IN-list dedup, null propagation,
   `Let` inlining, and similar O(n) idempotent rewrites. `fold` is
   safe to call repeatedly and takes over the call sites where
   lowering needs folded input (today's pre-lowering simplify), the
   sites where lowering generates new foldable structure (today's
   mid-lowering `simplify_stmt` calls), and the first step of
   `simplify` itself.
2. **The expensive rules stay in `simplify`**, which runs once on
   the lowered tree — and once at exec time after bind-value
   substitution, same as today.
3. **App-level rewrites move into `lower`** — model→PK, field→FK,
   `IsVariant`→discriminant, `model.via`→filter, `InSubquery`
   lifting over relations. These pattern-match on app-level
   constructs that lowering removes anyway; moving them into
   `lower` makes them part of the app→db conversion they belong in
   and gets the pre-lowering simplify pass out of the picture.

## Pipeline

```
Lower → Simplify → Plan → Exec
```

Simplify runs after Lower in the canonical pipeline, and again at
exec time after bind-value substitution — same as today. Plan and
Exec are otherwise unchanged.

`fold` is a reusable transform, not a pipeline stage. It is
invoked from:

1. **Inside `lower`, as a pre-pass.** Lowering's pattern matches
   assume folded input (`Value::Null` short-circuits, `Cast(Value)`
   collapse, IN-list literal items). Today these work because the
   pre-lowering `Simplify` pass folded everything first; after the
   refactor, `lower` calls `fold` itself.
2. **Inside `lower`, locally on rewrites that produce foldable
   structure.** Same pattern as today's mid-lowering
   `simplify_stmt` calls (`lower.rs:418`, `:444`, `:930`,
   `lower/relation.rs:397`), but calling the cheap transform
   instead of the full simplifier.
3. **Inside `simplify`, as its first step.** `simplify` consumes
   canonical, folded input; it folds first, then runs the
   heavyweight rules. This applies both to the canonical
   post-Lower call and to the exec-time post-substitution call.

## Catalog

Every rule in `engine/simplify/` falls into one of three locations.

### Move to lower (app-shaped rewrites)

Match on `Reference::Model`, `Reference::Field`, `IsVariant`,
`Source::Model { via }`, or `UpdateTarget::Query`, and have
nothing to match on after lowering:

- `simplify_via_association_for_{delete,insert,query}`
  (`association.rs`).
- `lift_in_subquery` (`lift_in_subquery.rs`).
- `simplify_expr_eq_operand` (`expr_binary_op.rs:7-47`) — model→PK
  and BelongsTo→FK rewriting.
- `rewrite_expr_in_list_when_model` (`expr_in_list.rs:33-71`).
- `try_variant_tautology_or` (`expr_or.rs:203-241`).
- **`UpdateTarget::Query` rewrite** (`simplify.rs:190-209`,
  `visit_stmt_update_mut`). `Update { target: Query(q), filter, .. }`
  becomes `Update { target: Model(m), filter: filter ∧ q.filter, .. }`
  by lifting the query's source model and merging its filter.
  `UpdateTarget::Query` does not exist post-lowering; the lowering
  walk's `visit_update_target_mut` panics on it. Moves into `lower`
  as a standalone pre-pass visitor, in the same pattern as
  `RewriteVia` and `LiftInSubquery`.

### Schema-aware peepholes (placement is implementation choice)

Each rule matches a small local pattern on the expression tree — a
[peephole](https://en.wikipedia.org/wiki/Peephole_optimization), in
compiler-optimization terms — and consults the schema for column
properties:

- `IS NULL` on non-nullable references — `expr_is_null.rs:6-15`.
- Redundant cast `cast(field, T)` when the field is already `T` —
  `expr_cast.rs:17-24`.
- Self-comparison `x = x → true` / `x != x → false` for
  non-nullable references — `expr_binary_op.rs:67-78`.

Today they match on `Reference::Field` and consult `app::Schema`.
After the refactor they match on `Reference::Column` and consult
`db::Schema` for column nullability and type. They can live in
either place:

- **Inside `lower`, at the rewrite site.** The field and column are
  both in hand during the field→column conversion, so the peephole
  fires for free.
- **Inside `simplify`, post-lowering.** They're peepholes — small
  pattern matches with one extra schema lookup — which is what
  `simplify` is for. Living here means they fire whenever
  `simplify` sees a fresh `Reference::Column`, including ones
  produced by other `simplify` rules.

Either is fine. The implementation picks based on whether other
`simplify` rules can produce shapes that newly enable these
peepholes; if not, the rewrite-site placement is slightly cheaper.

The fourth schema-aware rule, `is_always_null_derived_column`
(`expr_binary_op.rs:232-241`), already operates on
`ResolvedRef::Derived` and is db-level. It stays, in `fold`.

### `fold` — the cheap, reusable transform

O(n) per pass, idempotent on output, schema-free. Each rule fires
when a node matches a small, local pattern. Invoked from `lower`
(as pre-pass and locally on rewrites), from `simplify` (as its
first step), and from `exec_statement` (post-substitution). Rules
listed in the order they fire; canonicalization runs first so
subsequent rules can rely on canonical input:

- **Canonicalization**: literal-on-rhs swap for commutative
  operators, single canonical form for AND/OR operand ordering.
- **Constant folding**: `Value op Value`, `not(literal)`,
  `cast(literal)`, `is_null(literal)`, project into `Value::Record`,
  record-of-values to `Value::Record`, `Map`/`Any` over a constant
  base.
- **Boolean unit folding**: `TRUE ∧ x → x`, `FALSE ∨ x → x`, drop
  `TRUE` from AND, short-circuit AND on `FALSE`.
- **AND/OR flattening**: associative collapse of nested AND/OR.
- **Null propagation**: `x op null → null`.
- **Boolean-comparison rewrites**: `x = true → x`, `x != false → x`.
- **IN-list dedup**, single-item IN → equality.
- **Empty propagation**: empty-VALUES collapse, empty-source
  elimination, set-op flattening, set-op single-operand reduction.
- **`Let` inlining** over stable bindings. Today this rule lives
  in `simplify/expr_let.rs`; the refactor moves it into `fold`.
  This is load-bearing for the exec-time
  `Cast(Arg) → Cast(Value) → Value` collapse: lowering's
  `cast_expr` wraps `Arg` operands in `Cast(Arg, target_ty)` as a
  type-cast marker (`lower.rs:1586-1589`). When the surrounding
  statement is a `Let`, lowering converts `Stmt → Arg` bindings,
  the bindings become stable, `Let` inlining substitutes the
  binding's `Value` into the body, and the resulting `Cast(Value)`
  folds. Without `Let` inlining in `fold`, `Cast(Arg)` survives
  past extract-params and panics the SQL serializer.

### `simplify` — the heavyweight stage

Runs as a single visitor pass on the lowered tree. Begins with a
`fold` call to canonicalize and fold its input, then applies the
heavyweight rules.

**Structural rewrites.** O(n²) over an operand list or tree slice.
Reshape ORM-generated SQL into a form drivers handle well:

- OR factoring `(a ∧ b) ∨ (a ∧ c) → a ∧ (b ∨ c)`.
- OR-to-IN conversion. General-purpose query engines often skip
  this rewrite (or invert it for short lists). Toasty keeps it
  because driver SQL serialization handles `IN` better than long
  `OR` chains, and ORM filter composition is a common source of
  disjunctive equalities.
- Range-to-equality `x ≥ c ∧ x ≤ c → x = c`.
- Tuple decomposition `(a, b) = (x, y) → a = x ∧ b = y`. Composite
  primary keys produce row-equality expressions that drivers index
  poorly; tuple decomposition rewrites them into per-column
  comparisons.
- Match elimination, project-into-Match distribution, uniform-arms
  folding.
- Insert-statement list merging (`expr_list.rs:38-117`) — switches
  the `is_model()` gate to `is_table()`.

**Predicate inference.** O(n²), proves redundancy or refutation
rather than reshaping structure:

- Idempotent law `a ∧ a → a`, `a ∨ a → a`.
- Absorption `a ∧ (a ∨ b) → a`, `a ∨ (a ∧ b) → a`.
- Complement `a ∧ ¬a → false`, `a ∨ ¬a → true`.
- Contradiction detection `x = 1 ∧ x = 2 → false`,
  `x = 1 ∧ x ≠ 1 → false`.
- OR-branch pruning (`prune_or_branches`).

### Call-site changes

- The pre-lowering `Simplify` call (`lower.rs:55`) becomes a `fold`
  call inside `lower`.
- The post-lowering `simplify_stmt` call (`lower.rs:71`) is dropped:
  the parent pipeline runs `simplify` next, which folds and then
  runs the heavyweight rules.
- The four mid-lowering `simplify_stmt` calls (`lower.rs:418, 444,
  930`; `lower/relation.rs:397`) become `fold` calls on freshly
  generated sub-statement bodies. Each generated sub-statement is
  separately lowered and simplified through the canonical pipeline
  before being stitched into its parent.
- The exec-time `simplify_stmt` call (`exec_statement.rs:80`)
  remains a `simplify` call (which folds first, then runs the
  heavyweight rules) — same behavior as today.

## Sub-statement handling

Lowering generates several kinds of sub-statement: `INCLUDE`
subqueries, `EXISTS`-fallback subqueries for non-SQL drivers,
cascade-delete subqueries, child inserts for relation planning.
Each sub-statement is itself lowered, folded, and simplified
through the canonical pipeline before being stitched into its
parent. There is no remaining "simplify on a half-lowered
statement" call site.

## Invariants

- **`fold` idempotence.** Running `fold` twice on its own output is
  a no-op. Required for its many invocations to compose.
- **`fold` is schema-free.** Output is a function of input alone.
  Rules pattern-match on local structure only. This is what lets
  `lower`, `simplify`, and `exec_statement` all call it without
  worrying about which schema layer is in scope.
- **`simplify` precondition.** Input contains no `Source::Model`,
  `UpdateTarget::Model`, `InsertTarget::Model`, `Reference::Model`,
  `Reference::Field`, `IsVariant`, or `Association`.
- **`simplify` postcondition.** Output is `simplify`-idempotent.
- **`lower` postcondition.** Every reference is `Reference::Column`,
  every source is `Source::Table`, every `IsVariant` is gone, every
  schema-aware peephole has fired at the rewrite site (or, if
  placed in `simplify`, fires there).

## Edge cases

- **`build_include_subquery`** today emits a `Source::Model`
  subquery and runs `Simplify::with_context`. After the refactor,
  this is a recursive `lower_stmt` call on the synthesized
  subquery, which gets its own `fold` and `simplify` passes. The
  scope/dependency machinery already supports this for nested
  `INCLUDE`; verify the existing call site composes cleanly.
- **Variant tautology** moves into lowering with the other
  `IsVariant`-shaped rules. It fires there during lowering, the
  same as today's pre-lowering simplify pass. Longer-term goal: the
  equivalent post-lowering rule — a discriminant-equality OR
  covering every variant value folds to `true` — belongs in
  `simplify`, where it can also fire on lowered shapes that the
  `IsVariant` form never sees. Out of scope for this refactor.
