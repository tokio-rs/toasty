# `query!` Macro

A declarative macro for building Toasty queries. `query!` provides a concise,
SQL-inspired syntax for filtering, ordering, paginating, and eager-loading
model data. It builds a query object without executing it — the caller chains
`.exec(&mut db).await?` to run the query.

```rust
let users = query!(User FILTER .name == "Carl").exec(&mut db).await?;
```

Today, the equivalent query is:

```rust
let users = User::filter(User::fields().name().eq("Carl")).exec(&mut db).await?;
```

The macro eliminates the repetition of the model name in field paths, provides
infix operators instead of method chains, and reads closer to a query language.

## Syntax

```
query!($source [FILTER $filter] [ORDER BY $order_by] [OFFSET $offset] [LIMIT $limit])
```

All keywords are case-insensitive: `filter`, `FILTER`, `Filter` all work. The
clauses are optional and can appear in any combination, but must follow the
order shown above when present.

### Source

The source is a model type path, optionally followed by an include block:

```rust
// All users
query!(User)

// All users, eager-loading their todos
query!(User { todos })

// Nested eager loading
query!(User {
    todos { tags }
})
```

### Filter expressions

Filter expressions use infix operators and dot-prefixed field paths:

```rust
// Simple equality
query!(User FILTER .name == "Carl")

// Comparison operators
query!(User FILTER .age > 18)
query!(User FILTER .age >= 18)
query!(User FILTER .age < 65)
query!(User FILTER .age <= 65)
query!(User FILTER .name != "Carl")

// Logical operators
query!(User FILTER .name == "Carl" AND .age > 18)
query!(User FILTER .name == "Carl" OR .name == "Alice")

// Parenthesized grouping
query!(User FILTER .name == "Carl" AND (.age > 18 OR .age < 10))

// Null checks
query!(User FILTER .bio IS NONE)
query!(User FILTER .bio IS SOME)

// Negation
query!(User FILTER NOT .active)
query!(User FILTER NOT (.age > 18 AND .age < 65))
```

### Dot-prefixed field paths

A leading `.` starts a field path rooted at the source model's `fields()`
method. `.name` expands to `User::fields().name()`. Chained dots navigate
associations: `.todos.title` expands to `User::fields().todos().title()`.

### External references

`#ident` pulls a variable from the surrounding scope. `#(expr)` embeds an
arbitrary Rust expression. This follows the `quote!` convention:

```rust
let name = "Carl";
query!(User FILTER .name == #name)

query!(User FILTER .age > #(calculate_min_age()))
```

### Order by

```rust
query!(User ORDER BY .name ASC)
query!(User ORDER BY .created_at DESC)
query!(User FILTER .active == true ORDER BY .name ASC)
```

### Limit and offset

```rust
query!(User LIMIT 10)
query!(User OFFSET 20 LIMIT 10)
query!(User FILTER .active == true ORDER BY .name ASC LIMIT 10)
```

## Expansion

The macro expands each syntactic element into the corresponding method-chain
calls on the existing query builder API.

### Source expansion

```rust
// Input:
query!(User)

// Expands to:
User::all()
```

### Source with includes

```rust
// Input:
query!(User { todos })

// Expands to:
User::all().include(User::fields().todos())

// Nested:
// Input:
query!(User { todos { tags } })

// Expands to:
User::all()
    .include(User::fields().todos().tags())
```

### Filter expansion

Dot-prefixed paths expand to `Source::fields().path()` calls. Operators map to
method calls on the resulting field expression:

| Macro operator | Expansion |
|---|---|
| `.field == val` | `Source::fields().field().eq(val)` |
| `.field != val` | `Source::fields().field().ne(val)` |
| `.field > val` | `Source::fields().field().gt(val)` |
| `.field >= val` | `Source::fields().field().ge(val)` |
| `.field < val` | `Source::fields().field().lt(val)` |
| `.field <= val` | `Source::fields().field().le(val)` |
| `.field IS NONE` | `Source::fields().field().is_none()` |
| `.field IS SOME` | `Source::fields().field().is_some()` |
| `a AND b` | `a.and(b)` |
| `a OR b` | `a.or(b)` |
| `NOT expr` | `!expr` |

```rust
// Input:
query!(User FILTER .name == "Carl" AND .age > 18)

// Expands to:
User::filter(
    User::fields().name().eq("Carl")
        .and(User::fields().age().gt(18))
)
```

### External reference expansion

```rust
// Input:
let name = "Carl";
query!(User FILTER .name == #name)

// Expands to:
User::filter(User::fields().name().eq(name))

// Input:
query!(User FILTER .age > #(calculate_min_age()))

// Expands to:
User::filter(User::fields().age().gt(calculate_min_age()))
```

### Full expansion example

```rust
// Input:
query!(User { todos } FILTER .name == "Carl" ORDER BY .created_at DESC LIMIT 10)

// Expands to:
User::filter(User::fields().name().eq("Carl"))
    .include(User::fields().todos())
    .order_by(User::fields().created_at().desc())
    .limit(10)
```

## Association filters

The macro needs to express filters that cross association boundaries. There are
several patterns to consider.

### Filtering by has-many children (EXISTS / ANY)

"Find users who have at least one incomplete todo."

Today this is written as:

```rust
User::filter(
    User::fields().todos().any(Todo::fields().complete().eq(false))
)
```

The challenge: `Todo::fields()` breaks the convention that all field paths in
`query!` are relative to the source model. The macro user should not need to
name the associated model type explicitly.

#### Proposed syntax: `ANY` with nested dot paths

```rust
query!(User FILTER ANY .todos(.complete == false))
```

`ANY .assoc(filter)` means "at least one related record satisfies the filter."
Inside the parentheses, dot paths are relative to the association target model.
The macro resolves the target model type from the association field.

The macro expands `.todos` into `User::fields().todos()`, which returns a
`ManyField<User>`. The inner filter needs field expressions rooted at the
target model — but proc macros cannot resolve types. The macro only has tokens.

The solution is to add a `with` method to `ManyField` that provides a new path
scope via a closure. `with` accepts a closure whose parameter is the target
model's fields struct; it delegates to `any` internally:

```rust
impl<Origin> ManyField<Origin> {
    pub fn with<F>(self, f: F) -> Expr<bool>
    where
        F: FnOnce(TargetFields) -> Expr<bool>,
    {
        self.any(f(Target::fields()))
    }
}
```

The macro expands `ANY .todos(...)` into a `with` call:

```rust
// Input:
query!(User FILTER ANY .todos(.complete == false))

// Expands to:
User::filter(
    User::fields().todos().with(|__f| __f.complete().eq(false))
)
```

The macro stays at the token level — it does not need to know that `.todos`
points to `Todo`. The Rust compiler infers the closure parameter type from
`ManyField`, and field paths inside the `ANY` parentheses expand relative to
that parameter. No helper functions, no explicit model naming.

#### Nested association filters

```rust
// Users who have a todo with a tag named "urgent"
query!(User FILTER ANY .todos(ANY .tags(.name == "urgent")))
```

Each `ANY` nests one level deeper. The expansion chains `with` calls:

```rust
User::filter(
    User::fields().todos().with(|__f|
        __f.tags().with(|__f| __f.name().eq("urgent"))
    )
)
```

### Filtering by belongs-to parent

"Find todos whose user has name Carl."

Today:

```rust
Todo::filter(Todo::fields().user().eq(&carl))
```

Or using a subquery:

```rust
Todo::filter(
    Todo::fields().user().in_query(
        User::filter(User::fields().name().eq("Carl"))
    )
)
```

#### Proposed syntax: dot-path traversal

For direct equality against a model instance:

```rust
query!(Todo FILTER .user == #carl)
```

This works with no special syntax because `OneField` already implements `.eq()`.
The expansion is:

```rust
Todo::filter(Todo::fields().user().eq(carl))
```

For filtering by a parent's fields, use dot-path chaining:

```rust
query!(Todo FILTER .user.name == "Carl")
```

This is more involved. `.user.name` would need to expand to something like
`Todo::fields().user().name().eq("Carl")`. Today, `OneField` does not expose
the target model's primitive fields directly — it only has `.eq()` and
`.in_query()`.

Two options:

**Option 1: Expand to `in_query` with a subquery.**

```rust
// Input:
query!(Todo FILTER .user.name == "Carl")

// Expands to:
Todo::filter(
    Todo::fields().user().in_query(
        User::filter(User::fields().name().eq("Carl"))
    )
)
```

This requires the macro to split the path at the association boundary, which
means it needs to know that `.user` is an association and `.name` is a field on
the target. A proc macro cannot determine this from tokens alone.

**Option 2: Add field accessors to `OneField` so dot paths work directly.**

If `OneField<__Origin>` exposes `.name()` (returning an `Expr` path that
traverses the association), the macro can expand `.user.name` as a simple
method chain:

```rust
Todo::fields().user().name().eq("Carl")
```

The codegen already generates association field accessors on `ManyField` and
`OneField` (see `expand_field_association_methods` in `relation.rs`). These
currently return nested `ManyField`/`OneField` types for association-to-
association chaining. Extending this to also expose primitive field accessors
on `OneField` would make dot-path traversal work for belongs-to filters.

The query engine's simplification phase would then rewrite `user.name == "Carl"`
into an EXISTS subquery during compilation — the same rewrite it already
performs for association traversals.

**Option 2 is preferred** because it keeps the macro simple (no path splitting)
and pushes the complexity into the query engine where it belongs.

### Filtering by has-one

Has-one associations work the same as belongs-to for filtering purposes:

```rust
// User has_one profile
query!(User FILTER .profile.bio IS SOME)
```

Expands using the same dot-path traversal mechanism as belongs-to.

### Summary of association filter syntax

| Pattern | Syntax | Meaning |
|---|---|---|
| Has-many EXISTS | `ANY .assoc(filter)` | Parent has at least one child matching filter |
| Belongs-to equality | `.assoc == #val` | Association equals a model instance |
| Belongs-to field filter | `.assoc.field op val` | Filter by associated model's field |
| Has-one field filter | `.assoc.field op val` | Same as belongs-to |
| Nested has-many | `ANY .a(ANY .b(filter))` | Nested existence check |

## Parsing strategy

The macro input is parsed left-to-right with keyword-driven sections:

1. Parse the source: a type path, optionally followed by `{ includes }`.
2. If the next token is `FILTER` (case-insensitive): parse a filter expression.
3. If the next token is `ORDER` followed by `BY`: parse order-by expressions.
4. If the next token is `OFFSET`: parse an expression for the offset.
5. If the next token is `LIMIT`: parse an expression for the limit.

### Filter expression parsing

Filter expressions are parsed with standard precedence:

1. `OR` (lowest precedence)
2. `AND`
3. `NOT` (prefix unary)
4. Comparison operators (`==`, `!=`, `>`, `>=`, `<`, `<=`, `IS NONE`, `IS SOME`)
5. `ANY .assoc(filter)` and parenthesized groups `(expr)` (atoms)

A dot-prefixed path (`.field` or `.field.subfield`) is parsed as a sequence of
`.ident` tokens. On the right side of a comparison, the value is one of:

- A string literal (`"Carl"`)
- A numeric literal (`18`)
- A boolean literal (`true`, `false`)
- An external reference (`#ident` or `#(expr)`)
- A dot-prefixed field path (for field-to-field comparisons)

### Case-insensitive keywords

Keywords are matched case-insensitively by comparing the identifier's string
representation. `FILTER`, `filter`, `Filter` all match. This is handled
during parsing by lowercasing the identifier text before comparison.

`AND`, `OR`, `NOT`, `ANY`, `IS`, `NONE`, `SOME`, `ORDER`, `BY`, `ASC`, `DESC`,
`OFFSET`, `LIMIT` are all case-insensitive.

### Disambiguation

- `.` always starts a field path (no valid Rust expression starts with `.` in
  this context).
- `#` always starts an external reference.
- `ANY` is a keyword when followed by `.` (a field path). If the user has a
  variable named `any`, they use `#any`.
- `{` after the source type starts an include block, not a Rust block
  expression, because the source is always a type path.

## Implementation plan

### Phase 1: Basic query macro (no associations in filters)

#### Step 1: Create macro crate structure

Add parsing and expansion modules in `crates/toasty-macros/src/query/`. Define
the `QueryInput` AST types: source, filter expression, order-by, offset, limit.

#### Step 2: Implement parser

Parse source (type path + optional includes), filter expressions (with
precedence climbing), order-by, offset, limit. Handle `#ident` and `#(expr)`
external references.

#### Step 3: Implement expansion

Expand to method chains on the existing query builder API. Generate
`Source::all()`, `.filter()`, `.include()`, `.order_by()`, `.offset()`,
`.limit()` calls.

#### Step 4: Tests

- Compile tests for each syntax form
- Integration tests verifying the expanded queries return correct results

### Phase 2: Association filters

#### Step 5: Add `ANY` parsing and expansion

Parse `ANY .assoc(filter)` syntax. Expand using `ManyField::with()` as
described above. Add the `with` method to the generated `ManyField` structs.

#### Step 6: Add primitive field accessors to `OneField`

Extend codegen to generate primitive field accessors on `OneField`, enabling
dot-path traversal through belongs-to and has-one associations.

#### Step 7: Extend query engine for dot-path traversal

Verify that the simplification phase correctly rewrites field paths that
traverse associations into subqueries. Add support if missing.

#### Step 8: Tests

- Filter by has-many child fields
- Filter by belongs-to parent fields
- Filter by has-one fields
- Nested association filters
