# DynamoDB: OR Predicates in Index Key Conditions


## Problem

DynamoDB's `KeyConditionExpression` does not support OR — neither for partition keys nor
sort keys. This means queries like `WHERE user_id = 1 OR user_id = 2` on an indexed field
are currently broken for DynamoDB.

The engine must detect OR in index key conditions and fan them out into N individual
DynamoDB `Query` calls — one per OR branch — then concatenate the results.

A secondary motivation: the batch-load mechanism used for nested association preloads
(`rewrite_stmt_query_for_batch_load_nosql`) produces `ANY(MAP(arg[input], pred))`, which
at exec time expands to OR via `simplify_expr_any`. This hits the same DynamoDB
restriction and is addressed by the same fix.

## Where OR Can Reach a Key Condition

Only two engine actions use `KeyConditionExpression`:

- **`QueryPk`** — queries the primary table when exact PK keys cannot be extracted
- **`FindPkByIndex`** — queries a GSI to retrieve primary keys

`GetByKey` uses `BatchGetItem` (explicit key values, no expression), so OR is never
relevant there. `pk = v1 OR pk = v2` on the primary key produces
`IndexPlan.key_values = Some([v1, v2])`, routing to `GetByKey` directly — no issue.

### `QueryPk`

OR reaches `QueryPk.pk_filter` when `IndexPlan.key_values` is `None`:

- **User-specified OR on sort key**: `WHERE pk = v AND (sk >= s1 OR sk >= s2)` —
  range predicates have no extractable key values.
- **Batch-load** (e.g. a HasMany where the FK is the partition key of the child's
  composite primary key): `rewrite_stmt_query_for_batch_load_nosql` produces
  `ANY(MAP(arg[input], fk = arg[0]))`. The list is a runtime input, so `key_values`
  is `None`. At exec time `simplify_expr_any` expands it to OR.

### `FindPkByIndex`

`FindPkByIndex.filter` is the output of `partition_filter`, which isolates index key
conditions from non-key conditions. `partition_filter` on `AND` distributes cleanly:
`status = active AND (user_id = 1 OR user_id = 2)` produces
`index_filter = user_id = 1 OR user_id = 2` and `result_filter = status = active`.

OR reaches it in the same two ways as `QueryPk`:

- **User-specified OR**: `WHERE user_id = 1 OR user_id = 2` on a GSI partition key.
- **Batch-load**: same `ANY(MAP(arg[input], pred))` expansion path as above.

### Mixed OR Operands

`partition_filter` currently has a `todo!()` for OR operands that contain both index and
non-index parts — e.g. `(pk = 1 AND status = a) OR pk = 2`.

This is in scope. Strategy:

- Extract key conditions from each OR branch to build the fan-out:
  `ANY(MAP([1, 2], pk = arg[0]))`
- Apply the **full original predicate** as an in-memory post-filter:
  `(pk = 1 AND status = a) OR pk = 2`

This is conservative but correct, and consistent with how `post_filter` is already used.

## Canonical Form: `ANY(MAP(key_list, per_call_pred))`

All OR cases are represented uniformly as `ANY(MAP(key_list, per_call_pred))`:

- `key_list` — one entry per required `Query` call; each entry has one value per key
  column (scalar for partition-key-only, `Value::Record` for partition + sort key)
- `per_call_pred` — the key condition for one call, referencing element fields as
  `arg[0]`, `arg[1]`, ...

**Single key column** — `user_id = 1 OR user_id = 2`:
```
ANY(MAP([1, 2], user_id = arg[0]))
```

**Composite key** — `(todo_id = t1 AND step_id >= s1) OR (todo_id = t2 AND step_id >= s2)`:
```
ANY(MAP([(t1, s1), (t2, s2)], todo_id = arg[0] AND step_id >= arg[1]))
```

**Batch-load** — `ANY(MAP(arg[input], todo_id = arg[0]))` — already in canonical form;
no structural change needed, only the exec fan-out behavior changes.

## Design

### 1. Capability Flag

```rust
/// Whether OR is supported in index key conditions (e.g. DynamoDB KeyConditionExpression).
pub index_or_predicate: bool,
```

DynamoDB: `false`. All other backends: `true` (SQL backends never use these actions).

### 2. `IndexPlan` Output Contract

```rust
pub(crate) struct IndexPlan<'a> {
    pub(crate) index: &'a Index,

    /// Filter to push to the index. Guaranteed form:
    ///
    /// | Condition                          | Form                                             |
    /// |------------------------------------|--------------------------------------------------|
    /// | No OR                              | plain expr — `user_id = 1`                       |
    /// | OR, `index_or_predicate = true`    | `Expr::Or([branch1, branch2, ...])`              |
    /// | OR, `index_or_predicate = false`   | `ANY(MAP(Value::List([v1, ...]), per_call_pred))` |
    /// | Batch-load (any capability)        | `ANY(MAP(arg[input], per_call_pred))`            |
    pub(crate) index_filter: stmt::Expr,

    /// Non-index conditions applied in-memory after results return from each call.
    pub(crate) result_filter: Option<stmt::Expr>,

    /// Full original predicate applied after all fan-out results are collected.
    /// Set for mixed OR operands — see §"Mixed OR Operands".
    pub(crate) post_filter: Option<stmt::Expr>,

    /// Literal key values for direct lookup: a `Value::List` of `Value::Record` entries,
    /// one per lookup. Set by `partition_filter` when all key columns have literal equality
    /// matches. When `Some`, the planner routes to `GetByKey` and ignores `index_filter`.
    /// May coexist with a canonical `ANY(MAP(...))` `index_filter` — both are produced
    /// simultaneously by `partition_filter`; the planner always prefers `GetByKey`.
    pub(crate) key_values: Option<stmt::Value>,
}
```

**Planner routing (primary key path):**
```
key_values.is_some()          → GetByKey (BatchGetItem)
index_filter = ANY(MAP(...))  → fan-out via QueryPk × N
otherwise                     → single QueryPk call
```

### 3. Key Value Extraction in `index_match`

`partition_filter` extracts literal key values during filter partitioning, setting
`key_values` when all key columns have literal equality matches. This replaces the
current `try_build_key_filter` (kv.rs) post-hoc re-analysis of `index_filter`.

**What moves into `index_match`:** walking each OR branch, reading the RHS of each key
column's equality predicate, assembling `Value::List([Value::Record([v0, ...]), ...])`.

**What stays in the planner:** constructing `eval::Func` from `key_values` to drive the
`GetByKey` operation — a mechanical wrap requiring no further expression analysis.

**Why this matters for ordering:** if `partition_filter` produced the canonical
`ANY(MAP([1,2], pk=arg[0]))` form first, the downstream `try_build_key_filter` `Or` arm
would never fire, silently breaking the `GetByKey` path for primary key OR queries.
Extracting key values inside `partition_filter` eliminates this conflict — both outputs
are produced together.

### 4. Planner Invariant

When `!capability.index_or_predicate`, neither `FindPkByIndex.filter` nor
`QueryPk.pk_filter` contains `Expr::Or`. OR is always restructured into
`ANY(MAP(arg[i], per_call_pred))` by `partition_filter` before reaching the exec layer.

**Batch-load path** — `ANY(MAP(...))` is already produced upstream; the invariant holds.
Only the exec fan-out needs fixing.

**User-specified OR path** — `partition_filter` produces canonical form directly. The
planner consumes `IndexPlan.index_filter` as-is; no rewrite in `plan_secondary_index_execution`
or `plan_primary_key_execution`. For mixed OR operands, `partition_filter` additionally
sets `IndexPlan.post_filter` to the full original predicate.

### 5. Exec Fan-out

Both `action_find_pk_by_index` and `action_query_pk` receive the same treatment.

After substituting inputs into the filter, check for `ANY(MAP(arg[i], per_call_pred))`:

- **If present**: iterate over `input[i]` element by element; substitute each into
  `per_call_pred` and issue one driver call; concatenate results. Do **not** call
  `simplify_expr_any` — it would re-expand to OR.
- **Otherwise**: unchanged single-call path.

### 6. DynamoDB Driver

Revert the temporary OR-splitting workaround in `exec_find_pk_by_index`. The driver
is a dumb executor of a single valid key condition.

## Summary of Changes

| Location | Change |
|---|---|
| `Capability` | Add `index_or_predicate: bool`; `false` for DynamoDB |
| `IndexPlan` | Add `key_values: Option<stmt::Value>` field |
| `index_match` / `partition_filter` | `Or` arm: produce canonical `ANY(MAP(...))` when `!index_or_predicate`; extract `key_values`; fix mixed OR `todo!()` |
| `plan_primary_key_execution` | Route on `key_values` / `ANY(MAP(...))` instead of calling `try_build_key_filter` |
| `plan_secondary_index_execution` | No rewrite needed; consumes `IndexPlan.index_filter` as-is |
| `kv.rs` / `try_build_key_filter` | Remove (literal case now handled by `index_match`) |
| `action_find_pk_by_index` | Fan out over `ANY(MAP(...))` — one driver call per element; skip `simplify_expr_any` |
| `action_query_pk` | Same fan-out treatment |
| DynamoDB `exec_find_pk_by_index` | Revert OR-splitting workaround |
