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

### Existence checks (`EXISTS`)

"Find users who have at least one incomplete todo."

Today this is written as:

```rust
User::filter(
    User::fields().todos().any(Todo::fields().complete().eq(false))
)
```

In `query!`, this becomes:

```rust
query!(User FILTER EXISTS(.todos FILTER .complete == false))
```

The body of `EXISTS` is a sub-query. The first part — `.todos` — is the
"select": it names what is being selected, relative to the outer query. Here,
`.todos` selects the todos association of `User`. The rest of the sub-query
(`FILTER`, `ORDER BY`, etc.) operates relative to that selection, so `.complete`
refers to a field on `Todo`, not on `User`.

This expands to:

```rust
User::filter(
    User::fields().todos().any(
        Todo::fields().complete().eq(false)
    )
)
```

`EXISTS` expands to `.any()` because the select part of the sub-query is only
used to define the scope — we statically know this is an existence check and
can translate it directly to `.any()` at the macro level. There is no need to
pass a full statement through.

#### Multi-hop select paths

The select path inside `EXISTS` is not limited to a single association. It can
chain through multiple associations to reach deeper relations.

"Find all users with at least one todo tagged 'important'."

```rust
query!(User FILTER EXISTS(.todos.tags FILTER .name == "important"))
```

Here `.todos.tags` traverses two associations — from `User` to `Todo` to
`Tag`. The `FILTER` clause operates on `Tag` fields.

#### Nested `EXISTS`

When the sub-query itself needs an existence check, `EXISTS` can nest:

```rust
// Users who have a todo with a tag named "urgent"
query!(User FILTER EXISTS(.todos FILTER EXISTS(.tags FILTER .name == "urgent")))
```

This expands to:

```rust
User::filter(
    User::fields().todos().any(
        Todo::fields().tags().any(
            Tag::fields().name().eq("urgent")
        )
    )
)
```

The multi-hop form (`EXISTS(.todos.tags FILTER ...)`) and the nested form
(`EXISTS(.todos FILTER EXISTS(.tags FILTER ...))`) express the same query. The
multi-hop form is more concise when the intermediate model does not need its own
filter. The nested form is required when it does:

```rust
// Users who have an *incomplete* todo with a tag named "urgent"
query!(User FILTER EXISTS(.todos FILTER .complete == false AND EXISTS(.tags FILTER .name == "urgent")))
```

### Cardinality-one traversal

When an association has a cardinality of one — `BelongsTo` or `HasOne` — the
macro can traverse it with dot-path chaining, just like accessing a field.

"Find todos whose user has name Carl."

```rust
query!(Todo FILTER .user.name == "Carl")
```

This expands to:

```rust
Todo::filter(
    Todo::fields().user().name().eq("Carl")
)
```

This works because `.user` has a cardinality of one: there is exactly one user
per todo, so `.user.name` unambiguously refers to a single value. The same
pattern works for `HasOne` associations:

```rust
// User has_one profile
query!(User FILTER .profile.bio IS SOME)
```

For direct equality against a model instance, no traversal is needed:

```rust
query!(Todo FILTER .user == #carl)
```

Expands to:

```rust
Todo::filter(Todo::fields().user().eq(carl))
```

### Referencing the parent scope

Inside an `EXISTS` sub-query, dot-prefixed paths are relative to the sub-query's
select. Sometimes the filter needs to reference a field from the outer query.
The solution is to use an "absolute" path that names the root model type.

"Find all users that have a todo assigned to themselves."

```rust
query!(User FILTER EXISTS(.todos FILTER .assignee == User.name))
```

Here, `.assignee` is relative to the sub-query scope (`Todo`), but `User.name`
is an absolute path — it references the `name` field on the outer `User` query.
The macro recognizes `User.name` as absolute because `User` matches the root
select's model type.

### Summary of association filter syntax

| Pattern | Syntax | Meaning |
|---|---|---|
| Has-many EXISTS | `EXISTS(.assoc FILTER expr)` | At least one child matches |
| Multi-hop EXISTS | `EXISTS(.a.b FILTER expr)` | Traverse multiple associations |
| Nested EXISTS | `EXISTS(.a FILTER EXISTS(.b FILTER expr))` | Nested existence with intermediate filter |
| Cardinality-one field | `.assoc.field op val` | Filter by belongs-to/has-one field |
| Cardinality-one equality | `.assoc == #val` | Association equals a model instance |
| Parent reference | `Root.field` | Absolute path to outer query field |

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
5. `EXISTS(sub-query)` and parenthesized groups `(expr)` (atoms)

A dot-prefixed path (`.field` or `.field.subfield`) is parsed as a sequence of
`.ident` tokens. An absolute path (`Model.field`) is parsed as an `ident`
followed by `.ident` tokens. On the right side of a comparison, the value is
one of:

- A string literal (`"Carl"`)
- A numeric literal (`18`)
- A boolean literal (`true`, `false`)
- An external reference (`#ident` or `#(expr)`)
- A dot-prefixed field path (for field-to-field comparisons)
- An absolute path (`Model.field`, for parent scope references)

### Case-insensitive keywords

Keywords are matched case-insensitively by comparing the identifier's string
representation. `FILTER`, `filter`, `Filter` all match. This is handled
during parsing by lowercasing the identifier text before comparison.

`AND`, `OR`, `NOT`, `EXISTS`, `IS`, `NONE`, `SOME`, `ORDER`, `BY`, `ASC`,
`DESC`, `OFFSET`, `LIMIT` are all case-insensitive.

### Disambiguation

- `.` always starts a relative field path (no valid Rust expression starts with
  `.` in this context).
- `#` always starts an external reference.
- `EXISTS` is a keyword when followed by `(`. If the user has a variable named
  `exists`, they use `#exists`.
- An identifier followed by `.` on the right side of a comparison is an absolute
  path (parent scope reference). This is unambiguous because relative paths
  start with `.`, not an identifier.
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

#### Step 5: Add `EXISTS` parsing and expansion

Parse `EXISTS(.path FILTER expr)` syntax. Expand the select path to a
`fields()` chain and the filter body to an `.any()` call. Handle multi-hop
select paths and nested `EXISTS`.

#### Step 6: Cardinality-one dot-path traversal

Ensure dot-path chaining through `BelongsTo` and `HasOne` associations expands
to the corresponding method chain (e.g., `.user().name().eq(...)`).

#### Step 7: Parent scope references

Parse absolute paths (`Model.field`) in sub-query filters. Expand to the
appropriate field path rooted at the outer query's model.

#### Step 8: Tests

- Filter by has-many child fields (`EXISTS`)
- Multi-hop `EXISTS`
- Nested `EXISTS` with intermediate filters
- Filter by belongs-to/has-one fields (cardinality-one traversal)
- Parent scope references in sub-queries
