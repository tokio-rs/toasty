# Relationships

Models rarely exist in isolation. A blog has users, posts, and comments. An
e-commerce site has customers, orders, and products. Relationships define how
these models connect to each other.

In Toasty, you declare relationships on your model structs using attributes like
`#[belongs_to]`, `#[has_many]`, and `#[has_one]`. Toasty uses these declarations
to generate methods for traversing between models, creating related records, and
maintaining data consistency when records are deleted or updated.

## How relationships work at the database level

Relationships are implemented through **foreign keys** — a column in one table
that stores the primary key of a row in another table. For example, a `posts`
table has a `user_id` column that references the `users` table:

```text
users                    posts
┌────┬───────┐          ┌────┬──────────┬─────────┐
│ id │ name  │          │ id │ title    │ user_id │
├────┼───────┤          ├────┼──────────┼─────────┤
│  1 │ Alice │◄─────────│  1 │ Hello    │       1 │
│  2 │ Bob   │◄────┐    │  2 │ World    │       1 │
└────┴───────┘     └────│  3 │ Goodbye  │       2 │
                        └────┴──────────┴─────────┘
```

The `posts` table holds the foreign key (`user_id`). Each post points to exactly
one user. A user can have many posts.

This foreign-key pattern underlies each direct relationship type in Toasty. A
many-to-many relationship uses two direct relationships joined through a third
model.

## Many-to-many uses a join model

A many-to-many relationship lets each record on either side connect to multiple
records on the other side. Users can join multiple groups, and groups can contain
multiple users. A `memberships` table represents each user-group connection as a
row with foreign keys to both endpoint tables:

```text
users              memberships                 groups
┌────┬───────┐     ┌─────────┬──────────┐      ┌────┬───────────┐
│ id │ name  │     │ user_id │ group_id │      │ id │ name      │
├────┼───────┤     ├─────────┼──────────┤      ├────┼───────────┤
│  1 │ Alice │◄────│       1 │       10 │─────►│ 10 │ Rust      │
│  2 │ Bob   │◄────│       2 │       10 │─────►│ 20 │ Databases │
└────┴───────┘     │       1 │       20 │─────►└────┴───────────┘
                   └─────────┴──────────┘
```

In Toasty, the join table is a normal model such as `Membership`. Each endpoint
has a direct `#[has_many]` relation to the join model and a derived
`#[has_many(via = ...)]` relation to the opposite endpoint. Code creates,
updates, and deletes join-model records to change the connections; the derived
relation provides read-only traversal. The [Many-to-Many](./many-to-many.md)
chapter shows the model definition, traversal, filtering, preloading, and link
mutation.

## Relationship types

Toasty supports three direct relationship types. They differ in how many records
each side holds and which model contains the foreign key. Many-to-many is a
modeling pattern composed from these types rather than a fourth relation
attribute.

| Type | Foreign key on | Parent has | Child has | Example |
|---|---|---|---|---|
| [BelongsTo](./belongs-to.md) | This model | — | One parent | A post belongs to a user |
| [HasMany](./has-many.md) | Other model | Many children | — | A user has many posts |
| [HasOne](./has-one.md) | Other model | One child | — | A user has one profile |

### Which model gets which attribute?

The model whose table **contains the foreign key column** declares
`#[belongs_to]`. The model on the other side declares `#[has_many]` or
`#[has_one]`.

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    // User's table has no FK — declares has_many
    #[has_many]
    posts: toasty::Deferred<Vec<Post>>,
}

#[derive(Debug, toasty::Model)]
struct Post {
    #[key]
    #[auto]
    id: u64,

    // Post's table has the FK — declares belongs_to
    #[index]
    user_id: u64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::Deferred<User>,

    title: String,
}
```

### Relationship pairs

Most relationships are bidirectional — declared on both models. The `User` above
has `#[has_many] posts` and the `Post` has `#[belongs_to] user`. Toasty matches
these two sides into a **pair** automatically by looking at the model types —
field names do not factor into the matching. If there is ambiguity (for example,
a model with two `BelongsTo` relations pointing to the same parent type), use
`pair` to link them explicitly:

```rust,ignore
// On User: the child's relation field is named "owner", not "user"
#[has_many(pair = owner)]
posts: toasty::Deferred<Vec<Post>>,
```

You can define one-sided relationships with only `#[belongs_to]` on the child
and no corresponding `#[has_many]` or `#[has_one]` on the parent. This is useful
when you need to navigate from child to parent but not the reverse. The opposite
is not allowed — a `#[has_many]` or `#[has_one]` field always requires a
matching `#[belongs_to]` on the target model, because Toasty needs the foreign
key definition to know how the models connect.

### Lazy and eager relation fields

The relation field type controls when Toasty loads the related records.

Use `Deferred<_>` for a lazy relation. A normal query leaves the field unloaded.
Load it by calling the generated relation accessor, or preload it with
`.include(...)`:

```rust,ignore
#[has_many]
posts: toasty::Deferred<Vec<Post>>,

let posts = user.posts().exec(&mut db).await?;
```

Use the relation value directly for an eager relation. Toasty loads the relation
with every query that returns the model, as if the query had an implicit
`.include(...)`:

```rust,ignore
#[has_many]
posts: Vec<Post>,

let user = User::filter_by_id(user_id).get(&mut db).await?;
let post_count = user.posts.len();
```

The accepted eager field types are:

| Attribute | Lazy field type | Eager field type |
|---|---|---|
| `#[has_many]` | `Deferred<Vec<T>>` | `Vec<T>` |
| `#[has_one]` | `Deferred<T>` or `Deferred<Option<T>>` | `T` or `Option<T>` |
| `#[belongs_to]` | `Deferred<T>` or `Deferred<Option<T>>` | `T` or `Option<T>` |

Eager relations can load other eager relations. Toasty rejects schemas with an
eager-load cycle, such as `User.posts: Vec<Post>` and `Post.user: User`. Wrap at
least one relation in `Deferred<_>` to break the cycle.

## Required vs optional relationships

The nullability of the foreign key field controls whether the relationship is
required or optional.

### Required: non-nullable foreign key

```rust,ignore
#[index]
user_id: u64,

#[belongs_to(key = user_id, references = id)]
user: toasty::Deferred<User>,
```

Every post must have a user. The `user_id` column is `NOT NULL` in the database.

### Optional: nullable foreign key

```rust,ignore
#[index]
user_id: Option<u64>,

#[belongs_to(key = user_id, references = id)]
user: toasty::Deferred<Option<User>>,
```

A post can exist without a user. The `user_id` column allows `NULL`.

This distinction matters beyond just data modeling — it determines what happens
when a relationship is broken, as the next section explains.

## Data consistency on delete and unlink

When you delete a parent record or disassociate a child, Toasty automatically
maintains consistency based on the foreign key's nullability:

| Action | FK is required (`u64`) | FK is optional (`Option<u64>`) |
|---|---|---|
| Delete parent | Child is **deleted** | Child stays, FK set to `NULL` |
| Unset relation (e.g., `update().profile(None)`) | Child is **deleted** | Child stays, FK set to `NULL` |
| Delete child | Parent is unaffected | Parent is unaffected |

The logic: a required foreign key means the child cannot exist without its
parent. If the parent goes away, the child must go too. An optional foreign key
means the child can stand on its own, so Toasty sets the FK to `NULL` and leaves
the child in place.

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_many]
#     posts: toasty::Deferred<Vec<Post>>,
# }
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     user_id: u64,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::Deferred<User>,
#     title: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let user = toasty::create!(User {
    name: "Alice",
    posts: [{ title: "Hello" }],
})
.exec(&mut db)
.await?;

let posts = user.posts().exec(&mut db).await?;
assert_eq!(1, posts.len());

// user_id is required (u64), so deleting the user deletes the post too
user.delete().exec(&mut db).await?;

assert!(Post::get_by_id(&mut db, &posts[0].id).await.is_err());
# Ok(())
# }
```

If `user_id` were `Option<u64>` instead, the post would survive the deletion
with `user_id` set to `None`.

This behavior is applied at the application level by Toasty's query engine, not
by database-level foreign key constraints. Toasty inspects the schema and
generates the appropriate cascade deletes or null-setting updates automatically.

## Choosing the right relationship type

| You want to express… | Use | FK goes on |
|---|---|---|
| A post has one author | `Post` → `Deferred<User>` or `User` + `User` → `Deferred<Vec<Post>>` or `Vec<Post>` | `posts.user_id` |
| A user has one profile | `User` → `Deferred<Profile>` or `Profile` + `Profile` → `Deferred<User>` or `User` | `profiles.user_id` |
| A comment belongs to a post | `Comment` → `Deferred<Post>` or `Post` + `Post` → `Deferred<Vec<Comment>>` or `Vec<Comment>` | `comments.post_id` |
| Users join many groups and groups contain many users | Join model with two `BelongsTo` relations plus `has_many(via = ...)` on the endpoints | `memberships.user_id` and `memberships.group_id` |

When deciding between `HasOne` and `HasMany`, ask: "Can the parent have more
than one?" If yes, use `HasMany`. If exactly one (or zero), use `HasOne`. The
foreign key placement is the same either way — it always goes on the child.

When deciding between `HasOne` and `BelongsTo` for a one-to-one relationship,
ask: "Which model is the dependent one — the one that doesn't make sense without
the other?" Put the FK on the dependent model with `BelongsTo`, and declare
`HasOne` on the independent model.

## Composite foreign keys

When a parent model has a composite primary key, the foreign key on the child
spans the same set of columns. Pass arrays to `key` and `references` to list
each column:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
#[key(org_id, id)]
struct Team {
    org_id: u64,
    id: u64,

    #[has_many]
    members: toasty::Deferred<Vec<Member>>,
}

#[derive(Debug, toasty::Model)]
#[index(org_id, team_id)]
struct Member {
    #[key]
    #[auto]
    id: u64,

    org_id: u64,
    team_id: u64,

    #[belongs_to(key = [org_id, team_id], references = [org_id, id])]
    team: toasty::Deferred<Team>,
}
```

The first field in `key` pairs with the first field in `references`, the second
with the second, and so on, so the two arrays must have the same length. With
a single-column foreign key, the arrays can be omitted: `key = user_id,
references = id` is equivalent to `key = [user_id], references = [id]`.

The foreign key fields need a model-level composite index that covers them in
order — `#[index(org_id, team_id)]` on the struct, not a separate `#[index]`
on each field. Two single-column indexes don't compose into a covering index
for a composite foreign key. Without one, schema verification rejects the
model and suggests the exact attribute to add.

## What the following chapters cover

Each relationship type has its own chapter with full details on definition,
querying, creating, and updating:

- [**BelongsTo**](./belongs-to.md) — defining foreign keys, accessing the
  parent, setting the relation on create
- [**HasMany**](./has-many.md) — querying children, creating through the
  relation, inserting and removing, scoped queries
- [**Many-to-Many**](./many-to-many.md) — defining a join model, traversing in
  both directions, filtering by endpoints or join metadata, and changing links
- [**HasOne**](./has-one.md) — required vs optional, creating and updating the
  child, replace and unset behavior
- [**Preloading Associations**](./preloading-associations.md) — avoiding extra
  queries by loading relations upfront with `.include()`

> **Runnable example:** [`forum-relationships`] loads and traverses relations — `has_one`, preloading with `.include()`, `via` relations, and association filters.

[`forum-relationships`]: https://github.com/tokio-rs/toasty/tree/main/examples/forum-relationships
