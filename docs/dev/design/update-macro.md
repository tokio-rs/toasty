# `update!` Macro

`toasty::update!` is a macro for changing fields on records that
already exist in the database. You hand it a target — either a record
you have already loaded, or a query that selects which records to
change — together with the fields to set. It returns a value with an
`.exec(&mut db).await?` method that runs the update.

```rust
toasty::update!(user { name: "Alice Smith" }).exec(&mut db).await?;
```

The syntax mirrors a Rust struct literal. The target sits where the
struct's type name would go; the field-value pairs go inside the
braces. Only the fields you list change — every other field keeps its
current value.

## Motivation

Toasty also supports updates through a method chain on each record:

```rust
user.update()
    .name("Alice Smith")
    .exec(&mut db).await?;
```

For a one-field change this reads fine. Updates that touch several
fields, patch an [embedded struct][embedded-types], or add child
records on a [has-many][has-many] relation grow long quickly:

```rust
user.update()
    .name("Alice Smith")
    .meta(stmt::apply([
        stmt::patch(Metadata::fields().version(), 2),
        stmt::patch(Metadata::fields().status(), "published"),
    ]))
    .todos(stmt::apply([
        stmt::insert(Todo::create().title("buy milk")),
    ]))
    .exec(&mut db).await?;
```

The macro expresses the same update in a shape that visually matches
the model definition:

```rust
update!(user {
    name: "Alice Smith",
    meta: { version: 2, status: "published" },
    todos: [{ title: "buy milk" }],
}).exec(&mut db).await?;
```

The companion [`create!`][creating-records] macro already uses this
struct-literal shape for inserts. `update!` extends the same shape to
updates.

[embedded-types]: ../../guide/src/embedded-types.md
[has-many]: ../../guide/src/has-many.md
[creating-records]: ../../guide/src/creating-records.md

## What you can update

`update!` accepts three kinds of target.

### A loaded record

You already have an instance returned from a query
([Querying Records][querying]):

```rust
let mut user = User::filter_by_id(id).get(&mut db).await?;
update!(user { name: "Alice Smith" }).exec(&mut db).await?;
```

After `.exec()` returns, the local `user` value reflects the new
field values — the macro re-reads the row so the in-memory copy
matches the database.

[querying]: ../../guide/src/querying-records.md

### A query

You do not need a loaded record. Any query expression works as a
target, and the update applies to every record the query matches:

```rust
update!(User::filter_by_id(id) { name: "Bob" })
    .exec(&mut db).await?;

update!(User::filter(User::fields().active().eq(false)) {
    archived: true,
}).exec(&mut db).await?;
```

The update runs as a single `UPDATE` statement; no records are
loaded first.

### A scoped relation query

Methods that return a child query, such as
`user.todos().filter_by_done(false)`, are query builders too:

```rust
update!(user.todos().filter_by_done(false) { done: true })
    .exec(&mut db).await?;
```

This updates only the matching children belonging to `user`. See
[HasMany][has-many].

## Setting fields

The right side of each field is any Rust expression — a literal, a
variable, a function call:

```rust
update!(user {
    name: "Alice Smith",
    email: "alice.smith@example.com",
}).exec(&mut db).await?;
```

When a local variable already has the field's name, the
struct-literal shorthand works the same way it does in Rust:

```rust
let name = "Alice Smith";
update!(user { name }).exec(&mut db).await?;
```

To set an `Option<T>` field to `None`, give the type so the compiler
can infer the field type:

```rust
update!(user { bio: Option::<String>::None }).exec(&mut db).await?;
```

## Modifying `Vec<scalar>` fields

Vec-of-scalar fields support mutations like push, pop, extend, and
clear. Toasty exposes these as functions under `toasty::stmt`. Inside
`update!`, you can call them as if they were methods on the field:

```rust
update!(article { tags.push("rust") }).exec(&mut db).await?;
update!(article { tags.extend(["a", "b"]) }).exec(&mut db).await?;
update!(article { tags.pop() }).exec(&mut db).await?;
update!(article { tags.clear() }).exec(&mut db).await?;
```

`tags.push("rust")` expands to `tags: toasty::stmt::push("rust")`.
Any function in `toasty::stmt` reachable this way works. The shorthand
is one method call deep — for a chained expression, write out the
explicit `field: expr` form.

See [`Vec<scalar>` Fields][vec-scalar] for the full list of
operations and per-driver support.

[vec-scalar]: ../../guide/src/vec-scalar-fields.md

## Patching embedded sub-fields

An [embedded struct][embedded-types] is a struct stored inline on the
model. To change some of its sub-fields without rewriting the rest,
put a brace block on the right side:

```rust
update!(doc {
    meta: { version: 2, status: "published" },
}).exec(&mut db).await?;
```

Sub-fields you do not name keep their current values. Brace blocks
nest, so you can reach into embedded structs that contain embedded
structs:

```rust
update!(doc {
    meta: { priority: { level: 3 } },
}).exec(&mut db).await?;
```

To replace the embedded value wholesale, pass a typed value instead of
a brace block:

```rust
update!(doc {
    meta: Metadata { version: 2, status: "published".into(), priority: Normal },
}).exec(&mut db).await?;
```

## Inserting has-many children

On a [has-many][has-many] relation, a bracket-of-braces value adds
new child records. Each `{ ... }` inside the brackets is a create
builder for the child model — the same shape `create!` accepts:

```rust
update!(user {
    todos: [{ title: "buy milk" }, { title: "walk dog" }],
}).exec(&mut db).await?;
```

You can mix create-builder entries with explicit `stmt::*` values in
the same list — useful when an update both adds and removes children:

```rust
update!(user {
    todos: [{ title: "new" }, stmt::remove(&old)],
}).exec(&mut db).await?;
```

## Field-assignment cheatsheet

| Shape | Meaning |
|---|---|
| `field: expr` | Set the field to `expr`. |
| `field` | Shorthand for `field: field` (same as Rust struct literals). |
| `field.combinator(args)` | Shorthand for `field: toasty::stmt::combinator(args)`. |
| `field: { sub: val, ... }` | Patch the named sub-fields of an embedded field. |
| `field: [{ ... }, ...]` | Insert new children on a has-many field. |

## Behavior

### What `.exec()` returns

`update!(target { ... })` returns the same builder type that
`target.update()` produces, so the two forms are interchangeable past
the macro call.

- For an instance target, `.exec()` resolves to `()` and reloads the
  instance in place.
- For a query target, `.exec()` resolves to `()` without reloading
  anything.

### Compile-time field checks

The macro generates one method call per field on the underlying
builder. Misspelling a field, or using a field the model does not
expose for update, produces the compiler's standard
"no method named …" error at the macro call site.

### Concurrency control

If the model has a [`#[version]`][concurrency] field, instance
updates condition the write on the version the instance was loaded
with and increment the version atomically. A concurrent writer that
modified the row in between causes `.exec()` to return an error.

Query-based updates (`update!(User::filter(...) { ... })`) skip the
version guard — there is no single loaded version to check against.

[concurrency]: ../../guide/src/concurrency-control.md

### `#[update(expr)]` defaults

Fields annotated with [`#[update(expr)]`][field-options] apply on
every update, whether you reach them through the macro or the chain
form.

[field-options]: ../../guide/src/field-options.md

## Edge cases

- **Empty brace block** (`update!(target { })`). Parses without
  error; the engine rejects it at runtime since there is nothing to
  update.
- **Brace block on a non-embedded field**. Compile error — only
  embedded fields expose sub-field accessors.
- **Bracket-of-braces on a non-has-many field**. Compile error —
  only has-many fields expose a per-item create-builder accessor.
- **Chained method calls in the method shorthand**
  (`field.combinator(args).something()`). Parse error. The shorthand
  is one method deep; use `field: expr` for chained expressions.
- **Unknown name in the method shorthand**
  (`field.no_such_op()`). The macro emits
  `field: stmt::no_such_op(...)`; the compiler reports the missing
  function at the macro call site.
- **Target expression containing `{`**. The macro parses the target
  with `syn::Expr::parse_without_eager_brace` — the same rule that
  lets `if x { ... }` parse correctly. A target that contains an
  inline brace (e.g. a struct literal followed by a method call)
  needs parentheses:
  `update!((MyStruct { ... }.query()) { ... })`.

## Driver integration

Driver authors do not need to do anything for this macro. The macro
emits code against Toasty's existing update-builder API — every
assignment, every `Assign<T>` impl, and every `stmt::*` combinator
the macro produces is already part of the public surface. There are
no new `Operation` variants and no new capability flags. New
`stmt::*` combinators become reachable through the method shorthand
without changes to the macro.

## Alternatives considered

**Embedding query syntax in the macro** — for example
`update!(User FILTER .name == "Alice" { name: "Bob" })`. This mixes
two macro languages and pushes a target-selection decision into a
write-path macro. The current shape composes with [`query!`] directly
(`update!(query!(User FILTER ...) { ... })`), and the chain form
`update!(User::filter_by_name("Alice") { ... })` is shorter still.

**Set-replace semantics for `field: [{ ... }]` on has-many** — i.e.
dissociating every existing child and attaching only the listed ones.
Destructive replacement is rarely the intent at a call site listing a
few records. Insert is the common case and matches `create!`'s
reading of `[{ ... }]` as "attach these new records". Set-replace
stays reachable through `field: stmt::set([...])`, which makes the
destructive intent explicit at the call site.

**`in target { ... }` syntax for symmetry with `create!`'s scoped
form**. `create!` needs `in` to disambiguate from `Type { ... }`,
which Rust parses as a struct-literal expression. `update!` has no
type-only form — the target is always an expression — so `in` adds
nothing.

[`query!`]: ./query-macro.md

## Open questions

- **Versioned query-based updates** (deferrable). Query-based
  updates currently skip the version guard. Whether
  `update!(query { ... })` should opt into a guard for models with
  `#[version]` is a broader concurrency-control question.

## Out of scope

- **Compound-assignment operators** (`field += val`, `field -= val`,
  `field <<= val`). The intended mapping is `+=` →
  `stmt::increment`, `-=` → `stmt::decrement`, `<<=` →
  `stmt::push` (for `Vec<scalar>`) or `stmt::insert` (for has-many).
  The shift side already has both pieces; the arithmetic side does
  not — `Assignment` has no `Increment` variant, `BinaryOp` has no
  `Add`/`Sub`, and the SQL serializer cannot reference the column
  being assigned. Adding the operator sugar needs the engine
  arithmetic, an `Assignment::Increment` variant, SQL + DynamoDB
  lowering, and a public `stmt::increment` / `stmt::decrement` first.
  Once those land, the macro's method shorthand
  (`views.increment(1)`, `tags.push("rust")`) reaches them, and the
  operator sugar becomes a small parse rule that lowers
  `field += val` to `field: stmt::increment(val)`.

- **Batching multiple updates in one invocation**. A tuple form
  (`update!(( a { ... }, b { ... }))`) would mirror `create!`'s
  mixed tuple. Error semantics across heterogeneous targets and the
  interaction with version guards on instance entries need a
  separate design. The chain form
  `toasty::batch((a.update().…, b.update().…))` covers batching in
  the meantime.

- **Upsert semantics**. Tracked separately as a roadmap item; would
  likely be a distinct `upsert!` macro rather than an extension of
  `update!`.

- **Inline construction of `BelongsTo` / `HasOne` related records**
  during an update (`field: { ... }` for a new related instance).
  The underlying API does not provide a single call that creates a
  related record and re-points the FK.

- **Explicit `CONDITION` clause** for arbitrary
  optimistic-concurrency predicates (e.g.
  `update!(target { ... } CONDITION expr)`). The `#[version]`
  mechanism covers the common case; a general condition clause is a
  separate design.
