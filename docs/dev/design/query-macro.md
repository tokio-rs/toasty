# `query!` Macro

`query!` builds queries against a single model: filters over that model's own
fields, ordering, and pagination. It cannot cross an association boundary,
eager-load related models, test an optional field for presence, or use a
boolean field on its own as a predicate. This design covers those additions.

```rust
// Users with at least one incomplete todo
query!(User FILTER EXISTS(.todos FILTER .complete == false))

// All users, eager-loading their todos
query!(User { todos })
```

Tracked in [#808] — discussion of the feature happens there.

[#808]: https://github.com/tokio-rs/toasty/issues/808

## Syntax

### Include blocks

The source is a model type path, optionally followed by an include block:

```rust
// All users, eager-loading their todos
query!(User { todos })

// Nested eager loading
query!(User {
    todos { tags }
})
```

### Null checks

```rust
query!(User FILTER .bio IS NONE)
query!(User FILTER .bio IS SOME)
```

### Boolean fields as predicates

A boolean field path used on its own is a predicate, with or without `NOT`:

```rust
query!(User FILTER .active)
query!(User FILTER NOT .active)
query!(User FILTER .active AND .age > 18)
```

### Multi-key order by

`ORDER BY` takes a comma-separated list of keys, each with its own direction:

```rust
query!(User ORDER BY .last_name ASC, .first_name ASC)
query!(User ORDER BY .active DESC, .name)
```

## Expansion

### Include expansion

```rust
// Input:
query!(User { todos })

// Expands to:
User::all().include(User::fields().todos())

// Input:
query!(User { todos { tags } })

// Expands to:
User::all()
    .include(User::fields().todos().tags())
```

### Filter expansion

| Macro operator | Expansion |
|---|---|
| `.field IS NONE` | `Source::fields().field().is_none()` |
| `.field IS SOME` | `Source::fields().field().is_some()` |
| `.field` (boolean) | `Source::fields().field().eq(true)` |

`NOT` applies to the expanded expression, so `NOT .active` expands to
`User::fields().active().eq(true).not()`.

### Order by expansion

Multiple keys expand to a tuple, which `Query::order_by` accepts:

```rust
// Input:
query!(User ORDER BY .last_name ASC, .first_name DESC)

// Expands to:
User::all().order_by((
    User::fields().last_name().asc(),
    User::fields().first_name().desc(),
))
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

Dot-path chaining covers associations with a cardinality of one — `.user.name`
on a `BelongsTo` names a single value. It does not extend to has-many
associations: `.todos.title` names a list of values, not one value, so it
cannot be compared against a scalar. Filters that cross a has-many go through
`EXISTS`.

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

This form needs a builder API to expand into. `Path::any` takes a predicate
over the child model alone and gives the caller no handle on the outer row. The
statement AST already carries what is needed — `ExprReference` records a nesting
level and resolves against an ancestor model — so the work is a typed surface
over it, not a new engine capability.

### Summary of association filter syntax

| Pattern | Syntax | Meaning |
|---|---|---|
| Has-many EXISTS | `EXISTS(.assoc FILTER expr)` | At least one child matches |
| Multi-hop EXISTS | `EXISTS(.a.b FILTER expr)` | Traverse multiple associations |
| Nested EXISTS | `EXISTS(.a FILTER EXISTS(.b FILTER expr))` | Nested existence with intermediate filter |
| Parent reference | `Root.field` | Absolute path to outer query field |

## Parsing strategy

- The source parser accepts `{ includes }` after the type path, with nested
  blocks for nested includes.
- `ORDER BY` parses a comma-separated list of order-by expressions rather than a
  single one.
- `IS NONE` and `IS SOME` are postfix comparison operators on a field path.
- A field path with no operator after it is an atom, alongside parenthesized
  groups.
- `EXISTS(sub-query)` is an atom. Its body opens a new scope: the select path is
  resolved relative to the enclosing scope, and dot-prefixed paths in the body
  are rooted at the model that path reaches. The macro tracks the current
  model type per scope so it can emit the inner `Todo::fields()` root.
- An absolute path (`Model.field`) is parsed as an `ident` followed by `.ident`
  tokens, and is a valid right-hand side of a comparison.

### Case-insensitive keywords

`EXISTS`, `IS`, `NONE`, and `SOME` match case-insensitively, like the keywords
already recognized.

### Disambiguation

- `EXISTS` is a keyword when followed by `(`. If the user has a variable named
  `exists`, they use `#exists`.
- An identifier followed by `.` on the right side of a comparison is an absolute
  path (parent scope reference). This is unambiguous because relative paths
  start with `.`, not an identifier.
- `{` after the source type starts an include block, not a Rust block
  expression, because the source is always a type path.
