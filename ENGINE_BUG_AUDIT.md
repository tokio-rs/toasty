# Toasty Engine Bug Audit

Audit of the query engine (`crates/toasty/src/engine/`) and the client-side
expression evaluator in `toasty-core` that it dispatches into. Six
sub-agents reviewed simplify, lower, plan, exec, eval/fold, and
index/verify in parallel; the synthesis below de-duplicates their findings
and rules out the false-positives I could not verify against the source.

Each finding cites `file:line` for the offending code and the minimum
trigger I was able to construct or reproduce from the audit. Findings
marked **VERIFIED** were read end-to-end in this synthesis. The remaining
items are reported with the agent's confidence.

Branch under audit: `claude/toasty-engine-bug-audit-MWt10` at
`69d1d2c feat: add raw SQL execution API (#965)`.

---

## Critical

Wrong rows, silently dropped rows, or hard panics on legal queries that
go through normal Toasty APIs.

### C1. `.include()` of a `BelongsTo` with a composite FK builds a tautological filter — VERIFIED

- **File:line**: `crates/toasty/src/engine/lower/include.rs:344-346`
- **Symptom**: every parent row matches; whichever the DB picks (the
  subquery sets `query.single = true`, so LIMIT 1) is returned —
  effectively a random parent.
- **Trigger**: any model whose `belongs_to` has more than one FK column,
  fetched with `.include(child::FIELDS.parent())`:
  ```rust
  // Category has composite PK (id, revision); Todo's FK spans
  // (category_id, category_revision).
  Todo::filter_by_id(some_id)
      .include(Todo::FIELDS.category())
      .get(&db).await?;
  ```
- **Root cause**: the composite branch builds the target-PK reference
  from `ref_parent_field(fk_field.source)` *twice*. The single-FK arm
  immediately above correctly uses `ref_self_field(fk_field.target)` for
  the target side. The synthesized filter is therefore
  `record(parent.source_fk) == record(parent.source_fk)`, a tautology.
- **Coverage gap**: no integration test exercises `.include()` on a
  composite-FK `BelongsTo` —
  `relation_chain_composite_key.rs` and `relation_has_many_composite_key.rs`
  only navigate via relation chains, not `.include()`.

### C2. Multi-column cursor pagination uses AND instead of lexicographic OR — VERIFIED

- **File:line**: `crates/toasty/src/engine/lower/paginate.rs:28-46`
- **Symptom**: after `(a=1, b=2)` from cursor, rows like `(1, 3)` and
  `(2, 1)` — which should appear next — are skipped. The "next page"
  silently omits valid rows.
- **Trigger**: any `.paginate()` over an `ORDER BY` with more than one
  expression.
- **Root cause**: the loop in `rewrite_offset_after_as_filter` calls
  `body.filter.add_filter(expr)` once per column, which AND-conjoins.
  The correct lexicographic filter for `ORDER BY a, b` after cursor
  `(1, 2)` is `a > 1 OR (a = 1 AND b > 2)`; the code emits
  `a > 1 AND b > 2`.
- **Coverage gap**: every existing pagination test in
  `crud_composite_key_pagination.rs` orders by a single column.

### C3. `lift_in_subquery` asserts eq/ne, making the fallback path unreachable — VERIFIED

- **File:line**: `crates/toasty/src/engine/lower/lift_in_subquery.rs:351`
- **Symptom**: panics on any non-eq/ne comparison inside a
  `belongs_to.in_query(...)` subquery filter.
- **Trigger**:
  ```rust
  Post::filter(
      Post::FIELDS.user().in_query(
          User::filter(User::FIELDS.age().gt(30))   // gt → panic
      )
  ).all(&db).await?;
  ```
- **Root cause**: an `assert!(i.op.is_eq() || i.op.is_ne())` precedes an
  `if i.op.is_eq() || i.op.is_ne() { ... } else { self.fail = true; }`.
  The `else` branch is the intended graceful fallback (drop to verbatim
  `IN`-subquery form) but is dead code because the assert fires first.
  Removing the assert (or guarding its body) is the fix.

### C4. `.like()` on DynamoDB bypasses `verify` and panics in the driver — VERIFIED

- **File:line**: `crates/toasty/src/engine/verify.rs:298-310` (missing
  check); `crates/toasty-driver-dynamodb/src/lib.rs:410-414` (the actual
  `panic!`).
- **Symptom**: planner accepts the query, then the driver aborts the
  process at expression-render time.
- **Trigger**:
  ```rust
  User::filter(User::FIELDS.name().like("Al%".to_string()))
      .all(&db).await?;
  ```
- **Root cause**: `visit_expr_like` only records `unsupported_feature`
  when `case_insensitive && !native_ilike`. The case
  `!case_insensitive && !native_like` falls through. DynamoDB sets
  `native_like: false` (`capability.rs:692`). Per
  [AGENTS.md], this should reject with `unsupported_feature`, not panic
  in the driver.

### C5. `partition_filter` `todo!()`s on OR-of-AND with mixed indexable/non-indexable predicates — VERIFIED

- **File:line**: `crates/toasty/src/engine/index/index_match.rs:378,382,385`
- **Symptom**: planner panics on a legal DynamoDB query.
- **Trigger** (model with PK `user_id` and a non-indexed `name`):
  ```rust
  // WHERE (user_id = "u1" AND name = "bob") OR (user_id = "u2" AND name = "alice")
  ```
  Each branch of the outer OR legitimately splits into an index condition
  (`user_id = …`) *and* a residual filter (`name = …`). The OR
  partitioner has no plan for that shape and falls into `todo!`.
- **Root cause**: the OR arm of `partition_filter` is hard-coded to the
  "every branch is purely index or purely residual" case. Mixed branches
  (the realistic case) hit the `todo!` and the asserts at 382/385.

### C6. `partition_filter` `todo!()`s on `AnyOp` / `IsSuperset` / `Intersects` inside an indexed AND — VERIFIED

- **File:line**: `crates/toasty/src/engine/index/index_match.rs:440`
- **Symptom**: planner panics on a query the DDB driver could otherwise
  execute (the driver renders `AnyOp(Eq, …)` at
  `toasty-driver-dynamodb/src/lib.rs:415`).
- **Trigger** (DDB model with PK `id` and `tags: Vec<String>`):
  ```rust
  // WHERE id = "u1" AND tags.contains("foo")
  ```
  `match_restriction` accepts the AND because `id="u1"` matches column 0;
  `partition_filter` recurses on the `AnyOp` operand and hits the
  catch-all `_ => todo!`.
- **Root cause**: the catch-all should fall through to the
  "doesn't reference an index column → ship to result filter" arm rather
  than panic.

### C7. Client-side `eval` uses two-valued logic — NULL handling diverges from SQL on the DDB post-filter path — VERIFIED

Two distinct mis-handlings of `Value::Null` in `core::stmt::eval::Expr`,
both reachable through `exec::filter` on the NoSQL post-filter path
(`engine/plan/statement.rs:1514` → `mir::Filter` → `exec/filter.rs:34`).

**C7a. `NULL = NULL` evaluates to `true`** instead of `NULL`.
- **File:line**: `crates/toasty-core/src/stmt/eval.rs:198-199`
- `BinaryOp::Eq` is implemented as `(lhs == rhs).into()`, where `Value`
  derives `PartialEq` (`value.rs:33`). `Value::Null == Value::Null` is
  `true`, so a DDB post-filter of the form `col1 = col2` keeps rows
  where both are NULL — the same rows every SQL backend hides.

**C7b. Ordered comparison with NULL errors the whole query** instead of
dropping the row.
- **File:line**: `crates/toasty-core/src/stmt/eval.rs:200-203` →
  `cmp_ordered:457-466`.
- A DDB post-filter `WHERE age > 18` against a row with
  `age = NULL` returns `Err("ordered comparison with NULL is
  undefined")`. `exec/filter.rs:34` propagates the error via `?`, so the
  whole query fails — SQL semantics would simply omit the row.

The same root cause leaks into AND/OR (`eval.rs:174-184, 303-313`),
`Expr::Not` (`eval.rs:229-232`), and `Expr::InList` (`eval.rs:337-348`).
AND/OR's order-dependence is the more insidious face: `false AND NULL`
works (short-circuit on `false`); `NULL AND false` errors. Fold masks
the literal-NULL case, but at exec time operands typically resolve from
args/refs and bypass the fold.

### C8. UPDATE/DELETE with `.returning(...)` and partial PK on NoSQL panics — VERIFIED

- **File:line**: `crates/toasty/src/engine/plan/statement.rs:1378`
- **Symptom**: `assert!(columns.is_empty())` fires in release builds and
  aborts.
- **Trigger** (DynamoDB, composite PK `(user_id, status)`):
  ```rust
  Order::filter(Order::FIELDS.user_id.eq("u1"))   // only partition key
      .update(...)
      .returning(Order::FIELDS.total)
      .exec(&db).await?;
  ```
- **Root cause**: `extract_columns_from_returning` (called at line 215
  for every statement) inserts the returning columns into
  `load_data.select_items`. The mutation branch then asserts that set
  was empty before re-populating it with the index-key columns. The
  comment on line 1374 ("pre-populated into load_data.select_items")
  contradicts the assert.

### C9. `fold_expr_cast` `.unwrap()`s a fallible cast — VERIFIED

- **File:line**: `crates/toasty/src/engine/fold/expr_cast.rs:15`
- **Symptom**: planner panics during constant folding instead of
  surfacing a typed error.
- **Trigger**: any constant cast whose value is out of range, fails to
  parse, or has no implementation:
  - `cast(Value::I64(300), Type::I8)` — `i8::try_from` overflows.
  - `cast(Value::I64(-1), Type::U32)` — sign mismatch.
  - `cast(Value::String("not-a-uuid"), Type::Uuid)` — the inner
    `.parse().expect("could not parse uuid")` panics first
    (`ty.rs:344`), which is its own latent bug.
  - `cast(Value::I32(0), Type::Bool)` — falls through to the
    `todo!("value={value:#?}; ty={self:#?}")` at `ty.rs:397`.
- **Root cause**: `Type::cast` returns `Result`; the fold unwraps it.
  No fold/test exercises a fallible cast.

---

## High

Real correctness or robustness bugs, but with lower exposure (rare
input shapes, race conditions, latent invariants).

### H1. `QueryPk` multi-partition fan-out drops `ORDER BY` — VERIFIED

- **File:line**: `crates/toasty/src/engine/exec/query_pk.rs:60-64`
- **Symptom**: results are returned in partition-iteration order, not
  sort-key order.
- **Trigger** (DynamoDB):
  ```rust
  Model::filter(Model::FIELDS.pk.is_in(vec![a, b, c]))
      .order_by(Model::FIELDS.sk.desc())
      .all(&db).await?;
  ```
- **Root cause**: the assertion only guards against `limit`/pagination
  with multi-partition; the loop `.extend()`s each partition's rows
  without a merge-sort when `action.order` is `Some(_)`.

### H2. `try_extract_key_values` returns an `Arg` key for an `InList` without checking index cardinality — VERIFIED

- **File:line**: `crates/toasty/src/engine/index.rs:222-223`
- **Symptom**: `GetByKey` is dispatched with a `Value::List([scalar, ...])`
  against a composite-PK index that expects `Value::List([Record, ...])`.
  At best a typed error; at worst silently wrong rows.
- **Trigger**: composite-PK model + `IN` on only the partition column:
  ```rust
  Order::filter(Order::FIELDS.user_id.is_in(user_ids)).all(&db).await?;
  ```
- **Root cause**: the `Expr::InList { list: Expr::Arg, .. }` arm
  short-circuits with `Some(Expr::Arg(*arg))` without guarding on
  `index.columns.len() == 1` (which the literal-value arm immediately
  below does correctly).

### H3. `extract_shape` requires AND-operand order to match across OR branches — VERIFIED

- **File:line**: `crates/toasty/src/engine/index/or_rewrite.rs:204-234`,
  panic at `:167-173`.
- **Symptom**: an equivalent query written with operands in a different
  order panics with "OR index filter with multiple distinct branch
  shapes is not yet implemented".
- **Trigger** (DDB; composite PK `(user_id, status)`):
  ```rust
  // (user_id = "u1" AND status = "s1") OR (status = "s2" AND user_id = "u2")
  ```
- **Root cause**: `extract_shape` indexes by operand AST position rather
  than by index-column position; it should canonicalise against
  `index.columns`.

### H4. `ReadModifyWrite` write-row-count `assert_eq!` panics under READ COMMITTED races

- **File:line**: `crates/toasty/src/engine/exec/rmw.rs:158`
- **Symptom**: a concurrent writer that changes the row count between
  the savepoint read and the follow-up write aborts the process.
- **Root cause**: load-bearing assertion on the happy path; should be
  a typed error (e.g. `condition_failed`).

### H5. `prepare_post_filter` `todo!()`s on filters containing `Expr::Arg`

- **File:line**: `crates/toasty/src/engine/plan/statement.rs:1499`
- **Symptom**: planner panic.
- **Trigger**: DDB query that lacks `primary_key_ne_predicate`, uses
  `!=` on a PK column, and the filter also references a parent
  statement's column (`Expr::Arg`).
- **Root cause**: the rewrite only handles `Expr::Reference`; `Expr::Arg`
  hits the catch-all `todo!`. Symmetric arm needed.

### H6. Secondary-index `!=` predicate trips `assert!(post_filter.is_none())`

- **File:line**: `crates/toasty/src/engine/plan/statement.rs:1412`
- **Symptom**: any NoSQL query on a secondary index with `!=` on the
  matched column on a driver without `primary_key_ne_predicate` panics.
- **Root cause**: `apply_result_filter_on_results` was designed only
  for the primary-key path; the secondary-index branch was not
  generalised.

### H7. `Guard`'s false branch never `load`s its input variable — VERIFIED

- **File:line**: `crates/toasty/src/engine/exec/guard.rs:50-57`
- **Symptom**: today a benign slot leak (`VarStore` reference count
  never decremented). Becomes a real ordering bug the moment a planner
  change makes `Guard` share a producer with another consumer — the
  invariant "every action that names `input` calls `load` exactly once"
  is silently broken.
- **Root cause**: the `else` (guard failed) branch returns an empty
  stream without consuming `action.input`. Should `vars.load(input)`
  and discard the result.

### H8. `distribute_into_any` captures outer `Arg`s under the Map's bound name

- **File:line**: `crates/toasty/src/engine/index/or_rewrite.rs:116-146`
- **Symptom**: index filter has the wrong key when an outer AND operand
  references a parent argument that gets distributed into a `Map` body.
- **Root cause**: non-Any operands are extended into the Map body
  without bumping `nesting` on any `Expr::Arg` that crosses the Map
  boundary.

### H9. `extract_key_record` single-column path accepts `NULL` for the key

- **File:line**: `crates/toasty/src/engine/index.rs:222-237, 269-276`
- **Symptom**: an `id = NULL` filter on a single-column PK is routed to
  `GetByKey` with a `Value::Null` record — invalid DynamoDB GetItem.
- **Root cause**: the composite-key arm has an explicit Null filter
  (line 302); the scalar and `InList` arms do not. The `BinaryOp` arm
  at line 269 checks `op.is_eq()` and `columns.len() == 1` but never
  inspects the value.

---

## Medium

Real bugs, but either narrowly triggerable from the current public API
or with strictly bounded impact.

### M1. `partition_filter` drops `!=` on a mutation path — VERIFIED

- **File:line**: `crates/toasty/src/engine/index/index_match.rs:310-313`
  + `crates/toasty/src/engine/plan/statement.rs:1483, 1499`
- **Symptom**: on the mutation path, a `!=` predicate that
  `partition_filter` replaces with `true.into()` is not re-added as a
  `post_filter` (mutations enforce `post_filter.is_none()`). A
  `UPDATE … WHERE sort_key != X` on DDB could mutate rows the filter
  was meant to exclude.
- **Triggerability**: requires composite-PK with `=` on the partition
  key and `!=` on the sort key on a driver lacking
  `primary_key_ne_predicate`. Limited but reachable.

### M2. `is_always_non_nullable` claims `BinaryOp` is non-null — breaks complement law in 3VL

- **File:line**: predicate at `crates/toasty-core/src/stmt/expr.rs:227`;
  used at `crates/toasty/src/engine/simplify/expr_and.rs:100` and
  `expr_or.rs:167`.
- **Symptom**: `(a = 5) AND NOT(a = 5)` is rewritten to `FALSE` even
  when `a` is nullable; SQL 3VL says NULL. Filter context happens to
  hide the divergence (NULL filters out like FALSE), but projection or
  CASE contexts surface it as `false`/`true` literals.
- **Triggerability**: low today — Toasty queries are mostly filters —
  but the predicate is strictly unsound and would manifest the moment a
  Bool projection survives lowering.

### M3. `has_self_contradiction` rewrites without any nullability gate

- **File:line**: `crates/toasty/src/engine/simplify/expr_and.rs:56-58,
  248-278` (also reached from `prune_or_branches:181-244`).
- Same 3VL mismatch as M2, but without even a (broken) nullability
  check.

### M4. `simplify_expr_or::try_or_to_in_list` produces non-deterministic operand order

- **File:line**: `crates/toasty/src/engine/simplify/expr_or.rs:192-258`
- **Symptom**: SQL string for a given input AST is not stable —
  `hashbrown::HashMap` iteration order leaks into the output. Affects
  snapshot tests, prepared-statement cache hits, and log noise.
- **Triggerability**: any OR that mixes multiple equality groups.

### M5. Composite-PK `IN` of records uses partial index coverage as full coverage

- **File:line**: `crates/toasty/src/engine/index/index_match.rs:211-221`
- **Symptom**: an `InList { lhs: Expr::Record([colA]), … }` against a
  composite index `(colA, colB)` is treated as if the entire index is
  covered. The partition-filter pipeline over-commits.
- **Triggerability**: depends on whether lowering ever emits the record
  form for a partial composite-PK `IN`; not exercised in current
  tests.

### M6. Statement-level deps dropped when returning is a constant expression

- **File:line**: `crates/toasty/src/engine/plan/statement.rs:1741-1746`
- **Symptom**: sibling statements scheduled via batch APIs may never
  reach the execution plan when the parent's returning expression
  collapses to a constant.
- **Root cause**: the constant arm uses `Store::insert_with_deps` but
  never calls `apply_dependencies_to_node`; every other arm of
  `plan_output_node` does.

### M7. `rewrite_stmt_query_for_batch_load_nosql` `todo!()`s on `Arg::Sub`

- **File:line**: `crates/toasty/src/engine/plan/statement.rs:644-652`
- **Symptom**: a NoSQL batch-loaded query whose filter contains both
  `Arg::Ref` (the parent reference) and `Arg::Sub` (a subquery, e.g.
  `IN (SELECT ...)`) panics.
- **Triggerability**: reachable via a nested include whose child filter
  itself contains a subquery.

### M8. MySQL `UPDATE … RETURNING` workaround is not atomic — issue #881

- **File:line**: `crates/toasty/src/engine/exec/exec_statement.rs:303-319`
- **Symptom**: the follow-up SELECT runs outside a transaction wrap
  when `needs_transaction = false` and no outer transaction is active.
  Concurrent writers may change the matched rows between UPDATE and
  SELECT, returning wrong "returned" values.
- **Status**: acknowledged in the struct doc-comment (#881).

### M9. MySQL `INSERT … RETURNING` panics on non-auto-increment columns

- **File:line**: `crates/toasty/src/engine/exec/exec_statement.rs:351-371`
- **Symptom**: `assert!(column.auto_increment, ...)` aborts at exec
  time when a model uses a non-auto-increment PK (e.g. UUID) and the
  builder generated an INSERT…RETURNING. Should be a typed error at
  plan time or exec time.

### M10. `DeleteByKey` / `UpdateByKey` do not filter NULL or dedup keys

- **File:line**: `crates/toasty/src/engine/exec/delete_by_key.rs:32-46`,
  `update_by_key.rs:38-46`.
- **Symptom**: asymmetric with `GetByKey`. Duplicate keys reach the
  driver; behaviour depends on the driver (some no-op, others may
  surface a `condition_failed` error). NULL keys reach the driver too.
- **Triggerability**: depends on whether the planner ever passes
  duplicate or NULL keys; the invariant isn't asserted.

### M11. `fold_expr_in_list` panics on `Value::Null` or `Value::Record` rhs

- **File:line**: `crates/toasty/src/engine/fold/expr_in_list.rs:36-37, 53`
- **Symptom**: `todo!`/`panic` during fold.
- **Triggerability**: requires a malformed AST (rhs of an `InList` that
  isn't a list). Not reachable from the public surface API today, but
  the catch-all should `return None` rather than panic.

---

## Low

Latent gaps, missed optimisations, and perf — not user-observable
correctness today, but bear cleanup.

- **L1**: `compute_cost` prefers smaller unique indexes
  (`index_match.rs:262-283`). A `(a, b)` unique wins over `(a, b, c)`
  for a filter on all three. Perf only.
- **L2**: All non-unique indexes share `cost = 10`
  (`index_match.rs:279-281`); chosen by iteration order rather than
  selectivity. Perf only.
- **L3**: `rewrite_stmt_query_for_batch_load_sql` asserts at most one
  parent table (`plan/statement.rs:620`). Schemas with composite
  back-refs would hit it.
- **L4**: NestedMerge `Reference` rewrite ignores scope depth
  (`plan/nested_merge.rs:326-332`). The `Arg` arm handles it; the
  `Reference` arm is fragile if future shapes nest.
- **L5**: `Node::ty()` is `todo!()` for `NestedMerge` (`mir/node.rs:55`).
  Currently unreachable; defensive note.
- **L6**: `ReadModifyWrite` never sets `node.var`
  (`mir/read_modify_write.rs:36-55`). RMW is currently a terminal
  consumer; latent panic if anything downstream loads from its slot.
- **L7**: Many `_ => todo!`/`unreachable!` catch-alls that should be
  `return None`/`return false`: `index_match.rs:62-67, 136-138,
  412-414`, `lower/expr_pattern.rs:77`, `exec/project.rs:44`,
  `exec/kv.rs:55-59, 89-91`, `exec/nested_merge.rs:195, 205`,
  `select_item.rs:26, 79, 84, 100`, `extract_params.rs:78, 597, 604`.
- **L8**: `simplify::visit_expr_match_mut` skips `else_expr`
  (`simplify.rs:93-113`). Currently else branches are trivial
  projections, so no observable bug.
- **L9**: `simplify::substitute_let_bindings` doesn't shift binding
  `Arg`s when inserted into deeper scopes (`simplify/expr_let.rs:32-44`).
  Variable-capture bug latent on Let-in-Let, which no lowering path
  emits today.
- **L10**: `fold_expr_cast` strips through `is_null(cast(x, T))`
  unconditionally (`fold/expr_is_null.rs:16-19`). Sound for current
  casts; would diverge if a cast acquired side effects.
- **L11**: `fold/expr_is_superset` and `fold/expr_intersects` ignore
  `NULL` lhs. Toasty docs describe these as "vacuously true/false";
  may be intentional, but PG's `@>` says NULL.
- **L12**: Self-comparison rule only matches `Reference::Field`, not
  `Reference::Column` (`simplify/expr_binary_op.rs:28-37`). After
  lowering, all refs are columns; missed optimisation.
- **L13**: `fold/expr_binary_op` rewrites `x = true → x` and
  `x = false → NOT(x)` without checking that `x` has Bool type
  (`fold/expr_binary_op.rs:48-57`). Relies on upstream type checking.
- **L14**: `Scan` action discards the driver's `prev_cursor`
  (`exec/scan.rs:65-67`). Dead today; inconsistent with `query_pk`.
- **L15**: `Value::Null.is_a(any_type)` returns `true`
  (`value.rs:262`); `VarStore::store` accepts NULL into typed slots.
  Loose typing; would surface as a confusing error elsewhere if a
  driver returned NULL for a non-nullable column.

---

## Cross-cutting themes

Two clusters explain a disproportionate share of the findings:

1. **Two-valued vs three-valued logic in client-side eval** (C7, M2,
   M3, plus several lows). `core::stmt::eval::Expr` was written
   against Rust's `PartialEq` (`Null == Null` is `true`) and Rust's
   `Result` (NULL ordered comparison is an error). It is correct as
   long as eval is only run on operands the fold has already
   eliminated NULL from — but it isn't: filter, guard, nested-merge,
   etc., all call eval on args resolved at runtime. The fix is a
   single concept: every eval site that returns a bool needs to
   return one of `Some(true)`, `Some(false)`, `None` (NULL) and the
   call sites (especially `Filter`) need to treat `None` as
   "exclude row".

2. **`todo!` / `assert!` instead of fall-through in index planning**
   (C5, C6, H3, H5, H6, M7, L7). The index matcher has several
   pattern-match catch-alls that should answer "doesn't help me, ship
   to result filter" but answer with `todo!` instead. Each one is
   reachable from a different legal query shape; together they make
   the DDB path much more fragile than it needs to be.

A third quieter cluster is **load-bearing assertions in the plan/exec
boundary** (C8, H4, H6, M9): things the planner *thinks* it has
already canonicalised, asserted again at exec time. When the assert
fires, it surfaces as a process abort to the user.

---

## False positives ruled out

Items returned by sub-agents that I could not confirm against the
source or that are over-stated:

- *Lower #4* (`prepare_model_returning_for_context` `unreachable!` for
  eager HasMany) — the agent itself noted this is latent (not
  reachable through the current `is_insert_local_eager_relation`
  predicate). Filed at L-tier (omitted above).
- *Lower #6* (`extract_eq_value` decrements `nesting` by 1 unconditionally)
  — agent rated low–medium; the path is reachable only via nesting
  shapes that aren't produced by current lowering. Latent.
- *Eval/Fold #5* (`fold_expr_cast` strips through `is_null(cast(x, T))`)
  — included as L10. Sound for current casts; the agent agreed.
- *Simplify #11* (`try_factor_or` emits `true` that relies on a
  downstream fold) — works today; brittle, but the contract holds.
- *Exec criticals C7a/C7b also reported by eval-fold as "high" #2-#4*
  — same underlying issue, merged into C7.
