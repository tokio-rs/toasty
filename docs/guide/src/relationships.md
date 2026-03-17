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

```
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

This single pattern — a foreign key column in one table referencing the primary
key of another — underlies all three relationship types in Toasty.

## Relationship types

Toasty supports three relationship types. They differ in how many records each
side of the relationship holds, and which model contains the foreign key.

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
    posts: toasty::HasMany<Post>,
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
    user: toasty::BelongsTo<User>,

    title: String,
}
```

A simple rule: **the foreign key always lives on the "many" side** (or the
"dependent" side in a one-to-one). If a post can only have one author, the FK
goes on posts, not on users.

### Relationship pairs

Most relationships are bidirectional — declared on both models. The `User` above
has `#[has_many] posts` and the `Post` has `#[belongs_to] user`. Toasty matches
these two sides into a **pair** automatically by looking at the model types. If
the field names don't follow the default convention, use `pair` to link them
explicitly:

```rust,ignore
// On User: the child's relation field is named "owner", not "user"
#[has_many(pair = owner)]
posts: toasty::HasMany<Post>,
```

You can also define one-sided relationships (only `#[belongs_to]` without a
corresponding `#[has_many]` on the parent). This is useful when you need to
navigate from child to parent but not the reverse.

## Required vs optional relationships

The nullability of the foreign key field controls whether the relationship is
required or optional.

### Required: non-nullable foreign key

```rust,ignore
#[index]
user_id: u64,

#[belongs_to(key = user_id, references = id)]
user: toasty::BelongsTo<User>,
```

Every post must have a user. The `user_id` column is `NOT NULL` in the database.

### Optional: nullable foreign key

```rust,ignore
#[index]
user_id: Option<u64>,

#[belongs_to(key = user_id, references = id)]
user: toasty::BelongsTo<Option<User>>,
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
#     posts: toasty::HasMany<Post>,
# }
# #[derive(Debug, toasty::Model)]
# struct Post {
#     #[key]
#     #[auto]
#     id: u64,
#     #[index]
#     user_id: u64,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::BelongsTo<User>,
#     title: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let user = User::create()
    .name("Alice")
    .post(Post::create().title("Hello"))
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
| A post has one author | `Post` → `BelongsTo<User>` + `User` → `HasMany<Post>` | `posts.user_id` |
| A user has one profile | `User` → `HasOne<Profile>` + `Profile` → `BelongsTo<User>` | `profiles.user_id` |
| A comment belongs to a post | `Comment` → `BelongsTo<Post>` + `Post` → `HasMany<Comment>` | `comments.post_id` |

When deciding between `HasOne` and `HasMany`, ask: "Can the parent have more
than one?" If yes, use `HasMany`. If exactly one (or zero), use `HasOne`. The
foreign key placement is the same either way — it always goes on the child.

When deciding between `HasOne` and `BelongsTo` for a one-to-one relationship,
ask: "Which model is the dependent one — the one that doesn't make sense without
the other?" Put the FK on the dependent model with `BelongsTo`, and declare
`HasOne` on the independent model.

## What the following chapters cover

Each relationship type has its own chapter with full details on definition,
querying, creating, and updating:

- [**BelongsTo**](./belongs-to.md) — defining foreign keys, accessing the
  parent, setting the relation on create
- [**HasMany**](./has-many.md) — querying children, creating through the
  relation, inserting and removing, scoped queries
- [**HasOne**](./has-one.md) — required vs optional, creating and updating the
  child, replace and unset behavior
- [**Preloading Associations**](./preloading-associations.md) — avoiding extra
  queries by loading relations upfront with `.include()`
