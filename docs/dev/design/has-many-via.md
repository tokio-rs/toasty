# Multi-step relations (`has_many` / `has_one` via)

## Summary

Today a `has_many` or `has_one` relation is a single hop: one foreign key
links the two models. This adds a `via` option that lets a relation reach a
target by following a path of *existing* relations. A `User` that has many
`Comment`s, where each `Comment` belongs to an `Article`, can declare
`#[has_many(via = comments.article)] commented_articles: HasMany<Article>`
and query the distinct articles a user has commented on as a relation —
filterable, includable, and composable with the rest of the query API. The
`via` path may also step through embedded struct fields to reach a relation
that lives inside an embedded type.

## Motivation

The User → Comment → Article shape is common: a model is related to another
only *through* a third. Toasty has no way to express it. The user must leave
the relation API and chain queries by hand:

```rust,ignore
// Get the articles a user has commented on, the manual way.
let comments = user.comments().all(&mut db).await?.collect().await?;
let article_ids: Vec<_> = comments.iter().map(|c| c.article_id).collect();
let articles = Article::filter(Article::FIELDS.id().in_set(article_ids))
    .all(&mut db)
    .await?;
```

This is two round trips, it returns duplicates when a user comments on the
same article twice, and it is not a relation. It cannot be `.include()`d to
avoid the N+1, cannot be filtered as
`user.commented_articles().filter(...)`, and cannot be nested inside another
query.

The same gap appears whenever a relation is reachable only by composition:
`Customer` → `Order` → `Warehouse` (where the warehouse FK lives in an
embedded `ShippingInfo` struct on `Order`), or a classic join-model
many-to-many such as `User` → `Membership` → `Team`.

## User-facing API

### Declaring a multi-step relation

A `has_many` or `has_one` field declares a `via` path instead of pairing with
a `belongs_to`. The path is a dotted chain of relation fields, read left to
right, starting from the model the relation is declared on:

```rust
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[has_many]
    comments: toasty::HasMany<Comment>,

    // User → comments → article
    #[has_many(via = comments.article)]
    commented_articles: toasty::HasMany<Article>,
}

#[derive(Debug, toasty::Model)]
struct Comment {
    #[key]
    #[auto]
    id: u64,

    body: String,

    #[index]
    user_id: u64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<User>,

    #[index]
    article_id: u64,

    #[belongs_to(key = article_id, references = id)]
    article: toasty::BelongsTo<Article>,
}

#[derive(Debug, toasty::Model)]
struct Article {
    #[key]
    #[auto]
    id: u64,

    title: String,

    #[has_many]
    comments: toasty::HasMany<Comment>,
}
```

The relation's target type is `Article` because the path
`comments.article` ends at `Article`: `comments` is a `HasMany<Comment>`, and
`article` is a `BelongsTo<Article>` on `Comment`. Toasty checks at schema-build
time that the resolved target matches the declared `HasMany<Article>`.

A `via` relation needs no `pair` — it has no foreign key of its own. It is
*derived* entirely from the relations it traverses. Each step in the path may
be any relation kind (`belongs_to`, `has_many`, `has_one`); the kinds can be
mixed freely along one path.

### Querying through it

A `via` relation behaves like any other `has_many` for reads:

```rust
let user = User::get_by_id(&mut db, &1).await?;

// Every article this user has commented on, each listed once.
let articles = user.commented_articles().all(&mut db).await?;

// Filtered and ordered like any other relation query.
let recent = user
    .commented_articles()
    .filter(Article::FIELDS.published().eq(true))
    .order_by(Article::FIELDS.title())
    .all(&mut db)
    .await?;
```

It also composes with `.include()` to preload across the whole chain in one
plan:

```rust,ignore
let users = User::all()
    .include(User::FIELDS.commented_articles())
    .all(&mut db)
    .await?;
```

`has_one(via = ...)` works the same way and yields a single optional record —
use it when the path is guaranteed to reach at most one target, e.g.
`#[has_one(via = profile.organization)]`.

### Stepping through embedded fields

A `via` path is not limited to relation fields. When a relation is held inside
an embedded struct, the path steps through the embedded field to reach it:

```rust,ignore
#[derive(Debug, toasty::Model)]
struct Order {
    #[key] #[auto] id: u64,

    #[index] customer_id: u64,
    #[belongs_to(key = customer_id, references = id)]
    customer: toasty::BelongsTo<Customer>,

    // ShippingInfo is an embedded struct holding the warehouse FK.
    shipping: ShippingInfo,
}

#[derive(toasty::Embed)]
struct ShippingInfo {
    warehouse_id: u64,
    #[belongs_to(key = warehouse_id, references = id)]
    warehouse: toasty::BelongsTo<Warehouse>,
}

#[derive(Debug, toasty::Model)]
struct Customer {
    #[key] #[auto] id: u64,

    #[has_many]
    orders: toasty::HasMany<Order>,

    // Customer → orders → shipping (embedded) → warehouse
    #[has_many(via = orders.shipping.warehouse)]
    warehouses: toasty::HasMany<Warehouse>,
}
```

The `shipping` segment is an embedded struct field, not a relation; the path
keeps walking until it reaches the `warehouse` relation. (Embedded relations
themselves are a separate, in-progress feature — see Out of scope.)

### Read-only

A `via` relation is read-only in this design. You can query, filter, order,
paginate, and `.include()` it, but `.create()`, `.insert()`, and `.remove()`
are not generated for it. Writing through a multi-step path means
materializing or deleting intermediate records, and the right behavior is
not obvious — creating through `commented_articles` would have to invent a
`Comment`. For now, mutate the underlying relations directly
(`user.comments().create(...)`).

Whether read-only is a fundamental property of `via` relations or only a
limitation of this MVP is an open question — some path shapes may have
unambiguous write semantics. See Open questions.

## Behavior

**Resolution.** The `via` path is stored on the relation as a field path
rooted at the declaring model. At schema-build time Toasty walks the path one
segment at a time, re-rooting at each model it lands on, and resolves the
final target. The relation carries no `pair`.

**Lowering.** A `via` relation lowers to the same nested `IN (subquery)`
filter the engine already produces for single-step relations — one nesting
level per path step. `user.commented_articles()` becomes, in effect:

```sql
SELECT * FROM articles
WHERE id IN (
    SELECT article_id FROM comments
    WHERE user_id IN (SELECT id FROM users WHERE id = ?)
)
```

**Distinct by construction.** Because each step is a set-membership test, not
a join, a target row appears at most once regardless of how many intermediate
rows reach it. A user who comments on the same article ten times gets that
article once. This is a deliberate guarantee, and it differs from join-based
`has_many :through` in other ORMs. The limitation: a `via` relation yields
the distinct *targets*, not the join multiplicity — "how many comments link
this user to this article" is not available through the relation.

**Empty and null intermediates.** If an intermediate step produces nothing
(the user has no comments) the result is empty. If an intermediate step is an
optional `belongs_to` with a `NULL` foreign key, that row contributes nothing
to the next step — no error, no `NULL` target.

**Errors.** Path problems are reported at schema-build time, not at query
time:

- A segment that is not a relation and not an embedded field (e.g. a
  primitive `String` field) — the path cannot continue through it.
- A segment that does not name a field on the current model.
- A resolved target type that does not match the declared `HasMany<T>` /
  `HasOne<T>`.

**Interaction with `.include()`.** A `via` relation can be the target of
`.include()`. The include subquery builder follows the multi-step path the
same way the relation query does.

## Edge cases

- **`has_one(via = ...)` reaching more than one row.** The path's relation
  kinds do not by themselves guarantee uniqueness — `has_one(via = a.b)`
  where `a` is a `has_many` *can* reach many rows. Toasty applies the same
  single-row semantics as a plain `has_one` (the engine takes the first
  row); declaring `has_one` is an assertion by the model author.
- **Cycles.** Nothing prevents a path from revisiting a model
  (`a.b.a.b...`); the path is finite because it is written out explicitly, so
  this is not a loop. A `via` path that references *another* `via` relation
  is rejected in the first implementation (see Open questions).
- **NoSQL drivers.** Each subquery nesting level the planner cannot push down
  becomes a separate plan operation, materialized in sequence. A three-step
  `via` on DynamoDB is three dependent round trips. This is the same behavior
  as chaining single-step relations by hand — `via` does not make it worse,
  but a deep path costs one round trip per step.
- **Empty path / single-segment path.** A `via` with one segment is just a
  rename of an existing relation; allowed, but `pair` matching already covers
  the normal case.

## Driver integration

Nothing. A `via` relation lowers to nested `IN (subquery)` filters built from
the same `Association` machinery that single-step relations and `.include()`
already use. SQL drivers serialize the same subquery shape; non-SQL drivers
see the same materialize-and-feed plan operations. No new `Operation`
variants, no new capability flags, no SQL-dialect differences, and no impact
on out-of-tree drivers.

## Alternatives considered

**JOIN-based lowering.** Lower `via` to SQL `JOIN`s instead of nested
subqueries. Rejected: joins do not exist for NoSQL drivers, so the engine
would need two lowering strategies for one feature; and joins multiply rows,
forcing a `DISTINCT` pass to recover the natural set semantics. The
subquery-filter lowering is uniform across all drivers and is distinct by
construction.

**Named through-association (Rails `has_many :through`).** Name a single
intermediate association rather than a path:
`#[has_many(through = comments, source = article)]`. Rejected: it does not
generalize past two steps and has no way to express a step through an
embedded field. A path subsumes it: `comments.article` expresses the two-step
case and extends unchanged to `a.b.c.d`.

**A plain Rust method.** Let the user write
`fn commented_articles(&self) { self.comments()... }` by hand. Rejected: a
method is not a relation. It cannot be `.include()`d, cannot be filtered as a
relation query, and cannot be nested inside another model's query or
`.include()`.

## Open questions

- **Variant navigation syntax.** When the relation lives in an *embedded
  enum* variant rather than an embedded struct, the path must select a
  variant before it can name the variant's field. The schema `Path` type
  already supports a variant root, but the `via` attribute needs a notation
  for it — e.g. `orders.payment.account` (resolve `account` against all
  variants, error on ambiguity) versus an explicit
  `orders.payment[Account].account`. *Blocking implementation of the
  embedded-enum case; deferrable for plain-relation and embedded-struct
  paths.*
- **`via` referencing another `via`.** Path composition through a derived
  relation is natural but needs cycle detection across relation definitions.
  *Deferrable — reject for now.*
- **Distinctness as a committed contract.** The IN-subquery lowering makes
  `via` results distinct. Should that be a documented API guarantee users may
  rely on, or an implementation detail? *Blocking acceptance — it shapes user
  expectations.*
- **Is read-only fundamental?** This design generates no mutation methods on
  a `via` relation. It is unclear whether that is inherent or only the
  conservative MVP choice — some path shapes (for example, a path whose only
  non-`belongs_to` step is the last) may have unambiguous insert/remove
  semantics worth exposing later. *Deferrable — does not block the read-only
  MVP.*

## Out of scope

- **Writing through a `via` relation.** Read-only for now; whether some path
  shapes can support unambiguous create/insert/remove is an open question,
  not a settled "never."
- **`belongs_to(via = ...)`.** `belongs_to` is the foreign-key-owning side; a
  multi-step `belongs_to` has no foreign key to own. Only `has_many` and
  `has_one` get `via`.
- **Embedded enum variant paths.** Depends on the variant-syntax open
  question above and on relations-in-embedded-types support
  ([#670]). Embedded *struct* paths are in scope; embedded *enum variant*
  paths are not, in the first implementation.
- **Implicit-join-table many-to-many.** A join-model many-to-many
  (`User` → `Membership` → `Team`) is expressible today as
  `has_many(via = memberships.team)`. A many-to-many that hides the join
  model entirely remains the separate "Many-to-many" roadmap item.
- **Join multiplicity / aggregate-through.** `via` yields distinct targets;
  counting or aggregating over the intermediate rows that connect a source to
  a target is a separate concern (see the aggregates roadmap item).

[#670]: https://github.com/tokio-rs/toasty/issues/670
