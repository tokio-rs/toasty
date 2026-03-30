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

A trait for explicit update mutations on collection and embedded fields:

```rust
trait Assign<T> {
    fn assign(self, assignments: &mut stmt::Assignments, projection: stmt::Projection);
}
```

`Assign<T>` is implemented by `Assignment<T>` (the return type of the `insert`,
`remove`, `set`, and `patch` combinators), by arrays `[Q; N] where Q: Assign<T>`,
and by `Vec<Q>`.

A blanket `impl<E: IntoExpr<T>> Assign<T> for E` is **not possible** due to
Rust's trait coherence rules — the compiler cannot prove that `Assignment<T>`
won't implement `IntoExpr<T>` in the future, so the blanket would conflict with
`Assignment<T>`'s explicit `Assign<T>` impl. This means scalar and relation
field setters continue to accept `impl IntoExpr<T>` (a plain value means "set"),
while has-many and embedded field setters accept `impl Assign<T>` (requiring an
explicit combinator). Both paths write into the same `Assignments` map.

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
```

`Assignment<T>` implements `Assign<T>`, so these return values can be passed
directly to any setter that accepts `impl Assign<T>`.

### Scalars: no visible change

Passing a plain value to a scalar field still means "set":

```rust
user.update().name("Alice").exec(&mut db).await?;
```

Scalar setters accept `impl IntoExpr<T>`. `&str` implements
`IntoExpr<String>`, so the call site is unchanged.

### Has-many

The update builder generates a `.todos()` method (plural, matching the field
name) instead of today's singular `.todo()`. The method accepts
`impl Assign<List<Todo>>`.

#### Insert

Use `stmt::insert` to add a record to the collection:

```rust
user.update()
    .todos(stmt::insert(Todo::create().title("Buy groceries")))
    .exec(&mut db)
    .await?;
```

`stmt::insert` takes an `impl IntoExpr<Todo>` and returns an
`Assignment<List<Todo>>`, which `.todos()` accepts. A bare value won't compile
because `.todos()` requires `impl Assign<List<Todo>>` and plain values don't
implement `Assign` — callers must use an explicit combinator.

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

#### Replace

Use `stmt::set` with a list expression to replace the entire set:

```rust
user.update()
    .todos(stmt::set([
        Todo::create().title("Only todo"),
    ]))
    .exec(&mut db)
    .await?;
```

`stmt::set` takes an `impl IntoExpr<List<Todo>>` (the new set) and returns an
`Assignment<List<Todo>>`. This disassociates all current todos from the user
(following the same optional/required foreign key rules as remove), then
associates the new set.

Pass an empty slice to clear the set:

```rust
user.update()
    .todos(stmt::set([]))
    .exec(&mut db)
    .await?;
```

#### Multiple operations

Arrays of assignments implement `Assign<T>` when their elements do. This means
`[Q; N]: Assign<T> where Q: Assign<T>`. Combine
inserts, removes, and other operations by passing an array:

```rust
user.update()
    .name("Alice")
    .todos([
        stmt::insert(Todo::create().title("Buy groceries")),
        stmt::insert(Todo::create().title("Walk the dog")),
        stmt::remove(&old_todo),
    ])
    .exec(&mut db)
    .await?;
```

Each element is an `Assignment<List<Todo>>`. The array itself implements
`Assign<List<Todo>>` by applying each assignment in order.

This works from both instance updates and query updates:

```rust
User::filter_by_id(user_id)
    .update()
    .todos([
        stmt::insert(Todo::create().title("New task")),
        stmt::remove(&old_todo),
    ])
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
`impl Assign<Creature>`. Full replacement uses `stmt::set`:

```rust
user.update()
    .critter(stmt::set(Creature::Human { profession: "doctor".into(), age: 30 }))
    .exec(&mut db)
    .await?;
```

For partial updates, `stmt::patch` takes a field path and a value, returning
`Assignment<Creature>` without needing a closure or any generated builder type.
The path reuses the existing `fields()` accessor chain, which already returns
typed `Path<T, U>` values:

```rust
fn patch<T, U>(path: Path<T, U>, value: impl Assign<U>) -> Assignment<T>
```

`Path<T, U>` carries both the origin type `T` (the embedded type being patched)
and the leaf type `U` (the field being set). The origin type is what makes
`patch` return `Assignment<Creature>` — no extra type information needed at the
call site. `Path<T, U>` is already implemented in `toasty::stmt::Path`.

The value accepts `impl Assign<U>`, so plain values must be wrapped with
`stmt::set`. Nested patches work because `Assignment<U>` implements `Assign<U>`
directly.

```rust
user.update()
    .critter(stmt::patch(
        Creature::fields().human().profession(),
        stmt::set("doctor"),
    ))
    .exec(&mut db)
    .await?;
```

This produces `assignments.set([critter, profession], "doctor")` — updating the
profession column without touching the age column.

Multiple sub-field mutations combine via the same array pattern used for
has-many operations:

```rust
user.update()
    .critter([
        stmt::patch(Creature::fields().human().profession(), stmt::set("doctor")),
        stmt::patch(Creature::fields().human().age(), stmt::set(35)),
    ])
    .exec(&mut db)
    .await?;
```

Each `stmt::patch` returns `Assignment<Creature>`, and `[Assignment<T>; N]`
implements `Assign<T>`, so the array is accepted by `.critter()`.

#### Nested patching

Because `Assignment<T>` implements `Assign<T>`, `stmt::patch` composes with
itself. When an embedded type contains another embedded type, the inner patch
becomes the value argument to the outer patch:

```rust
user.update()
    .kind(
        stmt::patch(
            Kind::variants().admin().perm(),
            stmt::patch(
                Permission::fields().everything(),
                stmt::set(true),
            ),
        ),
    )
    .exec(&mut db)
    .await?;
```

Here `stmt::patch(Permission::fields().everything(), stmt::set(true))` returns
`Assignment<Permission>`, which implements `Assign<Permission>`. The outer
`stmt::patch` accepts it as the value for the `perm` path, returning
`Assignment<Kind>`. The nesting works to arbitrary depth — each layer resolves
one level of the field path.

This approach avoids generating update builder types for embedded types
entirely — the `fields()` path infrastructure already exists and provides full
type safety.

The `.with_critter()` closure method remains for now but can be removed once
`stmt::patch` is implemented.

### Setter bounds by field type

| Field type | Setter bound | Plain value | `stmt::` combinator | Array |
|---|---|---|---|---|
| Scalar (`String`) | `impl IntoExpr<T>` | Set the field | — | — |
| BelongsTo (`User`) | `impl IntoExpr<T>` | Set the association | — | — |
| HasMany (`List<Todo>`) | `impl Assign<List<T>>` | — | `stmt::insert`, `stmt::remove`, `stmt::set` | Multiple operations |
| Embedded (`Creature`) | `impl Assign<T>` | — | `stmt::set` (replace), `stmt::patch` (sub-field) | Multiple patches |

Scalar and relation setters use `impl IntoExpr<T>`. Has-many and embedded
setters use `impl Assign<T>`. The split exists because Rust's coherence rules
prevent a blanket `IntoExpr<T> → Assign<T>` impl. All paths write into the same
`Assignments` map. The `.with_` methods for embedded types remain for now
alongside `stmt::patch`.

### Summary

| Today | With `Assign` |
|---|---|
| `.name("Alice")` | `.name("Alice")` (unchanged) |
| `.todo(expr)` | `.todos(stmt::insert(expr))` |
| `.todo(a).todo(b)` | `.todos([stmt::insert(a), stmt::insert(b)])` |
| _not possible_ | `.todos(stmt::remove(&todo))` |
| _not possible_ | `.todos(stmt::set([...]))` (replace) |
| _not possible_ | `.todos([stmt::insert(a), stmt::remove(b)])` |
| `.critter(value)` | `.critter(stmt::set(value))` |
| `.with_critter(\|c\| c.profession("x"))` | `.critter(stmt::patch(path, stmt::set("x")))` |
| _not possible_ | `.kind(stmt::patch(path, stmt::patch(inner_path, stmt::set(val))))` |
