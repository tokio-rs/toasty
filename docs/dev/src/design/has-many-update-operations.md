# `Assign<T>`: Unified Update Mutations

## What exists today

Update builder setters use three different patterns depending on field type:

```rust
// Scalar: .field(impl IntoExpr<T>) → calls assignments.set()
user.update().name("Alice")

// HasMany: .todo(impl IntoExpr<T>) → calls assignments.insert()
user.update().todo(Todo::create().title("Buy groceries"))

// Embedded partial: .with_field(|builder| ...) → closure modifies sub-paths
user.update().with_critter(|c| c.profession("doctor"))
```

Each field type has its own method naming convention, its own argument type, and
its own implied behavior. HasMany generates a singular-named method (`.todo()`)
that can only insert — there's no way to remove or replace associated records
through the update builder. Chaining two `.todo()` calls panics because the
assignment map doesn't support multiple entries for the same field. Embedded
fields need a separate `.with_` method for partial updates. There's no
consistent pattern.

The relation accessor (`user.todos()`) supports insert and remove as standalone
operations, but these can't be combined with field updates in a single call:

```rust
user.todos().insert(&mut db, &todo).await?;
user.todos().remove(&mut db, &todo).await?;
```

## What has been built

### `Assign<T>`

A trait for update mutations, used as the bound on all update builder setters:

```rust
trait Assign<T> {
    fn assign(self, assignments: &mut stmt::Assignments, projection: stmt::Projection);
}
```

### Design principle: update mirrors create

Update builder setters accept the same types as create builder setters. The
default semantics are "set" — replace the field's current value with the new
one. This means any value you can pass to a create builder field also works in
the corresponding update builder field, with the meaning "set this field to
this value."

For has-many fields, passing a create builder or array of create builders
replaces the entire collection. Incremental mutations (insert one, remove one)
require explicit combinators (`stmt::insert`, `stmt::remove`). Combining
multiple operations in a single setter call uses `stmt::apply`.

### Coherence constraints and the `impl_assign_via_expr!` macro

A blanket `impl<E: IntoExpr<T>> Assign<T> for E` is **not possible** due to
Rust's trait coherence rules — the compiler cannot prove that `Assignment<T>`
won't implement `IntoExpr<T>` in the future, even within the same crate. This
would conflict with `Assignment<T>`'s explicit `Assign<T>` impl.

Instead, every `IntoExpr<T>` implementation is paired with a corresponding
`Assign<T>` implementation using the `impl_assign_via_expr!` macro:

```rust
// In assignment.rs — generates an Assign impl that delegates to IntoExpr
macro_rules! impl_assign_via_expr {
    ($source:ty => $target:ty) => {
        impl Assign<$target> for $source {
            fn assign(self, assignments: &mut Assignments, projection: Projection) {
                assignments.set(projection, IntoExpr::<$target>::into_expr(self).untyped);
            }
        }
    };
    // Generic variant
    ({ $($gen:tt)* } $source:ty => $target:ty) => { /* same body with generics */ };
}
```

This macro is invoked alongside every `IntoExpr` impl in `into_expr.rs`:

```rust
impl_into_expr_for_copy! {
    Bool(bool);    // generates both IntoExpr<bool> and Assign<bool> for bool
    I64(i64);
    String(String);
    // ...
}

impl IntoExpr<String> for &str { /* ... */ }
impl_assign_via_expr!(&str => String);
```

The `IntoExpr` impls for `T => T` are concrete (per-type via the macro), not
blanket. The `IntoExpr` and `Assign` impls for `T => Option<T>` and
`&T => Option<T>` are blanket (`impl<T: IntoExpr<T>>`). These don't conflict
because Rust's coherence checker can see that concrete types like `bool` or
`String` can never unify with `Option<U>`.

Some `IntoExpr` impls skip the `Assign` counterpart:

- `&Option<T> => T` — would conflict with `&T => T`
- `Arc<T>`, `Box<T>`, `Rc<T>` forwards — would conflict with `T => T`
- `&T => List<T>` — setting a list to a single reference has no meaning;
  `IntoExpr<List<T>>` exists for use with `stmt::insert`/`stmt::remove`

For model types, the `#[derive(Model)]` macro generates `Assign` impls
alongside `IntoExpr` impls: `Assign<Model>` for the model struct and
`Assign<Model>` / `Assign<Option<Model>>` for create builder structs. The
blanket `Assign<Option<T>> for T` covers the model struct's optional case.

### Assignment combinators

Free functions in `toasty::stmt` wrap an expression to change its assignment
semantics. Each function takes an `impl IntoExpr<T>` and returns an
`Assignment<U>` where `U` reflects the target field type. The key idea: the
generic parameters encode a type-level lift — `insert` takes an expression of
`T` (a single item) and produces an assignment for `List<T>` (a collection).

```rust
// toasty::stmt

/// Insert a value into a collection field.
/// Takes an expression of T, produces an assignment to List<T>.
fn insert<T>(expr: impl IntoExpr<T>) -> Assignment<List<T>>

/// Remove a value from a collection field.
/// Takes an expression of T, produces an assignment to List<T>.
fn remove<T>(expr: impl IntoExpr<T>) -> Assignment<List<T>>

/// Replace a field's value entirely.
/// Takes an expression of T, produces an assignment to T.
fn set<T>(expr: impl IntoExpr<T>) -> Assignment<T>

/// Apply multiple operations to a single field.
/// Takes an array or vec of assignments and applies each in order.
fn apply<T>(ops: impl Assign<T>) -> Assignment<T>
```

`Assignment<T>` implements `Assign<T>`, so these return values can be passed
directly to any setter that accepts `impl Assign<T>`.

### Scalars: no visible change

Passing a plain value to a scalar field still means "set":

```rust
user.update().name("Alice").exec(&mut db).await?;
```

Scalar setters accept `impl Assign<T>`. `&str` implements `Assign<String>`
(via `impl_assign_via_expr!`), so the call site is unchanged.

### Has-many

The update builder generates a `.todos()` method (plural, matching the field
name) instead of today's singular `.todo()`. The method accepts
`impl Assign<List<Todo>>`.

#### Replace (default)

Passing a value or array directly to a has-many setter replaces the entire
collection. This mirrors the create builder, where passing values means "these
are the associated records."

```rust
// Replace all todos with a single new one
user.update()
    .todos(Todo::create().title("Only todo"))
    .exec(&mut db)
    .await?;

// Replace all todos with multiple new ones
user.update()
    .todos([
        Todo::create().title("Buy groceries"),
        Todo::create().title("Walk the dog"),
    ])
    .exec(&mut db)
    .await?;
```

Arrays and `Vec`s of `impl IntoExpr<T>` implement `Assign<List<T>>` with set
semantics — all current members are disassociated (following the optional/required
foreign key rules described below), then the new set is associated.

Pass an empty array to clear the collection:

```rust
user.update()
    .todos(stmt::set::<List<Todo>>([]))
    .exec(&mut db)
    .await?;
```

#### Insert

Use `stmt::insert` to add a record to the collection without affecting existing
members:

```rust
user.update()
    .todos(stmt::insert(Todo::create().title("Buy groceries")))
    .exec(&mut db)
    .await?;
```

`stmt::insert` takes an `impl IntoExpr<Todo>` and returns an
`Assignment<List<Todo>>`, which `.todos()` accepts.

#### Remove

Use `stmt::remove` to detach a record from the set:

```rust
user.update()
    .todos(stmt::remove(&todo_a))
    .exec(&mut db)
    .await?;
```

`stmt::remove` takes an `impl IntoExpr<Todo>` (the item to remove) and returns
an `Assignment<List<Todo>>` (a mutation on the collection).

What "remove" means depends on the belongs-to side of the relationship:

- **Optional foreign key** (`user_id: Option<Id>`): The todo's `user_id` is set
  to `NULL`. The todo continues to exist.
- **Required foreign key** (`user_id: Id`): The todo is deleted. It can't exist
  without a user.

#### Multiple operations

Use `stmt::apply` to combine multiple operations on a single field:

```rust
user.update()
    .name("Alice")
    .todos(stmt::apply([
        stmt::insert(Todo::create().title("Buy groceries")),
        stmt::insert(Todo::create().title("Walk the dog")),
        stmt::remove(&old_todo),
    ]))
    .exec(&mut db)
    .await?;
```

`stmt::apply` takes an `impl Assign<T>` (here, an array of
`Assignment<List<Todo>>`) and returns `Assignment<List<Todo>>`. Each operation
is applied in order.

This works from both instance updates and query updates:

```rust
User::filter_by_id(user_id)
    .update()
    .todos(stmt::apply([
        stmt::insert(Todo::create().title("New task")),
        stmt::remove(&old_todo),
    ]))
    .exec(&mut db)
    .await?;
```

### Embedded types

Today, partially updating an embedded enum requires a separate `.with_` method:

```rust
user.update()
    .with_critter(|c| c.profession("doctor"))
    .exec(&mut db)
    .await?;
```

The `.with_critter()` method stays for now. The `.critter()` setter accepts
`impl Assign<Creature>`. Full replacement passes a value directly (plain values
implement `Assign<T>`):

```rust
user.update()
    .critter(Creature::Human { profession: "doctor".into(), age: 30 })
    .exec(&mut db)
    .await?;
```

For partial updates, `stmt::patch` takes a field path and a value, returning
`Assignment<Creature>` without needing a closure or any generated builder type.
The path reuses the existing `fields()` accessor chain, which already returns
typed `Path<T, U>` values:

```rust
fn patch<T, U>(path: Path<T, U>, value: impl Assign<U> + 'static) -> Assignment<T>
```

`Path<T, U>` carries both the origin type `T` (the embedded type being patched)
and the leaf type `U` (the field being set). The origin type is what makes
`patch` return `Assignment<Creature>` — no extra type information needed at the
call site. `Path<T, U>` is already implemented in `toasty::stmt::Path`.

The value accepts `Assignment<U>`, so plain values must be wrapped with
`stmt::set`. Nested patches also work because `stmt::patch` returns
`Assignment<U>`.

```rust
user.update()
    .critter(stmt::patch(Creature::fields().human().profession(), stmt::set("doctor")))
    .exec(&mut db)
    .await?;
```

This produces `assignments.set([critter, profession], "doctor")` — updating the
profession column without touching the age column.

Multiple sub-field mutations use `stmt::apply`:

```rust
user.update()
    .critter(stmt::apply([
        stmt::patch(Creature::fields().human().profession(), stmt::set("doctor")),
        stmt::patch(Creature::fields().human().age(), stmt::set(35)),
    ]))
    .exec(&mut db)
    .await?;
```

Each `stmt::patch` returns `Assignment<Creature>`. `stmt::apply` accepts the
array and returns `Assignment<Creature>`, which `.critter()` accepts.

#### Nested patching

Because `Assignment<T>` implements `Assign<T>`, `stmt::patch` composes with
itself. When an embedded type contains another embedded type, the inner patch
becomes the value argument to the outer patch:

```rust
user.update()
    .kind(
        stmt::patch(
            Kind::variants().admin().perm(),
            stmt::patch(Permission::fields().everything(), stmt::set(true)),
        ),
    )
    .exec(&mut db)
    .await?;
```

Here `stmt::patch(Permission::fields().everything(), stmt::set(true))` returns
`Assignment<Permission>`. The outer `stmt::patch` accepts it as the value for
the `perm` path, returning
`Assignment<Kind>`. The nesting works to arbitrary depth — each layer resolves
one level of the field path.

This approach avoids generating update builder types for embedded types
entirely — the `fields()` path infrastructure already exists and provides full
type safety.

The `.with_critter()` closure method remains for now but can be removed once
`stmt::patch` is implemented.

### Setter bounds by field type

| Field type | Setter bound | Plain value | `stmt::` combinator |
|---|---|---|---|
| Scalar (`String`) | `impl Assign<T>` | Set the field | `stmt::set` (explicit) |
| BelongsTo (`User`) | `impl Assign<T>` | Set the association | — |
| HasOne (`Profile`) | `impl Assign<T>` | Set the association | — |
| HasMany (`List<Todo>`) | `impl Assign<List<T>>` | Replace the collection | `stmt::insert`, `stmt::remove`, `stmt::apply` |
| Embedded (`Creature`) | `impl Assign<T>` | Full replacement | `stmt::patch`, `stmt::apply` |

All setters use `impl Assign<T>`. Plain values satisfy this bound for scalar,
relation, and embedded fields (via `impl_assign_via_expr!` and codegen). For
has-many fields, the bound is `impl Assign<List<T>>`. Arrays and `Vec`s of
`impl IntoExpr<T>` satisfy this bound with set semantics. Individual
insert/remove operations require explicit combinators, and combining multiple
operations uses `stmt::apply`. The `.with_` methods for embedded types remain
for now alongside `stmt::patch`.

### Summary

| Today | With `Assign` |
|---|---|
| `.name("Alice")` | `.name("Alice")` (unchanged) |
| `.todo(expr)` | `.todos(stmt::insert(expr))` |
| `.todo(a).todo(b)` | `.todos(stmt::apply([stmt::insert(a), stmt::insert(b)]))` |
| _not possible_ | `.todos(stmt::remove(&todo))` |
| _not possible_ | `.todos([create_a, create_b])` (replace) |
| _not possible_ | `.todos(stmt::apply([stmt::insert(a), stmt::remove(b)]))` |
| `.critter(value)` | `.critter(value)` (unchanged, plain value = full replace) |
| `.with_critter(\|c\| c.profession("x"))` | `.critter(stmt::patch(path, stmt::set("x")))` |
| _not possible_ | `.kind(stmt::patch(path, stmt::patch(inner_path, stmt::set(val))))` |
