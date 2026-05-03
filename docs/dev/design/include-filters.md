# Filtering included relations

## Summary

Extend relation paths with a `.filter(...)` combinator so `include(...)`
can load a subset of a relation's records — at the top level, at a
nested level, or at multiple levels in the same chain. A user who today
writes `.include(User::fields().todos())` can write
`.include(User::fields().todos().filter(Todo::fields().completed().eq(false)))`
to preload only the unfinished todos for each user. Works the same way
for `HasOne` / `BelongsTo` relations.

## Motivation

`include(...)` currently has no way to restrict which related records
get preloaded. Users have to choose between:

- Loading every related row and filtering in memory — wasteful, and for
  large relations effectively impossible.
- Issuing a separate query for the relation — loses the batching the
  engine already does for `include`, and forces the user to stitch
  results back to parents by hand.

The parent-side combinators `.any(...)` and `.all(...)` already accept a
predicate over the relation's fields, so the building blocks exist; they
just filter *which parents come back*, not *which children load*. The
two are complementary — see "Behavior" — and users routinely want both.

Nested includes are common (preload a user's posts, and for each post
its comments). Today those work via plain path chains; the same
chains should accept filters at any level.

## User-facing API

Every relation step accepts `.filter(predicate)`. The predicate is an
`Expr<bool>` written in terms of that relation target's own fields, the
same scope `.any(...)` and `.all(...)` already use.

### Filtering a `HasMany` include

```rust
// Load each user with only their incomplete todos preloaded.
let users: Vec<User> = User::all()
    .include(
        User::fields()
            .todos()
            .filter(Todo::fields().completed().eq(false)),
    )
    .exec(&mut db)
    .await?;

for user in &users {
    // `user.todos.get()` contains only incomplete todos.
    for todo in user.todos.get() {
        assert!(!todo.completed);
    }
}
```

A user with no matching todos still comes back — their `todos` is
loaded as an empty `Vec`, distinct from "not loaded".

### Filtering a `HasOne` / `BelongsTo` include

```rust
// Preload the profile only if it is public; otherwise it loads as None.
let user = User::filter_by_id(id)
    .include(
        User::fields()
            .profile()
            .filter(Profile::fields().public().eq(true)),
    )
    .get(&mut db)
    .await?;

match user.profile.get() {
    Some(profile) => { /* loaded and matches the filter */ }
    None => { /* either no profile exists, or it failed the filter */ }
}
```

The relation is still considered loaded; `.get()` does not panic. From
the parent's perspective a filtered-out 1-1 looks the same as a missing
relation.

### Nested includes with filters

`.filter(...)` works at any level of a chained include path. Each
filter applies to its own step and is evaluated in that step's relation
target.

```rust
// Load each user's published posts, and for each post its approved
// comments — both filtered at the database, in one chain.
let users: Vec<User> = User::all()
    .include(
        User::fields()
            .posts()
            .filter(Post::fields().published().eq(true))
            .comments()
            .filter(Comment::fields().approved().eq(true)),
    )
    .exec(&mut db)
    .await?;
```

The same effect can be achieved with two `.include` calls sharing a
prefix; the engine merges them.

```rust
let users: Vec<User> = User::all()
    .include(User::fields().posts().filter(Post::fields().published().eq(true)))
    .include(
        User::fields()
            .posts()
            .comments()
            .filter(Comment::fields().approved().eq(true)),
    )
    .exec(&mut db)
    .await?;
```

The two forms are interchangeable. Use whichever reads better.
Filters at the same step combine with AND, regardless of which form
introduced them.

### Composing the predicate

The argument to `.filter(...)` is an ordinary `Expr<bool>` — the same
predicate language used by top-level `.filter` and by `.any` /
`.all`. Compose with the standard combinators on `Expr<bool>`:

```rust
let users: Vec<User> = User::all()
    .include(
        User::fields().posts().filter(
            // Published posts that are popular OR pinned, but never drafts.
            Post::fields().published().eq(true)
                .and(
                    Post::fields().views().gt(100)
                        .or(Post::fields().pinned().eq(true)),
                )
                .and(Post::fields().draft().eq(true).not()),
        ),
    )
    .exec(&mut db)
    .await?;
```

Repeated `.filter(...)` calls at the same step combine with AND.
`OR` and `NOT` have no fluent equivalent — build them inside a single
`.filter(...)` with `Expr::or` / `Expr::not`.

### Composing with parent-side filters

`.filter(...)` on an included path is independent of `.any(...)` /
`.all(...)` on the parent query. Users frequently want both:

```rust
// Users who have at least one incomplete todo, with only their
// incomplete todos preloaded.
let users: Vec<User> = User::all()
    .filter(
        User::fields()
            .todos()
            .any(Todo::fields().completed().eq(false)),
    )
    .include(
        User::fields()
            .todos()
            .filter(Todo::fields().completed().eq(false)),
    )
    .exec(&mut db)
    .await?;
```

The parent filter decides which users come back; the include filter
decides which todos travel with each user. Toasty does not deduplicate
the predicate — if the same condition appears in both places, write it
in both places.

Existing `.include(User::fields().todos())` calls keep working
unchanged — a bare path is an unfiltered include.

## Behavior

- **Happy path.** For `HasMany`, the preloaded `Vec` contains exactly
  the related rows matching the predicate at that step, in whatever
  order the engine already produces for an unfiltered include. For
  `HasOne` / `BelongsTo`, the relation loads as `Some(record)` if the
  (single) related row matches, otherwise `None`.
- **Empty matches.** A `HasMany` parent with no matching children is
  still returned with an empty preloaded `Vec`. An include filter
  never removes parents.
- **Nested filters.** Each filter is evaluated in its own step's
  scope. A filter at depth 2 (e.g. on `comments` under `posts`) only
  excludes comment rows; posts that match the depth-1 filter still
  load with whatever subset of comments matches the depth-2 filter,
  including possibly none.
- **Filters merge across calls.** Multiple `.include(...)` calls with
  the same path step contribute filters to that step. They combine
  with `AND`. This applies whether the duplicate steps come from
  separate calls or the same chained expression — the engine cannot
  tell them apart.
- **Predicate language.** `.filter(...)` accepts any `Expr<bool>`,
  composed with the same combinators (`.and`, `.or`, `.not`,
  `.any`, `.all`, comparisons, `in_set`, …) as a top-level
  `.filter(...)`. There is nothing the predicate language at the
  top level can express that an include filter cannot.
- **Errors.** A predicate that references fields outside the relation
  step's model is a compile error (the typed path machinery already
  enforces this for `.any` / `.all`). Runtime errors from the driver
  propagate as `toasty::Error` exactly as for unfiltered includes.
- **Interaction with transactions.** None. Filtered includes use the
  same statements as unfiltered ones with extra `WHERE` predicates.

## Alternatives considered

- **Closure-based include builder** —
  `.include(|u| u.todos(|t| t.filter(...).limit(10)))`. Composes the
  same operations as the chained method-call syntax but doubles the
  macro surface (one generated sub-query builder per relation).
- **Separate `.include_query(path, query)` entry point.** The engine
  still has to inject the parent join, so the user-supplied query is
  effectively just an extra predicate — what `.filter(...)` already
  is, with worse ergonomics.
- **Macro DSL** — `include!(todos where !completed)`. Separate
  parser, hides type errors, diverges from the rest of the query API.

## Out of scope

- **`.limit` / `.order_by` on includes** — separate design.
- **Cross-scope predicates.** A filter like
  `Todo::fields().user_id().eq(User::fields().id())` (referencing
  parent fields) is not supported. `.any` / `.all` do not support it
  either; the typed path machinery rejects it at compile time.
- **Aggregations over filtered relations** (`count`, `sum`, …) —
  separate feature.
