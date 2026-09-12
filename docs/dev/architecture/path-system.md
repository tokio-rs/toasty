# Toasty Path System

A `Path` is a rooted reference to a field in the application schema. It names "the field at this location of this model" without committing to any particular database column or expression form. The query engine, the macro-generated query builder, and the schema layer all use paths to talk about fields.

This document describes how paths are represented, how the typed and untyped layers fit together, and where paths appear across the system.

## Overview

A path has two parts:

- A **root** — either a model (`PathRoot::Model(ModelId)`) or a specific enum variant (`PathRoot::Variant { parent, variant_id }`).
- A **projection** — a sequence of field indices that navigate through the rooted value.

```
Path {
    root: PathRoot::Model(user_model_id),
    projection: [2],          // third field on User
}
```

A path with an empty projection refers to the root itself. A path with a non-empty projection navigates one or more steps into the value.

The core type lives in `toasty-core/src/stmt/path.rs`. `Projection` (in `toasty-core/src/stmt/projection.rs`) is a small inline-optimized vector of field indices that supports identity (zero steps), single-step, and multi-step forms.

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

## Roots

`PathRoot` has two variants:

### `Model(ModelId)`

The default root. Subsequent projection steps index into the model's declared fields.

### `Variant { parent: Box<Path>, variant_id: VariantId }`

Used when a path navigates into a specific variant of an embedded enum. `parent` is the path that reaches the enum field itself; `variant_id` records which variant the path enters.

Variant-rooted paths support the closure-based `.matches()` API on enum variant handles. For example:

```rust
User::fields()
    .contact()           // Path<User, ContactInfo>
    .email()             // EmailVariantHandle<User>
    .matches(|e| e.address().eq("alice@example.com"))
```

`.matches()` does two things:

1. ANDs in a discriminant check (`is_variant`) so the filter only matches rows whose `contact` field is the `Email` variant.
2. Hands the closure a fields struct whose path was converted via `Path::into_variant(variant_id)`, so accessors inside the closure produce variant-rooted paths.

A variant-rooted path converts to an `ExprVariant` — the payload of the enum value as that variant — and its projection steps are the variant's local field positions. The expression carries no variant check; see [Variant guards](#variant-guards) for where the check comes from. Lowering resolves the selection against the enum's storage layout (discriminant column plus per-variant columns), so the physical record offsets stay inside the engine.

## Projection

A `Projection` is a sequence of `usize` field indices. Projections support three internal representations — identity (zero steps), single-step, and multi-step — chosen based on size to avoid allocation for the common cases.

The `path!` macro in `toasty-core/src/macros.rs` builds projections from a dot-separated index list:

```rust
let p: Path = path![.0 .1];   // two-step projection
```

Projection equality and hashing are designed so single-step projections compare and hash like a bare `usize`, which lets `IndexMap` lookups accept either form interchangeably.

## How Paths Are Used

Paths appear in every part of the system that needs to name a field.

### Filter expressions

Comparison methods on typed paths produce filter expressions:

```rust
User::filter(User::fields().name().eq("Alice"))
```

Each method (`eq`, `ne`, `gt`, `ge`, `lt`, `le`, `in_list`, `in_query`, `is_none`, `is_some`, `starts_with`, `like`, `ilike`) calls `Path::into_stmt()` to turn the path into an `Expr`, wraps it with the appropriate operator, and passes the result through `Expr::<bool>::from_predicate`, which adds the variant guards described below.

### Variant guards

A path into an enum variant (`contact().email().address()`) converts to an expression that selects the variant's payload without checking the variant. A predicate over such an operand only means something for rows of that variant, so every predicate constructor in the typed layer (`Path::eq`, `Expr::<T>::eq`, `Expr::<Option<T>>::is_none`, `in_list`, and the rest) runs `Expr::with_variant_guards` on the predicate it builds. This walks the predicate's operands, emits an `is_variant` check for every `ExprVariant` found — on either operand, and for each enclosing variant of a nested selection, outermost first — and ANDs the checks in front of the predicate.

The guards are fixed when the predicate is built, before it is combined with others, which sets the boolean scope:

- `a.ne(b)` requires the variants of both `a` and `b`; rows of other variants do not match, even when the variants share a column.
- `a.eq(b).not()` is `NOT (is_variant AND a = b)`: it negates the guarded equality as a whole and matches rows of other variants.
- Operands that are themselves predicates (`and`, `or` operands, subqueries) are not searched: they were built by these constructors and already carry their checks.

The `is_{variant}()` and `.matches()` methods build their `is_variant` check the same way, so a check on an enum reached through a variant of an enclosing enum requires the outer variant too.

### Ordering

`Path::asc()` and `Path::desc()` produce `OrderByExpr` values for `Query::order_by`.

### Eager loading via `include`

`Returning::Model` carries `include: Vec<Path>`. `Query::include` appends a path:

```rust
let mut q = User::all();
q.include(User::fields().todos());
```

During lowering (`engine/lower.rs:889`), `build_include_subquery` walks each include path, resolves it to a relation field, and replaces the field's `Null` placeholder in the returning expression with a subquery that loads the related records.

### Association traversal

`stmt::Association { source: Box<Query>, path: Path }` represents reaching a model by following a relation from another query's results. The simplification phase resolves `Association.path` to a relation field and rewrites the traversal into an explicit subquery. See [Query Engine Architecture](query-engine.md#phase-1-simplification) for how associations are simplified.

### Update and assignment targets

Update statements address fields by path. The same typed accessors used for filters identify the field being assigned.

### Variant filters

`.matches()` on a variant handle uses `Path::into_variant` to root subsequent path steps at the variant. This is the only way a path can navigate through an enum to reach a variant-specific field.

### Schema resolution

`Schema::resolve_field_path` (in `toasty-core/src/schema/app/schema.rs`) takes a path and returns the `Field` it refers to. The simplification phase uses this to turn relation paths into concrete relation metadata.

### Field-bitset metadata

`PathFieldSet` (in `toasty-core/src/stmt/path_field_set.rs`) is a bitset of field indices. It is used by `SparseRecord` to mark which fields are present in a partial record and by the schema mapping layer to track which fields back a column. It is named for the path system because its indices are projection-compatible — a single-step projection equals the corresponding bit — but it does not carry a root.

## Path-to-Expression Lowering

`Path::into_stmt()` is the bridge from path to expression IR. The conversion depends on the root:

**Model root:**
- Empty projection → `Expr::ref_ancestor_model(0)` (the root record itself).
- Non-empty projection → `Expr::ref_self_field(FieldId)` for the first step, followed by `Expr::project` for any remaining steps.

**Variant root:**
- Recursively converts the parent path to an expression that reaches the enum field and wraps it in `Expr::variant(parent, variant_id)`.
- Empty projection → the selection itself.
- Non-empty projection → `Expr::project(selection, steps)`, indexing the variant's fields by their local positions.

Chaining a variant-rooted path onto another path (`Path::chain`) keeps the selection: the result selects the same variant at the point the first path reaches the enum.

The result is consumed by the lowering phase, which translates field references to column references using the `TableToModel` mapping and replaces each `ExprVariant` with the record of the selected variant's column expressions (the enum decode's arm for that variant, discriminant dropped, decoding casts kept). The `is_variant` guards lower to discriminant comparisons. No `ExprVariant` reaches a driver. See [Query Engine Architecture](query-engine.md#phase-2-lowering) for how the resulting expressions become table-level statements.

## Further Reading

- [Query Engine Architecture](query-engine.md) — how paths are consumed during simplification, lowering, and planning.
- [Type System](type-system.md) — the compile-time/runtime boundary that the typed and untyped path layers mirror.
