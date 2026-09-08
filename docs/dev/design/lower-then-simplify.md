# Lower-then-simplify pipeline

## Summary

`Simplify` runs before lowering as well as after. The pre-lowering
pass exists because lowering's pattern matches assume folded input,
but it pays for every rule the visitor contains, including the O(n²)
ones — OR factoring, OR-to-IN conversion, range-to-equality, tuple
decomposition, contradiction detection, OR-branch pruning — on a tree
those rules cannot usefully rewrite. Replacing it with a `fold` call
gives lowering the folded input it needs and leaves the heavyweight
rules running once, on the lowered tree.

Two changes make that replacement lossless. The rules `simplify`
still owns that are cheap, local, and schema-free move into `fold`.
The peepholes that consult the schema switch from `Reference::Field`
and `app::Schema` to `Reference::Column` and `db::Schema`, because
after the change `simplify` never sees an app-level reference.

## Motivation

`Simplify` runs at twelve call sites: pre- and post-lowering
(`lower.rs:108`, `:124`), pre- and post-lowering on sub-statements
built through `lower_sub_stmt` (`lower.rs:1775`, `:1780`), on three
detached sub-statements inside the lowering walk (`lower.rs:792`,
`:977`, `:1002`), and at five exec-time sites
(`exec/exec_statement.rs:118`, `exec/upsert.rs:31`, `exec/kv.rs:34`,
`:74`, `:101`). Every call pays the full cost of the heavyweight
rules, which only do useful work on a lowered tree.

`fold` already holds most of the cheap rewrites — constant folding,
AND/OR flattening, null propagation, IN-list dedup, `Let` inlining —
and is safe to call repeatedly. Finishing the split means moving the
remaining cheap rules out of `simplify` and then swapping the
pre-lowering `Simplify` calls for `fold`.

## Pipeline

`fold` is a transform, not a pipeline stage. One invocation site is
missing: inside `lower`, in place of the pre-lowering `Simplify`
pass. Lowering's pattern matches assume folded input — `Value::Null`
short-circuits, `Cast(Value)` collapse, IN-list literal items — and
`fold` supplies all of it.

The exec-time sites keep calling `simplify`, which folds each node
before applying the heavyweight rules. A separate `fold` call there
would be redundant.

The "Phase 2: Simplification" section of
`docs/dev/architecture/query-engine.md` describes `engine/simplify.rs`
without mentioning `fold`; update it with the pipeline change.

## Catalog

### Rules still to move into `fold`

O(n), local, and schema-free, but still in `simplify`. The
pre-lowering `Simplify` call cannot become a `fold` call until they
move:

- Project into `Value::Record` constant evaluation
  (`simplify/expr_project.rs:6-20`).
- `Map` and `Any` over a constant base (`simplify/expr_map.rs:6`,
  `simplify/expr_any.rs:5`).
- Empty propagation: empty-VALUES collapse, empty-source elimination,
  set-op flattening, set-op single-operand reduction
  (`simplify.rs:113-135`, `simplify/stmt_query.rs:6-17`).
- Canonical operand ordering for AND/OR. No implementation exists.
  It gives the heavyweight AND/OR rules — idempotent law, absorption,
  contradiction detection — a single form to match against.

`is_always_null_derived_column` (`simplify/expr_binary_op.rs:125-142`)
stays in `simplify`. It resolves a reference through the expression
context to inspect a derived VALUES table, so it cannot satisfy
`fold`'s schema-free invariant.

### Schema-aware peepholes

Three rules match a small local pattern — a
[peephole](https://en.wikipedia.org/wiki/Peephole_optimization), in
compiler-optimization terms — and consult the schema for reference
properties:

- `IS NULL` on a non-nullable reference
  (`simplify/expr_is_null.rs:9-18`).
- Redundant cast `cast(x, T)` when `x` is already `T`
  (`simplify/expr_cast.rs:34-45`).
- Self-comparison `x = x → true` / `x != x → false` on non-nullable
  references (`simplify/expr_binary_op.rs:28-37`).

All three match `Reference::Field` and resolve through `app::Schema`.
They must match `Reference::Column` and consult `db::Schema` for
column nullability and type. They can then live in either place:

- **Inside `lower`, at the rewrite site.** The field and column are
  both in hand during the field→column conversion, so the peephole
  fires for free.
- **Inside `simplify`, post-lowering.** They're peepholes — small
  pattern matches with one extra schema lookup — which is what
  `simplify` is for. Living here means they fire whenever `simplify`
  sees a fresh `Reference::Column`, including ones produced by other
  `simplify` rules.

Either is fine. The implementation picks based on whether other
`simplify` rules can produce shapes that newly enable these
peepholes; if not, the rewrite-site placement is slightly cheaper.

### `simplify`

Insert-statement list merging (`simplify/expr_list.rs:13-34`) gates
on `insert.target.is_model()`. Once `simplify` only sees lowered
trees, the gate becomes `is_table()`, and the `Returning::Model`
check on the same path (`:28-31`) needs its db-level equivalent.

### Call-site changes

- The pre-lowering `Simplify` calls (`lower.rs:108` for whole
  statements, `lower.rs:1775` for sub-statements built through
  `lower_sub_stmt`) become `fold` calls.
- The three detached sub-statement `simplify_stmt` calls
  (`lower.rs:792`, `:977`, `:1002`) stay. Each of those
  sub-statements detaches into its own HIR entry through
  `new_sub_statement`, so no later pass sees it and the heavyweight
  rules have to run locally.
- The exec-time calls stay `simplify` calls.

## Invariants

- **`simplify` precondition.** Input contains no `Source::Model`,
  `UpdateTarget::Model`, `InsertTarget::Model`, `Reference::Model`,
  `Reference::Field`, `IsVariant`, or `Association`. Enforce it with
  a `debug_assert` at the visitor entry. The `Source::Model { via }`
  arm at `simplify.rs:214-219`, which falls into a `todo!`, goes away
  with the change, as do the `Reference::Field` matches in the
  peepholes.
- **`lower` postcondition.** Every reference is `Reference::Column`,
  every source is `Source::Table`, every `IsVariant` is gone, every
  schema-aware peephole has fired at the rewrite site (or, if placed
  in `simplify`, fires there).

## Open questions

- **Where the post-lowering simplify runs.** `lower.rs:124` is the
  only post-lowering `simplify_stmt` call, and `Engine::exec`
  (`engine.rs:87-104`) runs `normalize → verify → lower → plan →
  exec` with no simplify stage between lower and plan. Either the
  call stays inside `lower`, or `Simplify` becomes a stage in
  `Engine::exec`. Blocking implementation.

## Out of scope

- The post-lowering form of the variant tautology: a
  discriminant-equality OR covering every variant value folds to
  `true`. The `IsVariant` form of the rule fires during lowering. The
  lowered form would also catch shapes the `IsVariant` form never
  sees, but it is a separate rule.
