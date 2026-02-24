# Option Association Preload Fix

## Problem

After preloading `HasOne<Option<T>>`, calling `.get()` panics with "association not
loaded" even though the preload ran successfully. The root cause is that `Value::Null`
is overloaded in the association load path:

- `HasOne::load(Value::Null)` → `Self::default()` (the "unloaded" state)
- Nested merge produces `Value::Null` when no matching row found

For `HasOne<Option<T>>` these two meanings collide: "not loaded" and "loaded but no
related record exists" are indistinguishable.

## Solution

Encode nullable single-value association results as variant Records in the nested merge
projection, so `Value::Null` can remain exclusively the "unloaded" sentinel:

| Value | Meaning |
|---|---|
| `Value::Null` | Unloaded (unchanged) |
| `Value::Record([0])` | Loaded as `None` |
| `Value::Record([1, Record([..fields])])` | Loaded with model data |

The variant ID makes `None` unambiguous even if the association model is a unit struct
(zero fields), which becomes relevant when partial model loading is added.

The encoding is applied in the **plan-time projection** for nullable single-value
associations, not inside the nested merge execution logic. `exec/nested_merge.rs`
continues to emit `Value::Null` for a no-row result; the projection expression
transforms it before it reaches `HasOne::load`.

## Changes Required

### New: `Expr::Conditional` (`toasty-core`)

A standard ternary expression:

```
Conditional { condition: Expr, then: Expr, else_: Expr }
```

Evaluates `condition` (a bool expr); yields `then` or `else_` accordingly. Both
branches share the parent arg scope — no new binding mechanism needed. Maps to
SQL `CASE WHEN … THEN … ELSE … END` when lowered, though for this fix it only
appears in nested merge projections and never reaches the SQL layer.

### New: `nullable: bool` on `hir::Arg::Sub`

Set during lowering when an include is processed for a nullable `has_one` or
`belongs_to` field. Carries the field's nullable flag through to the plan stage.

### Updated: `plan/nested_merge.rs` — `build_projection_from_expr`

When building the projection arg for a nullable single-value nested child, wrap it:

```
// was:
*expr = Expr::arg(N)

// becomes (when arg.nullable && nested_child.single):
*expr = Expr::Conditional {
    condition: Expr::is_null(Expr::arg(N)),
    then:  Expr::record([Value::UInt(0)]),
    else_: Expr::record([Value::UInt(1), Expr::arg(N)]),
}
```

### Updated: `HasOne::load` and `BelongsTo::load`

Match on the variant wrapper:

```
Value::Null        → Self::default()                           // unloaded
Record([0])        → Self { value: Some(Box::new(T::load(Value::Null)?)) }
Record([1, inner]) → Self { value: Some(Box::new(T::load(inner)?)) }
```

For `HasOne<Option<T>>`, the `Record([0])` arm passes `Value::Null` to `T::load`
where `T = Option<Profile>`. `Option<T>::Primitive::load(Value::Null)` already returns
`Ok(None)` — no change needed there.

## What Does Not Change

- `exec/nested_merge.rs` — still emits `Value::Null` for no-row; projection handles encoding.
- `IntoExpr` impls, `lower.rs`, `ExprIsNull`, SQL generation, all drivers — untouched.
- `Option<T>::Primitive::load` — `Value::Null` → `None` path unchanged.
- DB schema — no changes.
- `HasMany` — unaffected; uses a list, not the single-value path.
