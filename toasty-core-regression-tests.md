# toasty-core Regression Test Audit

This document lists toasty-core components that are large or complex enough to warrant
dedicated tests in `crates/toasty-core/tests/`, ranked roughly by priority.

## Already tested

| Component | Files |
|---|---|
| Expression evaluation (`eval.rs`) | `stmt_eval_*.rs` (18 files) |
| Expression construction from values | `stmt_from_expr.rs`, `stmt_from_value.rs`, `stmt_try_from_value.rs` |
| Type variants (basic existence/equality) | `stmt_infer.rs` |
| Schema validation (missing model) | `schema_missing_model.rs` |
| Projection | `stmt_projection.rs` |
| Index helpers | `stmt_index.rs` |

## Not yet tested — recommended additions

### 1. Type inference — `Value::infer_ty()` (HIGH PRIORITY)

**Location**: `src/stmt/value.rs`
**Proposed file**: `stmt_infer_value_ty.rs`

`Value::infer_ty()` converts a runtime value to its `stmt::Type`. Every value variant
should return the correct type, and the empty-list edge case (`Type::list(Type::Null)`)
is easy to miss. Nested containers (list-of-list, record-of-record) should also be covered.

### 2. Type inference — `ExprContext::infer_expr_ty()` (HIGH PRIORITY)

**Location**: `src/stmt/cx.rs`
**Proposed file**: `stmt_infer_expr_ty.rs`

`ExprContext::infer_expr_ty()` drives the query-planning phase. All schema-free branches
can be exercised using `ExprContext::new_free()`:

- Boolean predicates (`And`, `Or`, `IsNull`, `BinaryOp`) → `Type::Bool`
- `Cast` → returns the explicit target type
- `List` → `Type::list(infer(items[0]))`
- `Record` → `Type::Record(per-field types)`
- `Arg` → looked up by position from the `args` slice; test multiple positions and
  the nested-scope (nesting > 0) path
- `Map` — creates a new argument scope from the list item type; test that the
  resulting type wraps the mapped type in a `List`
- `Project` — unwraps field or list-item type one step at a time

### 3. Type equivalence — `Type::is_equivalent()` (MEDIUM PRIORITY)

**Location**: `src/stmt/ty.rs`
**Proposed file**: `stmt_type_equivalent.rs`

`is_equivalent()` is like `==` but `Null` matches anything (commutatively). The
following need explicit tests:

- Identical scalars are equivalent
- Different scalars are not
- `Null` is equivalent to every other type (both orderings)
- `List<Null>` is equivalent to `List<String>` (recursive Null propagation)
- `Record<[I32, Null]>` is equivalent to `Record<[I32, String]>`
- Records of different lengths are not equivalent
- `Unknown` is only equivalent to itself (not to `Null`)

### 4. Value type checking — `Value::is_a()` (MEDIUM PRIORITY)

**Location**: `src/stmt/value.rs`
**Proposed file**: `stmt_value_is_a.rs`

`Value::is_a(&Type)` is used during evaluation to validate that a value matches an
expected type. Coverage should include:

- `Value::Null` is a member of every type
- Scalar values match their own type and no other
- `Value::List([])` (empty) is a member of any `List<T>`
- Non-empty lists match only a list of the same element type
- Records must have the same length and matching field types

### 5. Expression property methods (LOW PRIORITY)

**Location**: `src/stmt/expr.rs`
**Proposed file**: `stmt_expr_properties.rs`

`is_stable()`, `is_const()`, and `is_eval()` classify expressions for the query
optimiser. Worth covering:

- Constants (`Expr::Value`) are stable, const, and eval
- `Expr::Reference` is not const, not eval
- `Expr::Default` is not stable, not const
- Composed expressions (e.g. `And(const, const)`) inherit child properties
- `Expr::Map` with a non-const base is not const

### 6. Schema verification — `verify::relations_are_indexed` (LOW PRIORITY)

**Location**: `src/schema/verify/relations_are_indexed.rs`
**Proposed file**: `schema_verify.rs`

The existing `schema_missing_model.rs` covers one validation path. Additional cases:

- A `has_many` relation without an index on the FK column → error
- A `belongs_to` relation with an index → OK
- Multiple relations, only some missing indexes → collect all errors

### 7. DB schema diffing — `schema::db::Diff` (LOW PRIORITY)

**Location**: `src/schema/db/diff.rs`
**Proposed file**: `schema_db_diff.rs`

Generating a `Diff` between two `db::Schema` values drives migrations. The diff should
reflect added tables, dropped tables, added columns, and dropped columns correctly.
