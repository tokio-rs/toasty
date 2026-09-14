# Filtering included relations

## Summary

A filtered include loads a subset of a relation, but the loaded value is
indistinguishable from a full load: `user.todos.get()` hands back
whatever subset the query asked for. Give relation fields a third state,
`Partial`, alongside `Unloaded` and `Loaded`, reachable through
`.get_partial()` and `.is_partial()`, so code that asks for the full set
of `user.todos` cannot be handed a filtered subset without noticing.

Two gaps in the filter surface close alongside it: `.filter(...)` should
be usable at any step of a chained include path rather than only the
last, and filtering the include of a `via` relation should work rather
than panic.

## Motivation

A relation field like `User::todos` denotes a set — *the* todos
belonging to a user — and code reading `user.todos.get()` relies on
that. Two `User` values with the same id can have `todos` loaded with
different subsets depending on which query produced them, and `.get()`
callers cannot tell. A function that takes `&User` and computes
`user.todos.get().len()` reports the size of whatever subset its caller
happened to preload. The invariant is restored by splitting loaded into
two states, so the field's type reflects whether it carries the full set
or a known subset.

Filters are also unavailable in two places users reach for them.
`.filter(...)` yields a terminal include, so a chained path can only
carry a filter at its final step; a nested include that filters at depth
1 has to be split across two `.include(...)` calls. And `via` relations
— including every many-to-many — reject filters outright, so the
most common reason to filter an include (a large join table) is the one
case that does not work.

## User-facing API

### Loaded, unloaded, and partial

An eagerly loaded association sits in one of three states:

- `Unloaded` — the relation was never fetched.
- `Loaded` — the relation was fetched in full. `user.todos` carries
  every todo for that user.
- `Partial` — the relation was fetched with a filter. `user.todos`
  carries a known subset; the remaining todos exist in the database but
  were not returned.

`.get()` returns the records only when the relation is `Loaded`, and
panics for both `Unloaded` and `Partial`. Code that wants to consume a
filtered subset uses `.get_partial()`, which returns the records for
`Loaded` and `Partial` alike and still panics for `Unloaded`.

```rust
let users: Vec<User> = User::all()
    .include(
        User::fields()
            .todos()
            .filter(Todo::fields().completed().eq(false)),
    )
    .exec(&mut db)
    .await?;

for user in &users {
    // The relation is `Partial`, not `Loaded` — `.get()` would panic.
    for todo in user.todos.get_partial() {
        assert!(!todo.completed);
    }
}
```

The same applies to a filtered `HasOne` / `BelongsTo`, where
`.get_partial()` returns `Option<&T>`:

```rust
match user.profile.get_partial() {
    Some(profile) => { /* loaded and matches the filter */ }
    None => { /* either no profile exists, or it failed the filter */ }
}
```

From the parent's perspective a filtered-out 1-1 looks the same as a
missing relation, which is why the caller has to ask for the partial
value explicitly instead of reading a `None` through `.get()`.

`.is_partial()` reports the state when the two cases need different
handling:

```rust
if user.todos.is_partial() {
    // Filtered subset — iterate, but do not treat it as the full set.
    for todo in user.todos.get_partial() {
        // ...
    }
} else {
    // Full set — safe to count, paginate, derive aggregates.
    let total = user.todos.get().len();
}
```

The split is the point: a function that takes `&User` and reads
`user.todos.get()` keeps working only when its caller loaded the full
set. Callers that opt into filtered includes opt into the more explicit
accessor.

### Filters at any step of a chain

`.filter(...)` applies at its own step of a chained include path, at any
depth, and is evaluated in that step's relation target:

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

A filter therefore no longer terminates the path: `.filter(...)` and
`.order_by(...)` return a value that still exposes the target model's
relation accessors, so the chain continues. The same result stays
expressible as two `.include(...)` calls sharing the `posts` prefix; the
chained form avoids repeating the prefix.

### Filtering a `via` relation

Filters work on multi-step `via` relations, including many-to-many:

```rust
// Each user with only their non-archived groups preloaded.
let users: Vec<User> = User::all()
    .include(
        User::fields()
            .groups()
            .filter(Group::fields().archived().eq(false)),
    )
    .exec(&mut db)
    .await?;
```

The predicate is evaluated against the relation target — `Group` here,
not the join model. Filtering the join model is not expressible.

## Behavior

- **State assignment.** A step is `Partial` when every `.include(...)`
  contributing to it carries a filter. A step reached by any unfiltered
  include is `Loaded`, because the full set was fetched. States are
  per-step: in a nested include, each level's state is determined
  independently by whether that level was filtered.
- **Empty matches.** A `HasMany` parent whose children all fail the
  predicate is still returned, with its relation `Partial` carrying an
  empty `Vec`.
- **Errors.** `.get()` on a `Partial` relation panics with a message
  naming the relation and pointing at `.get_partial()`. This is a
  programmer error in the same class as `.get()` on an `Unloaded`
  relation, not a `toasty::Error`.
- **Interaction with `.unload()`.** Unloading a `Partial` relation
  returns it to `Unloaded`, same as unloading a `Loaded` one.

## Edge cases

- **Eager relations.** An eagerly loaded relation contributes an
  implicit unfiltered include, so filtering its include still yields the
  full set in the `Loaded` state. Filtering an eager relation has no
  observable effect.
- **Required singular relations.** Filtering the include of a
  non-nullable `HasOne` / `BelongsTo` is rejected with
  `invalid_statement`, so a `Partial` singular relation always carries
  an `Option` and `.get_partial()` always has a `None` case to report.
- **`via` on key-value backends.** Including a multi-step `via` relation
  requires the database to execute the join, so it is SQL-only; filtered
  `via` includes inherit that restriction rather than adding one.

## Driver integration

Nothing. Filtered includes are ordinary statements with extra `WHERE`
predicates, and the `Partial` state is decided by the engine when it
builds the include, not by the driver.

## Alternatives considered

- **No third state; document the hazard.** `.get()` keeps returning
  whatever was loaded and the guide warns about it. Rejected: the
  consumer of a `&User` is usually not the code that wrote the query,
  so the warning lands on the wrong person.
- **`.is_partial()` without changing `.get()`.** Cheaper and
  source-compatible, but every existing `.get()` call site stays
  silently wrong the first time someone upstream adds a filter.
- **Closure-based include builder** —
  `.include(|u| u.posts(|p| p.filter(...).comments(|c| c.filter(...))))`.
  Places modifiers at every step without a terminal-value problem, but
  adds a generated sub-builder per relation and diverges from the path
  syntax `.any` / `.all` already use.
- **Leave filters terminal; require two `.include(...)` calls.** Every
  nested filtered include is expressible that way already. Rejected:
  the shared prefix is repeated verbatim, and the reader has to match
  the calls up by path to see what a level loads.

## Open questions

- **`via` filters: thread or reject?** Threading a per-relation
  predicate through the `via` JOIN chain is the goal. If it does not
  work out, the fallback is an `unsupported_feature` error rather than
  a panic — but that leaves many-to-many, the most common case, without
  filters. Blocking implementation of the `via` case.
- **Subquery predicates inside include filters.** `.filter(...)` accepts
  any `Expr<bool>`, which includes `.any` / `.all` / `in_set` over a
  further relation. Whether include lowering handles a subquery-bearing
  predicate at every backend is unestablished, and nothing exercises it.
  Blocking implementation.
- **Predicate scope is not checked at compile time.** `.filter(...)`
  takes an `Expr<bool>`, which carries no origin model, so
  `User::fields().todos().filter(Post::fields().title().eq("x"))`
  type-checks. Tracking the origin in `Expr<bool>` would fix `.any` /
  `.all`, which have the same hole, but touches the whole predicate
  API; rejecting it at verify time with `invalid_statement` is the
  cheap option. Deferrable.

## Out of scope

- **Named filtered relations as model fields.** A complementary
  approach is to give a recurring filtered view its own field:

  ```rust
  #[derive(Model)]
  struct User {
      #[has_many]
      todos: Deferred<Vec<Todo>>,

      #[has_many(filter = Todo::fields().active().eq(true))]
      active_todos: Deferred<Vec<Todo>>,
  }
  ```

  `active_todos` would be a relation in its own right —
  `.include(User::fields().active_todos())` preloads it, and because
  the field denotes the filtered set, it loads as `Loaded` rather than
  `Partial`. It composes cleanly with include filters (the unnamed case
  stays `.filter(...)`; the named case gets a stable identity), but the
  macro surface, the syntax for the embedded predicate, and the
  interaction with `.any` / `.all` on the named relation each need
  their own treatment.
- **`.limit` on includes** — separate design.
- **Cross-scope predicates.** A filter like
  `Todo::fields().user_id().eq(User::fields().id())`, referencing
  parent fields, is not supported. `.any` / `.all` do not support it
  either.
- **Aggregations over filtered relations** (`count`, `sum`, …) —
  separate feature.
