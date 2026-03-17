# Has-Many Set Operations on Update Builders

## What exists today

The update builder for a model with a `HasMany` relation exposes one operation:
**insert**. You can add new or existing associated records while updating a
model instance or query:

```rust
// Instance update: create a new todo while renaming the user
user.update()
    .name("Bob")
    .todo(Todo::create().title("new todo"))
    .exec(&mut db)
    .await?;

// Query update: same thing, without loading the user first
User::filter_by_id(user_id)
    .update()
    .todo(Todo::create().title("new todo"))
    .exec(&mut db)
    .await?;

// Reassign an existing todo to this user
user.update()
    .todo(&existing_todo)
    .exec(&mut db)
    .await?;
```

Separately, the relation accessor (`user.todos()`) supports insert and remove,
but these are standalone operations — they can't be combined with field updates
in a single call:

```rust
user.todos().insert(&mut db, &todo).await?;
user.todos().remove(&mut db, &todo).await?;
```

## What's missing

Three operations are missing from the update builder:

1. **Remove** — Remove specific todos from the user during an update.
2. **Replace** — Replace the entire set of todos with a new set.
3. **Multiple inserts** — Add more than one todo in a single update call.

Today, trying to chain two `.todo()` calls panics because the assignment map
doesn't support multiple entries for the same field. And there is no way to
express remove or replace through the update builder at all.

## What will be built

### Inserting multiple todos

Chain `.todo()` calls to add several todos in one update:

```rust
user.update()
    .name("Alice")
    .todo(Todo::create().title("Buy groceries"))
    .todo(Todo::create().title("Walk the dog"))
    .todo(&existing_todo)
    .exec(&mut db)
    .await?;
```

Each `.todo()` call appends to the insert list. The update creates "Buy
groceries" and "Walk the dog" as new records, and reassigns `existing_todo` to
this user — all in one operation alongside the name change.

This also works from a query:

```rust
User::filter_by_id(user_id)
    .update()
    .todo(Todo::create().title("First"))
    .todo(Todo::create().title("Second"))
    .exec(&mut db)
    .await?;
```

### Removing todos

Call `.remove_todo()` to detach specific todos during an update:

```rust
user.update()
    .name("Alice")
    .remove_todo(&todo_to_remove)
    .exec(&mut db)
    .await?;
```

What "remove" means depends on the belongs-to side of the relationship:

- **Optional foreign key** (`user_id: Option<Id>`): The todo's `user_id` is set
  to `NULL`. The todo continues to exist.
- **Required foreign key** (`user_id: Id`): The todo is deleted. It can't exist
  without a user.

You can remove multiple todos:

```rust
user.update()
    .remove_todo(&todo_a)
    .remove_todo(&todo_b)
    .exec(&mut db)
    .await?;
```

### Combining insert and remove

Insert and remove can appear in the same update. This adds some todos and
removes others in one call:

```rust
user.update()
    .todo(Todo::create().title("New task"))
    .remove_todo(&old_todo)
    .exec(&mut db)
    .await?;
```

The operations are independent — removes don't affect the inserted records and
inserts don't affect the removed records.

### Replacing the entire set

Call `.set_todos()` to replace all current todos with a new set:

```rust
user.update()
    .set_todos([
        Todo::create().title("Only todo"),
    ])
    .exec(&mut db)
    .await?;
```

This disassociates all existing todos from the user (following the same
optional/required foreign key rules as remove), then associates the new set.
The user ends up with exactly the todos listed in the call — no more, no less.

Pass an empty slice to remove all todos:

```rust
user.update()
    .set_todos([])
    .exec(&mut db)
    .await?;
```

Replace also accepts existing records:

```rust
user.update()
    .set_todos([&todo_a, &todo_b])
    .exec(&mut db)
    .await?;
```

`.set_todos()` cannot be combined with `.todo()` or `.remove_todo()` on the
same update — these are conflicting operations. Attempting to mix them produces
a compile-time error.

### Summary of generated methods

For a `User` model with `#[has_many] todos: HasMany<Todo>`, the update builder
gains these methods:

| Method | What it does |
|---|---|
| `.todo(expr)` | Add a todo (create or reassign). Repeatable. |
| `.remove_todo(expr)` | Remove a specific todo. Repeatable. |
| `.set_todos(exprs)` | Replace all todos with the given set. |

The singular form (`.todo()`, `.remove_todo()`) comes from the relation's
configured singular name. The plural form (`.set_todos()`) uses the field name.

## Future direction: `IntoAssignment<T>` and unified setter methods

The methods above (`.todo()`, `.remove_todo()`, `.set_todos()`) are a stepping
stone toward a unified update API. The long-term design replaces all
per-operation methods with a single setter per field, where the argument itself
carries the mutation semantics.

### The problem

Today, update builder setters use three different patterns depending on field
type:

```rust
// Scalar: .field(impl IntoExpr<T>) → calls assignments.set()
user.update().name("Alice")

// HasMany: .todo(impl IntoExpr<T>) → calls assignments.insert()
user.update().todo(Todo::create().title("Buy groceries"))

// Embedded partial: .with_field(|builder| ...) → closure modifies sub-paths
user.update().with_critter(|c| c.profession("doctor"))
```

Each field type has its own method naming convention, its own argument type, and
its own implied behavior. HasMany needs additional methods (`.remove_todo()`,
`.set_todos()`) that don't exist for other field types. Embedded fields need a
separate `.with_` method that takes a closure. There's no consistent pattern.

### `IntoAssignment<T>`

A single trait replaces all of these:

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
field name) instead of `.todo()` / `.remove_todo()` / `.set_todos()`:

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
| HasMany (`Vec<Todo>`) | Insert one record | Insert, remove, replace via patch builder |

Every setter method has the same signature. The argument type determines the
behavior. No more `.todo()` vs `.remove_todo()` vs `.set_todos()` vs
`.with_critter()` — each field gets one method named after the field.

### Migration path

1. Ship `.todos()` alongside the existing `.todo()` method. Ship
   `IntoAssignment` alongside `IntoExpr` in setters.
2. Deprecate `.todo()`, `.remove_todo()`, `.set_todos()`, and `.with_*()`.
3. Remove the deprecated methods in a future release.

### Summary

| Today | With `IntoAssignment` |
|---|---|
| `.name("Alice")` | `.name("Alice")` (unchanged) |
| `.todo(expr)` | `.todos(expr)` |
| `.todo(a).todo(b)` | `.todos(\|t\| { t.insert(a); t.insert(b); })` |
| `.remove_todo(expr)` | `.todos(\|t\| t.remove(expr))` |
| `.set_todos(exprs)` | `.todos(\|t\| t.set(exprs))` |
| `.critter(value)` | `.critter(value)` (unchanged) |
| `.with_critter(\|c\| c.profession("x"))` | `.critter(\|c\| c.profession("x"))` |
