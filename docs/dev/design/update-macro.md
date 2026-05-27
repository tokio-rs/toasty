# `update!` Macro

`update!` expands struct-literal syntax into an update-builder method
chain. Call `.exec(&mut db).await?` on the result to execute the
update.

```rust
update!(user { name: "Alice Smith" }).exec(&mut db).await?;
```

The macro accepts any expression with an `.update()` method —
instances, query builders, and scoped relation accessors — and
returns the same builder the chain form produces.

[`create!`]: ./static-assertions-create-macro.md

## Motivation

The chain form requires one method call per field. Updates that
change several fields, patch embedded sub-fields, or insert has-many
children grow long:

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

The macro form collapses the same update into struct-literal-shaped
syntax:

```rust
update!(user {
    name: "Alice Smith",
    meta: { version: 2, status: "published" },
    todos: [{ title: "buy milk" }],
}).exec(&mut db).await?;
```

`create!` already uses struct-literal syntax for inserts; `update!`
extends the same shape to updates.

## User-facing API

### Targets

`update!` accepts three kinds of target expression.

**Instance.** The macro applies the update to a loaded model. The
instance reflects the new values after `.exec()`:

```rust
update!(user { name: "Alice Smith" }).exec(&mut db).await?;
```

**Query.** The macro applies the update to every record the query
matches:

```rust
update!(User::filter_by_id(id) { name: "Bob" })
    .exec(&mut db).await?;

update!(User::filter(User::fields().active().eq(false)) {
    archived: true,
}).exec(&mut db).await?;
```

**Scoped relation query.** A relation accessor is a query builder:

```rust
update!(user.todos().filter_by_done(false) { done: true })
    .exec(&mut db).await?;
```

### Field assignments

A field entry takes one of these shapes:

| Shape | Meaning |
|---|---|
| `field: expr` | Set the field to `expr`. |
| `field` | Shorthand for `field: field`. |
| `field.combinator(args)` | Shorthand for `field: toasty::stmt::combinator(args)`. |
| `field: { sub: val, ... }` | Partial update of an embedded field. |
| `field: [{ ... }, ...]` | Insert new children of a has-many relation. |

### Setting fields

```rust
update!(user {
    name: "Alice Smith",
    email: "alice.smith@example.com",
}).exec(&mut db).await?;
```

`expr` is any Rust expression — literals, variables, function calls.
Variable shorthand matches Rust struct literals:

```rust
let name = "Alice Smith";
update!(user { name }).exec(&mut db).await?;
```

### Method shorthand for `stmt::*` combinators

Collection mutations live in `toasty::stmt`. The macro reaches them
as method calls on the field:

```rust
update!(article { tags.push("rust") }).exec(&mut db).await?;
update!(article { tags.extend(["a", "b"]) }).exec(&mut db).await?;
update!(article { tags.pop() }).exec(&mut db).await?;
update!(article { tags.clear() }).exec(&mut db).await?;
```

`field.combinator(args)` lowers to
`field: toasty::stmt::combinator(args)`. Any function in
`toasty::stmt` works. The shorthand is one method call deep — for a
chained expression, use the explicit `field: expr` form.

### Patching embedded fields

A brace block on the right side of `field:` updates the named
sub-fields and leaves the rest unchanged:

```rust
update!(doc {
    meta: { version: 2, status: "published" },
}).exec(&mut db).await?;
```

Brace blocks nest for embedded types within embedded types:

```rust
update!(doc {
    meta: { priority: { level: 3 } },
}).exec(&mut db).await?;
```

To replace an embedded value wholesale, pass the typed value:

```rust
update!(doc {
    meta: Metadata { version: 2, status: "published".into(), priority: Normal },
}).exec(&mut db).await?;
```

### Inserting into has-many relations

A bracket-of-braces literal on a has-many field inserts new children.
Each `{ ... }` is a create builder for the child model:

```rust
update!(user {
    todos: [{ title: "buy milk" }, { title: "walk dog" }],
}).exec(&mut db).await?;
```

Items can mix brace-block builders with `stmt::*` values in the same
update — useful for combining inserts and removals:

```rust
update!(user {
    todos: [{ title: "new" }, stmt::remove(&old)],
}).exec(&mut db).await?;
```

### Setting an optional field to `None`

```rust
update!(user { bio: Option::<String>::None }).exec(&mut db).await?;
```

## Behavior

### Return type

`update!(target { ... })` returns the same builder
`target.update()` returns. For an instance target, `.exec()`
resolves to `()` and reloads the instance. For a query target,
`.exec()` resolves to `()` without reload.

### Field validation

The macro emits one method call per named field on the update
builder. A field the model does not expose for update fails with the
compiler's standard "no method named …" error at the macro call
site.

### Concurrency control

Instance updates carry the version-guard behavior described in
[Concurrency Control][cc]. `update!(user { ... })` calls
`user.update()`, which sets the version assignment and condition
when `User` carries a `#[version]` field. Query-based updates do not
add a concurrency guard.

[cc]: ../../guide/src/concurrency-control.md

### `#[update(expr)]` defaults

Fields annotated with `#[update(expr)]` continue to apply on every
update through both the macro and the chain form.

## Edge cases

- **Empty brace block** (`update!(target { })`). Permitted by the
  parser; the engine rejects empty assignments at runtime.

- **Brace block on a non-embedded field**. Compile error — the
  sub-field accessor only exists on embedded types.

- **Bracket-of-braces on a non-has-many field**. Compile error — the
  per-item create-builder accessor only exists on has-many fields.

- **Chained method calls in the shorthand form**
  (`field.combinator(args).something()`). Parse error. The shorthand
  is one method deep; for a chained expression, use `field: expr`.

- **Unknown combinator in the shorthand form**
  (`field.no_such_op()`). The macro emits
  `field: stmt::no_such_op(...)`; the compiler reports the missing
  function at the macro call site.

- **Target expression containing `{`.** The target is parsed with
  `syn::Expr::parse_without_eager_brace`, the same approach `create!`
  uses for its scoped form. A target that needs a brace (e.g. a
  struct literal) must be parenthesized:
  `update!((MyStruct { ... }.query()) { ... })`.

## Driver integration

Driver authors do nothing. The macro emits the existing update
builder API — every assignment, every `Assign<T>` impl, every
`stmt::*` combinator the macro produces is already part of the
public surface.

The method shorthand emits `stmt::combinator(args)` for any name the
user writes. Missing functions surface as ordinary compile errors.
New `stmt::*` combinators are reachable through the shorthand
without macro changes.

The macro adds no new `Operation` variants or capability flags.

## Alternatives considered

**Embed query syntax in the macro** —
`update!(User FILTER .name == "Alice" { name: "Bob" })`. Mixes two
macro languages and pushes a target-selection decision into a
write-path macro. The current shape composes with [`query!`]
directly: `update!(query!(User FILTER ...) { ... })`. The chain form
`update!(User::filter_by_name("Alice") { ... })` is shorter and
works without `query!`.

[`query!`]: ./query-macro.md

**Set-replace semantics for `field: [{ ... }]` on has-many**.
Dissociating every existing child is destructive and rarely the
intent at a call site listing a few new ones. Insert is the common
case and mirrors `create!`'s reading of `[{ ... }]` as "attach these
new records". Set-replace stays reachable through
`field: stmt::set([...])`, which makes the destructive intent
explicit at the call site.

**`in target { ... }` syntax for symmetry with `create!`'s scoped
form**. `create!` needs `in` to disambiguate from `Type { ... }`,
which parses as a Rust path. Update has no type-only form — the
target is always an expression — so `in` adds nothing.

## Open questions

- **Versioned query-based updates** (deferrable). Query-based
  updates do not apply a version guard. Whether
  `update!(query { ... })` should opt into a guard for models with
  `#[version]` is a broader concurrency-control question.

## Out of scope

- **Compound-assignment operator sugar** (`field += val`,
  `field -= val`, `field <<= val`). The intended mapping is `+=` →
  `stmt::increment`, `-=` → `stmt::decrement`, `<<=` → `stmt::push`
  (for `Vec<scalar>`) or `stmt::insert` (for has-many). `<<=`'s two
  targets exist; the arithmetic side does not — `Assignment` has no
  `Increment` variant, `BinaryOp` has no `Add`/`Sub`, and the SQL
  serializer cannot reference the column being assigned. Adding the
  operator sugar requires landing engine arithmetic, an
  `Assignment::Increment`-shaped variant, SQL + DynamoDB lowering,
  and the `stmt::increment` / `stmt::decrement` surface API first.
  The macro's method shorthand (`views.increment(1)`,
  `tags.push("rust")`) reaches these combinators once they exist.

  When the engine pieces land, adding the operator sugar to the
  macro is a small parse rule that lowers `field += val` to
  `field: stmt::increment(val)`. No further macro infrastructure is
  needed.

- **Batching multiple updates in one macro invocation**. A tuple
  form (`update!(( a { ... }, b { ... }))`) would mirror `create!`'s
  mixed tuple, but error semantics across heterogeneous targets and
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
