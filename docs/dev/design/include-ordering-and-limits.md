# Ordering and limiting included relations

## Summary

Add `.order_by(...)` and `.limit(...)` to many-valued relation includes. The
modifiers order and limit the related records loaded for each parent, so a query
can preload records such as the three newest posts for every user without
changing which users the parent query returns.

## Motivation

An include can now filter a relation, but it cannot control the order or number
of matching records. Loading a dashboard with each user's latest posts requires
loading every matching post or querying each user separately. The first option
returns records the application does not use. The second loses include batching
and makes the application attach posts to users itself.

Ordering without a limit is also useful. Code that renders a relation in a
defined order should not have to sort every loaded `Vec` after the query
finishes.

## User-facing API

Call `.order_by(...)` and `.limit(...)` on a many-valued relation path passed to
`.include(...)`. The expressions passed to `.order_by(...)` use the included
model's fields, just like ordering a query for that model directly.

The generated relation handles expose `.filter(...)`, `.order_by(...)`, and
`.limit(...)` uniformly. These modifiers are supported only when the last
relation in the include path is many-valued. Toasty rejects their use on a
singular relation when the query executes.

### Loading the first related records

The limit applies independently to every parent. This query loads up to three
posts for each user, not three posts across all users:

```rust
let users: Vec<User> = User::all()
    .include(
        User::fields()
            .posts()
            .order_by(Post::fields().published_at().desc())
            .order_by(Post::fields().id().asc())
            .limit(3),
    )
    .exec(&mut db)
    .await?;

for user in users {
    for post in user.posts.get() {
        // At most three posts, newest first.
    }
}
```

The second ordering expression breaks ties between posts with the same
`published_at`. Add a unique final ordering expression when the selected records
must be stable across repeated executions.

### Combining filtering, ordering, and limiting

Include modifiers compose in the same expression. Their call order does not
change their logical order: Toasty filters the relation, orders the matching
records, and then applies the limit.

```rust
let users: Vec<User> = User::all()
    .include(
        User::fields()
            .posts()
            .filter(Post::fields().published().eq(true))
            .order_by(Post::fields().published_at().desc())
            .order_by(Post::fields().id().asc())
            .limit(3),
    )
    .exec(&mut db)
    .await?;
```

The parent query remains independent. Add a parent-side `.filter(...)` or
`.order_by(...)` when the users themselves also need filtering or ordering.

### Singular relations are rejected at runtime

The modifier methods are present on `HasOne` and `BelongsTo` paths in the first
version, but using any of them produces an `invalid_statement` error. This
applies to required and optional singular relations.

Before this change, an optional singular relation accepts an include filter and
loads `None` when the related row does not match. With this design, the same
call still compiles but returns `invalid_statement`. Many-valued include filters
keep their existing behavior.

```rust
let result = User::all()
    .include(
        User::fields()
            .profile()
            .filter(Profile::fields().public().eq(true)),
    )
    .exec(&mut db)
    .await;

// result is Err(invalid_statement)
```

The error identifies `profile` as a singular relation and states that include
filters, ordering, and limits require a many-valued relation.

### Modifying more than one relation level

A modifier applies to the last relation in its include path. Use separate
includes when more than one level needs options:

```rust
let users: Vec<User> = User::all()
    .include(
        User::fields()
            .posts()
            .order_by(Post::fields().published_at().desc())
            .limit(5),
    )
    .include(
        User::fields()
            .posts()
            .comments()
            .filter(Comment::fields().approved().eq(true))
            .order_by(Comment::fields().created_at().desc())
            .limit(2),
    )
    .exec(&mut db)
    .await?;
```

This loads up to five posts per user and up to two approved comments per loaded
post. The two includes share `posts` as a prefix, but end at different relation
paths, so they do not conflict.

### Combining repeated includes

Toasty merges modifiers from every include ending at the same relation path.
Each include first builds its own ordering using normal query semantics:
multiple `.order_by(...)` calls append expressions, and later expressions act
as tie-breakers. A tuple is a shorter way to add several expressions at once.

When separate includes provide ordering for the same relation path, the entire
ordering from the later include replaces the earlier ordering. An include
without `.order_by(...)` does not clear an ordering supplied by another
include.

```rust
let users: Vec<User> = User::all()
    .include(
        User::fields().posts().order_by((
            Post::fields().id().asc(),
            Post::fields().published_at().desc(),
        )),
    )
    .include(
        User::fields()
            .posts()
            .order_by(Post::fields().published_at().asc()),
    )
    .exec(&mut db)
    .await?;
```

The effective ordering is only `published_at ASC`. The second include replaces
the complete `id ASC, published_at DESC` ordering from the first include.

Every explicit `.limit(...)` replaces the previously merged limit. The last
explicit limit wins, whether the calls appear in one include expression or in
separate includes. An include without a limit does not clear an earlier limit.

```rust
let users: Vec<User> = User::all()
    .include(User::fields().posts().limit(default_limit))
    .include(User::fields().posts().limit(3))
    .exec(&mut db)
    .await?;
```

The merged include has a limit of three.

Filters keep their existing merge rule. Filters from separate includes combine
with `OR`; repeated filters in one include combine with `AND`; and a bare
include removes filtering for that path. Ordering and limits still apply after
the filter merge. For example, two filtered includes with limits form one
relation query with the combined filter, merged ordering, and last explicit
limit. They do not produce two independently limited subsets.

The following query loads up to five posts that are either published or pinned:

```rust
let selected = Post::fields()
    .published()
    .eq(true)
    .or(Post::fields().pinned().eq(true));

let users: Vec<User> = User::all()
    .include(
        User::fields()
            .posts()
            .filter(selected)
            .order_by(Post::fields().published_at().desc())
            .limit(5),
    )
    .exec(&mut db)
    .await?;
```

## Behavior

- For each parent, Toasty evaluates the include filter, orders the matching
  related records, and retains at most the requested limit.
- Include ordering controls the records stored in that relation. It does not
  affect the order of the parent query or another included relation.
- Includes ending at the same relation path produce one relation query. Their
  filters, ordering, and limits merge using the rules above.
- An include limit never removes a parent. A parent with no matching related
  records has an empty loaded relation.
- `.limit(0)` loads an empty relation for every parent. A limit greater than the
  number of matching records loads all matching records.
- `.limit(...)` without `.order_by(...)` is allowed, as it is on a normal query.
  The selected records have no stable order. Applications that depend on which
  records are selected must provide a deterministic order.
- Ordering uses the target database's native comparison, collation, and null
  ordering behavior. Toasty does not normalize those differences across
  backends.
- Using `.filter(...)`, `.order_by(...)`, or `.limit(...)` on an include whose
  last relation is `HasOne` or `BelongsTo` returns `invalid_statement`. The
  relation's nullability does not change this rule.
- An ordered eager relation still loads the full relation in the requested
  order. A limited include of an eager relation is invalid because an eager
  relation promises the full relation value. Declare it as `Deferred` if a
  query should load a limited subset.
- Driver failures propagate as `toasty::Error`. A backend that cannot execute
  the requested ordering for the selected access path returns
  `unsupported_feature`.

The engine may fetch more related records than the final limit while executing
a batched include. The limit is a result contract, not a guarantee about the
number of rows read from the database. This leaves room to add per-parent limit
pushdown without changing user code.

## Edge cases

When ordering expressions compare equal, the database may return tied records
in any order. A limit can therefore select different tied records on different
executions. Ordering by a unique field last makes the selection deterministic.

An include ending at `posts.comments` configures `comments`, not `posts`.
Another include ending at `posts` may configure the post order and limit; this
is a shared prefix, not a repeated terminal include.

Cardinality validation uses the last relation step. A modifier on
`posts.author` is rejected when `author` is singular, even though `posts` is
many-valued. A modifier on `profile.posts` is allowed when `posts` is
many-valued, even though `profile` is singular.

A bare include contributes no ordering or limit. It makes the merged relation
unfiltered, but explicit ordering and the last explicit limit still apply.

When several includes ending at the same relation specify ordering, the last
such include supplies the complete ordering. Reversing those include calls can
therefore replace the complete ordering. Includes without ordering do not
participate in this choice.

A limit greater than `i64::MAX` panics while constructing the include, matching
the existing top-level `.limit(...)` behavior.

## Driver integration

This feature adds no `Driver` capability flags and no `Operation` variants.
Out-of-tree drivers require no source changes.

The query verifier rejects modifiers on singular relation includes before the
engine dispatches an operation. Drivers do not receive these invalid queries.

The engine sends include ordering through the same query operation used for
top-level ordering. SQL drivers receive the ordinary `ORDER BY` expressions on
the child query. Key-value drivers receive their existing ordering field when
the selected index supports it. Existing backend restrictions still apply; for
example, a DynamoDB scan cannot implement arbitrary ordering.

The engine applies the include limit independently while merging each parent's
related records. It must not forward that value as one global limit on a
batched child query, because that would limit all parents together. Drivers do
not need a grouped-limit operation for the initial implementation.

## Alternatives considered

**Reject repeated terminal includes with modifiers.** Rejection avoids deciding
which ordering or limit wins, but it prevents independent application code from
adding options to the same relation. Last-ordering-wins and last-limit-wins give
the merged relation one deterministic query.

**Merge ordering fields across separate includes.** Keeping each field's first
position while replacing repeated fields lets separate includes contribute
tie-breakers, but makes the result depend on field-level merge rules. Replacing
the complete ordering at the include boundary keeps composition separate from
the append behavior inside one include.

**Append ordering from separate includes.** Treating separate includes like
chained `.order_by(...)` calls prevents later application code from replacing
an earlier ordering. Replacement makes the include boundary an explicit
override point.

**Omit modifier methods from singular relation handles.** A compile-time error
is preferable to a runtime error, but the current generated path types do not
preserve the last relation's cardinality through every nested traversal. The
first version emits the methods uniformly and validates the terminal relation
at runtime. A later path-type change can remove the methods without changing
valid many-valued include queries.

**Execute each repeated include and union the results.** This preserves each
request's limit, but it requires extra queries, deduplication, and a rule for
ordering the union. Merging all options into one relation query avoids those
extra semantics.

**Apply one limit to the batched child query.** A global limit makes the result
depend on which parents happen to appear first and can leave later parents with
no related records. Include limits are per parent.

**Require `.order_by(...)` before `.limit(...)`.** A required order would make
limited selection deterministic only when the ordering itself is unique. It
would also differ from normal Toasty queries. The API allows an unordered limit
and documents its unspecified selection.

**Require native grouped-limit support.** SQL window functions and per-key
queries can reduce the rows transferred for some backends, but making them a
requirement would withhold useful result semantics from other access paths.
The initial implementation limits during relation merging; native pushdown can
be added as an optimization.

## Open questions

None.

## Out of scope

- **Offset and cursor pagination inside an include.** Pagination state belongs
  to one relation traversal and needs a separate API from eager loading.
- **Per-parent total counts.** A limited include returns records, not the number
  of additional matching records.
- **Native per-parent limit pushdown.** SQL window queries and backend-specific
  key queries may optimize execution later without changing this contract.
- **Compile-time rejection for singular relations.** The first version reports
  `invalid_statement`; cardinality-aware generated path types can make these
  calls fail to compile later.
- **New support for multi-step `via` includes.** A backend must already support
  the include path and its filter/order expressions; this design does not add
  new `via` lowering.
