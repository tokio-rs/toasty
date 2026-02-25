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
| Projection — `Projection` struct, `Project` trait, `Expr::project/entry`, `Value::entry` | `stmt_projection.rs`, `stmt_eval_project.rs`, `stmt_value_entry.rs` |
| Index helpers | `stmt_index.rs` |
| Type inference — `Value::infer_ty()` | `stmt_infer_value_ty.rs` |
| Type inference — `ExprContext::infer_expr_ty()` | `stmt_infer_expr_ty.rs`, `stmt_infer_expr_reference_ty.rs` |
| Value type checking — `Value::is_a()` | `stmt_value_is_a.rs` |

## Not yet tested — recommended additions

### 1. Type equivalence — `Type::is_equivalent()` (MEDIUM PRIORITY)

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

### 2. Expression property methods (LOW PRIORITY)

**Location**: `src/stmt/expr.rs`
**Proposed file**: `stmt_expr_properties.rs`

`is_stable()`, `is_const()`, and `is_eval()` classify expressions for the query
optimiser. Worth covering:

- Constants (`Expr::Value`) are stable, const, and eval
- `Expr::Reference` is not const, not eval
- `Expr::Default` is not stable, not const
- Composed expressions (e.g. `And(const, const)`) inherit child properties
- `Expr::Map` with a non-const base is not const

### 4. Schema verification — `verify::relations_are_indexed` (LOW PRIORITY)

**Location**: `src/schema/verify/relations_are_indexed.rs`
**Proposed file**: `schema_verify.rs`

The existing `schema_missing_model.rs` covers one validation path. Additional cases:

- A `has_many` relation without an index on the FK column → error
- A `belongs_to` relation with an index → OK
- Multiple relations, only some missing indexes → collect all errors

### 5. DB schema diffing — `schema::db::Diff` (LOW PRIORITY)

**Location**: `src/schema/db/diff.rs`
**Proposed file**: `schema_db_diff.rs`

Generating a `Diff` between two `db::Schema` values drives migrations. The diff should
reflect added tables, dropped tables, added columns, and dropped columns correctly.
