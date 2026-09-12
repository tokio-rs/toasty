# Toasty Path System

A `Path` is a rooted reference to a field in the application schema. It names "the field at this location of this model" without committing to any particular database column or expression form. The query engine, the macro-generated query builder, and the schema layer all use paths to talk about fields.

This document describes how paths are represented, how the typed and untyped layers fit together, and where paths appear across the system.

## Overview

A path has a root model (`ModelId`) and an ordered sequence of selections:

- `PathStep::Field(index)` selects a field.
- `PathStep::Variant(variant_id)` selects an embedded enum variant. Field
  indices following this step are local to that variant.

```rust
Path::from_steps(user_model_id, [
    PathStep::Field(2),
    PathStep::Variant(email_variant_id),
    PathStep::Field(0),
])
```

An empty path refers to the root model. Single-field paths store their step
inline and support const construction for generated accessors. Chaining paths
appends all selections, including variant steps.

The core type lives in `crates/toasty-core/src/stmt/path.rs`. `Projection`,
in `crates/toasty-core/src/stmt/projection.rs`, is a separate sequence of
ordinary field indices used for record and database-value projections.

## Why Paths Exist

Toasty queries are written against the application schema (models and fields), but they execute against database tables. Paths give the engine a stable way to refer to a model field across this gap:

- The macro-generated query builder produces paths from typed field accessors — `User::fields().name()` returns a `Path<User, String>`.
- The simplification and lowering phases inspect paths to resolve relation traversals into subqueries and to translate model fields into table columns.
- The schema layer resolves a path to a `Field` definition via `Schema::resolve_field_path`.

Paths are also what `into_stmt()` converts into the engine's expression IR. A path is not itself an expression — it's a reference that becomes one when the engine needs to read a value.

## Typed and Untyped Layers

Paths exist in two layers, mirroring the boundary documented in [Type System](type-system.md):

### `toasty::stmt::Path<T, U>` — typed, user-facing

The wrapper in `crates/toasty/src/stmt/path.rs` carries two phantom type parameters: the root model `T` and the value type `U` at the end of the path. The typed layer:

- Prevents mixing models (`User::fields().email()` on a `Todo` query is a compile error).
- Selects the right comparison methods based on `U` (`is_none` only exists on `Path<T, Option<U>>`, `starts_with` only on string-typed paths).
- Produces `Expr<bool>` and `OrderByExpr` values from typed comparisons.

### `toasty_core::stmt::Path` — untyped, engine-facing

When a statement crosses into the engine via `db.exec()`, the generic parameters are erased and only the untyped path remains. This is the form the simplification, lowering, and planning phases see.

Conversion is one-way: typed paths convert to untyped via `From<Path<T, U>> for stmt::Path`, but the engine never reconstructs the typed form.

## Variant selection

`Path::into_variant(variant_id)` appends a variant-selection step to the
existing path. It preserves the root model and any enclosing selections.
Variant handles use this operation for direct field access and `.matches()`:

```rust
User::fields()
    .contact()
    .email()
    .matches(|e| e.address().eq("alice@example.com"))
```

The closure receives accessors carrying the selected variant. `.matches()`
also adds an explicit discriminant check, so a body that does not reference
any variant field still requires that variant.

Expression construction preserves these selections in `ExprPath`. It does
not assign record offsets. Schema-aware lowering resolves each variant and
field and produces ordinary projections. For
an enum record, the first field after a variant selection gets a one-slot
offset because slot zero holds the discriminant.

## Projection

A `Projection` is a sequence of `usize` field indices. Projections support three internal representations — identity (zero steps), single-step, and multi-step — chosen based on size to avoid allocation for the common cases.

Construct an ordinary field projection from indices:

```rust
let projection = Projection::from([0, 1]);
```

Projection equality and hashing are designed so single-step projections compare and hash like a bare `usize`, which lets `IndexMap` lookups accept either form interchangeably.

## How Paths Are Used

Paths appear in every part of the system that needs to name a field.

### Filter expressions

Comparison methods on typed paths produce filter expressions:

```rust
User::filter(User::fields().name().eq("Alice"))
```

Each method (`eq`, `ne`, `gt`, `ge`, `lt`, `le`, `in_list`, `in_query`, `is_none`, `is_some`, `starts_with`, `like`, `ilike`) calls `Path::into_stmt()` to turn the path into an `Expr`, then wraps it with the appropriate operator.

### Ordering

`Path::asc()` and `Path::desc()` produce `OrderByExpr` values for `Query::order_by`.

### Eager loading via `include`

`Returning::Model` carries `include: Vec<Include>`, where each entry contains a path and optional query modifiers. `Query::include` appends a path:

```rust
let mut q = User::all();
q.include(User::fields().todos());
```

During lowering, `build_include_subquery` walks each include path, resolves it to a relation field, and replaces the field's `Null` placeholder in the returning expression with a subquery that loads the related records.

### Association traversal

`stmt::Association { source: Box<Query>, path: Path }` represents reaching a model by following a relation from another query's results. The simplification phase resolves `Association.path` to a relation field and rewrites the traversal into an explicit subquery. See [Query Engine Architecture](query-engine.md#phase-1-simplification) for how associations are simplified.

### Update and assignment targets

Update statements address fields by path. The same typed accessors used for filters identify the field being assigned.

### Variant filters

`.matches()` on a variant handle uses `Path::into_variant` to select the variant for subsequent field steps. Direct variant field access uses the same selections.

### Schema resolution

`Schema::resolve_field_path` (in `toasty-core/src/schema/app/schema.rs`) takes a path and returns the `Field` it refers to. The simplification phase uses this to turn relation paths into concrete relation metadata.

### Field-bitset metadata

`PathFieldSet` (in `toasty-core/src/stmt/path_field_set.rs`) is a bitset of field indices. It is used by `SparseRecord` to mark which fields are present in a partial record and by the schema mapping layer to track which fields back a column. It is named for the path system because its indices are projection-compatible — a single-step projection equals the corresponding bit — but it does not carry a root.

## Path-to-Expression Lowering

`Path::into_stmt()` converts paths into expressions:

- An empty path becomes `Expr::ref_ancestor_model(0)`.
- A leading field becomes `Expr::ref_self_field(FieldId)`.
- Remaining field-only steps become an ordinary `ExprProject`.
- Steps containing a variant selection remain in `ExprPath`.

Predicate constructors use `Expr::with_path_guards` to attach an `AND` of
variant checks from every value operand. The paths keep their explicit steps
for schema-aware lowering. Guards are present before folding, so a rewrite
such as `flag == true` can simplify the comparison without losing its guard.
Boolean combinations and subqueries retain their own predicate scopes.

This boundary distinguishes testing a value from negating a predicate. For a
variant-scoped optional field `p`, `p.is_some()` requires the selected variant
and a non-null value. `p.is_none().not()` negates the entire guarded null
predicate, so rows of other variants match. Lowering does not infer this
scope from the expression's operator.

Lowering resolves relation paths to foreign keys or subqueries and translates
variant-local fields into ordinary projections or column references. Predicate
guards include every enclosing variant on both operands. For example,
comparing two `primary().human()` paths requires both enum fields to select
`Primary`, even if other variants use the same key columns. The same rule
applies to scalar field comparisons and to both equality and inequality.

Include processing preserves explicit selections while traversing embedded
fields and re-rooting nested includes at relations. Drivers receive ordinary
field projections and discriminant predicates; they do not interpret path
variant steps.

## Further Reading

- [Query Engine Architecture](query-engine.md) — how paths are consumed during simplification, lowering, and planning.
- [Type System](type-system.md) — the compile-time/runtime boundary that the typed and untyped path layers mirror.
