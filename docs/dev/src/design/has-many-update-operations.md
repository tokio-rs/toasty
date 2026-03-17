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

### Scalars: no visible change

Passing a plain value to a scalar field still means "set":

```rust
user.update().name("Alice").exec(&mut db).await?;
```

`&str` implements `IntoExpr<String>`, which has a blanket `IntoAssignment`
impl that calls `assignments.set()`. Nothing changes at the call site.

### Has-many: the `todos()` method

The update builder generates a single `.todos()` method (plural, matching the
field name) instead of today's singular `.todo()`:

```rust
// Passing a value implies insert (one todo)
user.update()
    .todos(Todo::create().title("Buy groceries"))
    .exec(&mut db)
    .await?;
```

For multiple operations, pass a closure. The closure receives a patch builder
that records the full mutation:

```rust
user.update()
    .name("Alice")
    .todos(|t| {
        t.insert(Todo::create().title("Buy groceries"));
        t.insert(Todo::create().title("Walk the dog"));
        t.remove(&old_todo);
    })
    .exec(&mut db)
    .await?;
```

The closure runs synchronously at build time — it records operations, it doesn't
execute them. All operations run when `.exec()` is called.

This works from both instance updates and query updates:

```rust
User::filter_by_id(user_id)
    .update()
    .todos(|t| {
        t.insert(Todo::create().title("New task"));
        t.remove(&old_todo);
    })
    .exec(&mut db)
    .await?;
```

#### Insert

`insert()` adds a new or existing record to the set. Call it multiple times to
add several:

```rust
user.update()
    .todos(|t| {
        t.insert(Todo::create().title("First"));
        t.insert(Todo::create().title("Second"));
        t.insert(&existing_todo);
    })
    .exec(&mut db)
    .await?;
```

All current todos remain. The new ones are added.

#### Remove

`remove()` detaches a specific record:

```rust
user.update()
    .todos(|t| {
        t.remove(&todo_a);
        t.remove(&todo_b);
    })
    .exec(&mut db)
    .await?;
```

All other todos remain untouched.

What "remove" means depends on the belongs-to side of the relationship:

- **Optional foreign key** (`user_id: Option<Id>`): The todo's `user_id` is set
  to `NULL`. The todo continues to exist.
- **Required foreign key** (`user_id: Id`): The todo is deleted. It can't exist
  without a user.

#### Replace

`set()` replaces the entire set. It disassociates all current members, then
associates the given records:

```rust
user.update()
    .todos(|t| {
        t.set([Todo::create().title("Only todo")]);
    })
    .exec(&mut db)
    .await?;
```

Pass an empty slice to clear the set:

```rust
user.update()
    .todos(|t| {
        t.set([]);
    })
    .exec(&mut db)
    .await?;
```

`set()` consumes the patch builder. It cannot be combined with `insert()` or
`remove()` — these are conflicting intents and produce a compile-time error.

#### How it works

The `.todos()` method accepts `impl IntoAssignment<HasMany<Todo>>`. Multiple
types implement this trait:

```rust
// A closure that configures the patch builder
impl<F: FnOnce(&mut TodosPatch)> IntoAssignment<HasMany<Todo>> for F { ... }

// A single create builder — shorthand for insert
impl IntoAssignment<HasMany<Todo>> for TodoCreateBuilder { ... }

// A reference to an existing record — shorthand for insert
impl IntoAssignment<HasMany<Todo>> for &Todo { ... }
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

| Field type | Plain value | Closure |
|---|---|---|
| Scalar (`String`) | Set the field | — |
| Embedded (`Creature`) | Replace the whole value | Patch specific sub-fields |
| BelongsTo (`User`) | Set the association | — |
| HasMany (`HasMany<Todo>`) | Insert one record | Insert, remove, replace via patch builder |

Every setter method has the same signature. The argument type determines the
behavior. No more `.todo()` vs `.remove_todo()` vs `.set_todos()` vs
`.with_critter()` — each field gets one method named after the field.

### Summary

| Today | With `IntoAssignment` |
|---|---|
| `.name("Alice")` | `.name("Alice")` (unchanged) |
| `.todo(expr)` | `.todos(expr)` |
| `.todo(a).todo(b)` | `.todos(\|t\| { t.insert(a); t.insert(b); })` |
| _not possible_ | `.todos(\|t\| t.remove(expr))` |
| _not possible_ | `.todos(\|t\| t.set(exprs))` |
| _not possible_ | `.todos(\|t\| { t.insert(a); t.remove(b); })` |
| `.critter(value)` | `.critter(value)` (unchanged) |
| `.with_critter(\|c\| c.profession("x"))` | `.critter(\|c\| c.profession("x"))` |
