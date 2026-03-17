# `IntoAssignment<T>`: Unified Update Mutations

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

## What will be built

### `IntoAssignment<T>`

A single trait unifies all update mutations:

```rust
trait IntoAssignment<T> {
    fn into_assignment(self, field: &mut Assignments, projection: Projection);
}
```

Every update builder setter becomes `.field(impl IntoAssignment<T>)`. The
argument decides what kind of mutation to record. For most uses, the call site
looks the same as today because `IntoExpr<T>` gets a blanket impl of
`IntoAssignment<T>` that defaults to the right operation for the field type.

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

`Assignment<T>` implements `IntoAssignment<T>`, so these return values can be
passed directly to any setter that accepts `impl IntoAssignment<T>`.

### Scalars: no visible change

Passing a plain value to a scalar field still means "set":

```rust
user.update().name("Alice").exec(&mut db).await?;
```

`&str` implements `IntoExpr<String>`, which has a blanket `IntoAssignment`
impl that calls `assignments.set()`. Nothing changes at the call site.

You can also be explicit, though there's no reason to:

```rust
user.update().name(stmt::set("Alice")).exec(&mut db).await?;
```

### Has-many

The update builder generates a `.todos()` method (plural, matching the field
name) instead of today's singular `.todo()`. The method accepts
`impl IntoAssignment<List<Todo>>`.

#### Insert

Pass a value directly to insert one todo. A bare `IntoExpr<Todo>` gets a
blanket `IntoAssignment<List<Todo>>` impl that defaults to insert:

```rust
user.update()
    .todos(Todo::create().title("Buy groceries"))
    .exec(&mut db)
    .await?;
```

Or be explicit with `stmt::insert`:

```rust
user.update()
    .todos(stmt::insert(Todo::create().title("Buy groceries")))
    .exec(&mut db)
    .await?;
```

Both produce the same assignment. `stmt::insert` takes an `impl IntoExpr<Todo>`
and returns an `Assignment<List<Todo>>`, which `.todos()` accepts.

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

Here `stmt::set` takes an `impl IntoExpr<List<Todo>>` (the new set) and returns
an `Assignment<List<Todo>>`. This disassociates all current todos from the user
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

Arrays of assignments implement `IntoAssignment<T>` when their elements do.
This means `[Q; N]: IntoAssignment<T> where Q: IntoAssignment<T>`. Combine
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
`IntoAssignment<List<Todo>>` by applying each assignment in order.

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

### Embedded types: partial updates without `.with_`

Today, partially updating an embedded enum requires a separate `.with_` method:

```rust
user.update()
    .with_critter(|c| c.profession("doctor"))
    .exec(&mut db)
    .await?;
```

With `IntoAssignment`, the regular setter handles both full replacement and
partial updates. Passing a value replaces the whole field. Passing a closure
patches it in place:

```rust
// Replace the entire enum value
user.update()
    .critter(Creature::Human { profession: "doctor".into(), age: 30 })
    .exec(&mut db)
    .await?;

// Partial update: change just one variant field
user.update()
    .critter(|c| {
        c.profession("doctor");
    })
    .exec(&mut db)
    .await?;
```

Both go through `.critter(impl IntoAssignment<Creature>)`. The plain value
impl calls `assignments.set()`. The closure impl writes individual sub-path
assignments, producing something like
`assignments.set([critter, profession], "doctor")` — updating the profession
column without touching the age column.

This replaces the `.with_critter()` method entirely.

### What `IntoAssignment` unifies

The same `.field(impl IntoAssignment<T>)` pattern covers every field type:

| Field type | Plain value | `stmt::` combinator | Array |
|---|---|---|---|
| Scalar (`String`) | Set the field | `stmt::set` (explicit) | — |
| Embedded (`Creature`) | Replace the whole value | — | Patch specific sub-fields via closure |
| BelongsTo (`User`) | Set the association | — | — |
| HasMany (`List<Todo>`) | Insert one record | `stmt::insert`, `stmt::remove`, `stmt::set` | Multiple operations |

Every setter method has the same signature. The argument type determines the
behavior. No more `.todo()` vs `.remove_todo()` vs `.set_todos()` vs
`.with_critter()` — each field gets one method named after the field.

### Summary

| Today | With `IntoAssignment` |
|---|---|
| `.name("Alice")` | `.name("Alice")` (unchanged) |
| `.todo(expr)` | `.todos(expr)` or `.todos(stmt::insert(expr))` |
| `.todo(a).todo(b)` | `.todos([stmt::insert(a), stmt::insert(b)])` |
| _not possible_ | `.todos(stmt::remove(&todo))` |
| _not possible_ | `.todos(stmt::set([...]))` |
| _not possible_ | `.todos([stmt::insert(a), stmt::remove(b)])` |
| `.critter(value)` | `.critter(value)` (unchanged) |
| `.with_critter(\|c\| c.profession("x"))` | `.critter(\|c\| c.profession("x"))` |
