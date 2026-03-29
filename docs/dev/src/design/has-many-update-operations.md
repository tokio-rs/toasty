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

A single trait unifies all update mutations:

```rust
trait Assign<T> {
    fn assign(self, assignments: &mut stmt::Assignments, projection: stmt::Projection);
}
```

Every update builder setter becomes `.field(impl Assign<T>)`. The argument
decides what kind of mutation to record. `IntoExpr<T>` has a blanket impl of
`Assign<T>` that defaults to set, so plain values work everywhere — scalars,
collections, embedded types. For collection fields, a plain value of the
collection type (e.g., `impl IntoExpr<List<Todo>>`) means "replace the entire
set". A single item (`impl IntoExpr<Todo>`) won't compile for a `List<Todo>`
field because `Assign<Todo>` is not `Assign<List<Todo>>` — callers must use an
explicit combinator (`stmt::insert`, `stmt::remove`) to specify intent.

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

`&str` implements `IntoExpr<String>`, which has a blanket `Assign` impl that
calls `assignments.set()`. Nothing changes at the call site.

You can also be explicit, though there's no reason to:

```rust
user.update().name(stmt::set("Alice")).exec(&mut db).await?;
```

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
`Assignment<List<Todo>>`, which `.todos()` accepts. The combinator is required
for single items — passing a bare `Todo::create()` directly to `.todos()` won't
compile because `Assign<Todo>` is not `Assign<List<Todo>>`. A bare *list*
expression would compile (it means "replace" via the blanket impl), but a single
item is ambiguous so the caller must be explicit.

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

A plain list expression replaces the entire set:

```rust
user.update()
    .todos([
        Todo::create().title("Only todo"),
    ])
    .exec(&mut db)
    .await?;
```

The array implements `IntoExpr<List<Todo>>`, which satisfies `Assign<List<Todo>>`
via the blanket impl, defaulting to set. This disassociates all current todos
from the user (following the same optional/required foreign key rules as remove),
then associates the new set.

`stmt::set` can also be used explicitly, though there's no reason to:

```rust
user.update()
    .todos(stmt::set([
        Todo::create().title("Only todo"),
    ]))
    .exec(&mut db)
    .await?;
```

Pass an empty slice to clear the set:

```rust
user.update()
    .todos([])
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
`impl Assign<Creature>`, which covers full replacement via a plain value:

```rust
user.update()
    .critter(Creature::Human { profession: "doctor".into(), age: 30 })
    .exec(&mut db)
    .await?;
```

For partial updates, `stmt::patch` will take a field path and a value, returning
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

The value accepts `impl Assign<U>`. Plain values work via the blanket
`IntoExpr<U> -> Assign<U>` impl. Nested patches work because `Assignment<U>`
implements `Assign<U>` directly — no need for `Assignment` to implement
`IntoExpr`.

```rust
user.update()
    .critter(stmt::patch(Creature::fields().human().profession(), "doctor"))
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
        stmt::patch(Creature::fields().human().profession(), "doctor"),
        stmt::patch(Creature::fields().human().age(), 35),
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
                true,
            ),
        ),
    )
    .exec(&mut db)
    .await?;
```

Here `stmt::patch(Permission::fields().everything(), true)` returns
`Assignment<Permission>`, which implements `Assign<Permission>`. The outer
`stmt::patch` accepts it as the value for the `perm` path, returning
`Assignment<Kind>`. The nesting works to arbitrary depth — each layer resolves
one level of the field path.

This approach avoids generating update builder types for embedded types
entirely — the `fields()` path infrastructure already exists and provides full
type safety.

The `.with_critter()` closure method remains for now but can be removed once
`stmt::patch` is implemented.

### What `Assign` unifies

The same `.field(impl Assign<T>)` pattern covers every field type:

| Field type | Plain value | `stmt::` combinator | Array |
|---|---|---|---|
| Scalar (`String`) | Set the field | `stmt::set` (explicit) | — |
| Embedded (`Creature`) | Replace the whole value | `stmt::patch` (sub-field) | Multiple patches |
| BelongsTo (`User`) | Set the association | — | — |
| HasMany (`List<Todo>`) | Replace the set | `stmt::insert`, `stmt::remove` | Multiple operations |

Every setter method has the same signature. The argument type determines the
behavior. No more `.todo()` vs `.remove_todo()` vs `.set_todos()` — each field
gets one method named after the field. The `.with_` methods for embedded types
remain for now alongside `stmt::patch`.

### Summary

| Today | With `Assign` |
|---|---|
| `.name("Alice")` | `.name("Alice")` (unchanged) |
| `.todo(expr)` | `.todos(stmt::insert(expr))` |
| `.todo(a).todo(b)` | `.todos([stmt::insert(a), stmt::insert(b)])` |
| _not possible_ | `.todos(stmt::remove(&todo))` |
| _not possible_ | `.todos([...])` (replace) |
| _not possible_ | `.todos([stmt::insert(a), stmt::remove(b)])` |
| `.critter(value)` | `.critter(value)` (unchanged) |
| `.with_critter(\|c\| c.profession("x"))` | `.critter(stmt::patch(Creature::fields().human().profession(), "x"))` |
| _not possible_ | `.kind(stmt::patch(path, stmt::patch(inner_path, val)))` (nested) |
