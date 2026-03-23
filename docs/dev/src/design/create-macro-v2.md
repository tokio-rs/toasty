# `create!` Macro v2

Redesign of the `create!` macro syntax to support mixed-type batch creation,
better disambiguation between type targets and scope targets, and compile-time
required field verification.

## Syntax

### Single creation (struct-literal form)

```rust
toasty::create!(User { name: "Carl", email: "carl@example.com" })
```

No comma between the type path and `{`. This is visually identical to Rust's
struct literal syntax, making it immediately recognizable.

### Scoped creation (`in` keyword)

```rust
toasty::create!(in user.todos() { title: "buy milk" })
```

The `in` keyword prefixes the scope expression, unambiguously marking it as a
scope target. No comma is needed — `in` is not a valid start of a type path or
expression in this position, so it cleanly disambiguates.

The scope expression after `in` is parsed with `Expr::parse_without_eager_brace`
(from `syn`). This prevents the parser from consuming the `{ fields }` body as
part of the expression — the same technique Rust uses for `for pat in expr {
body }`. A bare `{` can only start an expression as a block or struct literal;
`parse_without_eager_brace` suppresses struct literal parsing, and a block
would require `;` or a trailing expression, so the field body `{ name: "Carl" }`
is never ambiguous with the scope expression.

### Batch creation (same type shorthand)

```rust
toasty::create!(User::[
    { name: "Carl", email: "carl@example.com" },
    { name: "Alice", email: "alice@example.com" },
])
```

`Type::[items]` creates multiple records of the same type. The `::` makes this
syntactically distinct from both the struct-literal form and array indexing.

### Batch creation (mixed types)

```rust
toasty::create!((
    User { name: "Carl", email: "carl@example.com" },
    Article { title: "Hello World", author: &carl },
))
```

A `(items)` tuple where each item is a struct-literal form or a scoped `in`
creation. This leverages the batch infrastructure (`IntoStatement` tuple) to
compose multiple inserts of different types into a single batch operation.
Brackets `[...]` are reserved for same-type batches (which return `Vec`).

Scoped items can be mixed into the batch:

```rust
toasty::create!((
    User { name: "Carl", email: "carl@example.com" },
    in user.friends() { name: "Bob" },
))
```

## Parsing Strategy

The macro input starts with one of four forms, distinguished by the first
tokens:

| First tokens | Form | Target |
|---|---|---|
| `Path {` | Single creation | Type |
| `in` | Scoped creation | Scope |
| `Path :: [` | Same-type batch | Type |
| `(` | Mixed-type tuple | Multiple types |

Parsing steps:

1. If input starts with `(` → mixed-type tuple
2. If input starts with `[` → error (use `(...)` for tuples or `Type::[...]` for same-type batch)
3. If input starts with `in` → scoped creation: call
   `Expr::parse_without_eager_brace` for the scope expression, then parse
   `{ fields }`
4. Otherwise, parse as `syn::Path`:
   - If followed by `{` → single creation (struct-literal form)
   - If followed by `:: [` → same-type batch

Inside a `(` tuple, each item is parsed with the same disambiguation: `in`
prefix → scoped item, `Path {` → type-target item.

## Expansion

### Single creation

```rust
// Input:
toasty::create!(User { name: "Carl", email: "carl@example.com" })

// Expands to:
{
    User::__verify_create().name().email().check();
    User::create().name("Carl").email("carl@example.com")
}
```

Returns a `UserCreate` builder. The caller chains `.exec(&db)` to execute.

### Scoped creation

```rust
// Input:
toasty::create!(in user.todos() { title: "buy milk" })

// Expands to:
user.todos().create().title("buy milk")
```

No verification chain — the scope expression is not a type path, and the
relation context already implies certain fields.

### Same-type batch

```rust
// Input:
toasty::create!(User::[
    { name: "Carl", email: "carl@example.com" },
    { name: "Alice", email: "alice@example.com" },
])

// Expands to:
{
    User::__verify_create().name().email().check();
    User::__verify_create().name().email().check();
    toasty::batch([
        User::create().name("Carl").email("carl@example.com"),
        User::create().name("Alice").email("alice@example.com"),
    ])
}
```

Returns `toasty::batch([...])` with an array of create builders. Since all items
are the same type, this returns a `Vec<Model>`. Each item gets its own
verification chain.

### Mixed-type tuple

```rust
// Input:
toasty::create!((
    User { name: "Carl", email: "carl@example.com" },
    Article { title: "Hello World" },
))

// Expands to:
{
    User::__verify_create().name().email().check();
    Article::__verify_create().title().check();
    toasty::batch((
        User::create().name("Carl").email("carl@example.com"),
        Article::create().title("Hello World"),
    ))
}
```

Returns `toasty::batch(( ... ))` with a tuple of create builders. The result
is a tuple `(User, Article)` matching the input structure:

```rust
let (user, article) = toasty::create!((
    User { name: "Carl", email: "carl@example.com" },
    Article { title: "Hello World" },
))
.exec(&mut db).await?;
```

### Mixed tuple with scoped items

```rust
// Input:
toasty::create!((
    User { name: "Carl", email: "carl@example.com" },
    in carl.todos() { title: "buy milk" },
))

// Expands to:
{
    User::__verify_create().name().email().check();
    toasty::batch((
        User::create().name("Carl").email("carl@example.com"),
        carl.todos().create().title("buy milk"),
    ))
}
```

Scoped items in a tuple do not get verification chains (same as standalone
scoped creation). Type-target items get verification as usual.

Same-type batches expand to `toasty::batch([...])` (array → `Vec`), while
mixed-type tuples expand to `toasty::batch((...))` (tuple → tuple). Both
compose with `toasty::batch()` via `IntoStatement`.

## Compile-Time Required Field Verification

See `create-macro-required-field-verification.md` for the full design. Summary:

- `#[derive(Model)]` generates a hidden `__verify_create()` method on each
  model that returns a ZST verifier with typestate tracking
- Required field methods transition type params from `NotSet` to `Set`
- Optional field methods return `Self` (identity)
- `check()` is only available when all required-field traits are satisfied
- `#[diagnostic::on_unimplemented]` gives per-field error messages
- The `create!` macro emits verification chains before the builder chains
- Verification is only emitted for type-target forms (single, same-type batch,
  mixed-type batch), not scoped creation

## Nested Creation

Nested struct bodies and relation lists work the same as today within each item:

```rust
toasty::create!(User {
    name: "Carl",
    email: "carl@example.com",
    todos: [
        { title: "buy milk" },
        { title: "write code" },
    ],
})
```

The verification chain for nested bodies calls the relation method as a no-op:

```rust
User::__verify_create().name().email().with_todos().check();
```

Nested model verification (e.g., `Todo`'s required fields) is not covered by
the verification chain. The nested model's builder catches missing fields at
the database level.

## Migration from v1

### Breaking changes

| v1 syntax | v2 syntax |
|---|---|
| `create!(User, { name: "Carl" })` | `create!(User { name: "Carl" })` |
| `create!(user.todos(), { ... })` | `create!(in user.todos() { ... })` |
| `create!(User, [{ ... }, { ... }])` | `create!(User::[ { ... }, { ... } ])` |

The v1 type-target forms (`create!(User, { ... })` and `create!(User, [...])`)
are removed. The scope form now uses the `in` keyword prefix instead of a
comma separator.

## Implementation Plan

### Phase 1: Macro v2 syntax

#### Step 1: Update `create!` macro parser

Rewrite `crates/toasty-macros/src/create/parse.rs` to handle the four forms:

1. `(` → mixed-type tuple
2. `in expr { ... }` → scoped creation
3. `Path {` → single creation
4. `Path :: [` → same-type batch

Update `Target` enum and `CreateInput` to represent the new forms.

#### Step 2: Update `create!` macro expansion

Rewrite `crates/toasty-macros/src/create/expand.rs` to generate:

- Builder chains as today
- Tuple output for batch forms

No verification chains yet — those are added in phase 2.

#### Step 3: Update existing tests and examples

All existing `create!` usages need to be updated to the new syntax. This
includes:

- Integration tests in `crates/toasty-driver-integration-suite/src/tests/`
- Examples in `examples/`
- Benchmarks

#### Step 4: Add syntax tests

- Tests for each syntax form (single, scoped, same-type batch, mixed-type batch)
- Type alias tests (`type Foo = User; create!(Foo { ... })`)

### Phase 2: Compile-time required field verification

(From `create-macro-required-field-verification.md`)

#### Step 5: Implement verification codegen

- Add `Set`/`NotSet` markers to `toasty::codegen_support`
- Add `is_required_on_create()` to codegen `Field`
- Generate verifier struct, traits, and `__verify_create()` in
  `expand/create.rs`

#### Step 6: Wire verification into `create!` expansion

Update macro expansion to emit `__verify_create()` chains before builder chains
for type-target forms (single, same-type batch, mixed-type batch). Scoped
creation is unchanged.

#### Step 7: Add verification tests

- Compile-fail tests for missing required fields
- Tests verifying optional fields can be omitted without error
