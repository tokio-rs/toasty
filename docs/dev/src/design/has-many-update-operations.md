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

## Future direction: unified `.todos()` method

The methods above (`.todo()`, `.remove_todo()`, `.set_todos()`) are a stepping
stone. The long-term API consolidates everything into a single `.todos()` method
that takes a mutation description. The singular `.todo()` method gets deprecated.

### The `Patch` builder

A generated `Patch` type describes what to do with the set. The `.todos()`
method accepts a closure that receives a `Patch` builder:

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

The closure runs synchronously at build time — it only records operations, it
doesn't execute them. The update builder collects the patch and sends all
operations to the database when `.exec()` is called.

### Insert

`Patch::insert()` adds a new or existing record to the set. Call it multiple
times to add several:

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

This keeps all current todos and adds the new ones.

### Remove

`Patch::remove()` detaches a specific record from the set:

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

### Combined insert and remove

A single patch can mix inserts and removes:

```rust
user.update()
    .todos(|t| {
        t.insert(Todo::create().title("New task"));
        t.remove(&old_todo);
    })
    .exec(&mut db)
    .await?;
```

### Replace

`Patch::set()` replaces the entire set. It disassociates all current members,
then associates the given records:

```rust
user.update()
    .todos(|t| {
        t.set([
            Todo::create().title("Only todo"),
        ]);
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

`set()` consumes the patch builder. It cannot be followed by `insert()` or
`remove()` calls — these are conflicting intents and produce a compile-time
error.

### Shorthand for common cases

When the patch is a single operation, passing just the value implies insert —
preserving the feel of today's `.todo()` method:

```rust
// These are equivalent:
user.update().todos(Todo::create().title("New")).exec(&mut db).await?;
user.update().todos(|t| t.insert(Todo::create().title("New"))).exec(&mut db).await?;
```

This works because `.todos()` accepts `impl IntoTodosPatch`, and both closures
and single create expressions implement this trait. The generated trait looks
like:

```rust
// Generated per has-many relation
trait IntoTodosPatch {
    fn into_patch(self, patch: &mut TodosPatch);
}

// A closure that configures the patch
impl<F: FnOnce(&mut TodosPatch)> IntoTodosPatch for F {
    fn into_patch(self, patch: &mut TodosPatch) {
        self(patch);
    }
}

// A single create builder implies insert
impl IntoTodosPatch for TodoCreateBuilder {
    fn into_patch(self, patch: &mut TodosPatch) {
        patch.insert(self);
    }
}

// A reference to an existing record implies insert
impl IntoTodosPatch for &Todo {
    fn into_patch(self, patch: &mut TodosPatch) {
        patch.insert(self);
    }
}
```

### Migration path

1. Ship the `.todos(|t| ...)` method alongside the existing `.todo()` method.
2. Deprecate `.todo()`, `.remove_todo()`, and `.set_todos()`.
3. Remove the deprecated methods in a future release.

The deprecation warnings guide users to the new form:

```
warning: `todo()` is deprecated, use `todos(|t| t.insert(...))` or `todos(...)` instead
```

### Summary

| Old (deprecated) | New |
|---|---|
| `.todo(expr)` | `.todos(expr)` or `.todos(\|t\| t.insert(expr))` |
| `.todo(a).todo(b)` | `.todos(\|t\| { t.insert(a); t.insert(b); })` |
| `.remove_todo(expr)` | `.todos(\|t\| t.remove(expr))` |
| `.set_todos(exprs)` | `.todos(\|t\| t.set(exprs))` |
| `.todo(a).remove_todo(b)` | `.todos(\|t\| { t.insert(a); t.remove(b); })` |
