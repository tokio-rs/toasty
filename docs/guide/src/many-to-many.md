# Many-to-Many Relationships

A many-to-many relationship connects multiple records on each side. A user can
join multiple groups, and each group can contain multiple users. Relational
databases store these connections in a join table with one foreign key for each
endpoint.

Toasty represents the join table as a model. Direct `HasMany` and `BelongsTo`
relations connect each endpoint to that join model. A
`#[has_many(via = ...)]` field provides direct, read-only traversal from one
endpoint to the other.

## Defining the join model

The following schema connects `User` and `Group` through `Membership`:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[has_many]
    memberships: toasty::Deferred<Vec<Membership>>,

    #[has_many(via = memberships.group)]
    groups: toasty::Deferred<Vec<Group>>,
}

#[derive(Debug, toasty::Model)]
struct Group {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[has_many]
    memberships: toasty::Deferred<Vec<Membership>>,

    #[has_many(via = memberships.user)]
    users: toasty::Deferred<Vec<User>>,
}

#[derive(Debug, toasty::Model)]
#[key(user_id, group_id)]
struct Membership {
    #[index]
    user_id: u64,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::Deferred<User>,

    #[index]
    group_id: u64,

    #[belongs_to(key = group_id, references = id)]
    group: toasty::Deferred<Group>,

    role: String,
}
```

Each relation has a separate job:

- `User::memberships` and `Group::memberships` are direct, mutable relations to
  the join model.
- `Membership::user` and `Membership::group` own the two foreign keys.
- `User::groups` follows `memberships.group` to reach groups without exposing
  the join step to callers.
- `Group::users` follows `memberships.user` in the opposite direction.

The composite primary key on `(user_id, group_id)` permits one membership for
each user-group pair. Use a separate generated primary key when duplicate join
rows have distinct meanings. A `via` query still returns each target once; query
the join model when each duplicate row must remain visible.

The join model can store data about the connection. `Membership::role` belongs
on `Membership` because it describes one user's role in one group, not the user
or group independently.

## Creating a link

Create a join-model record to connect two existing endpoints:

```rust,ignore
let membership = toasty::create!(Membership {
    user: &user,
    group: &group,
    role: "member",
})
.exec(&mut db)
.await?;
```

The `user` and `group` setters fill `user_id` and `group_id`. Creating the
membership does not update either endpoint row.

The derived `user.groups()` relation is read-only. It does not generate
`create`, `insert`, or `remove` operations because those operations would need
to construct or identify a `Membership` row. Create and delete `Membership`
records instead.

## Querying both directions

The derived relations return the opposite endpoints:

```rust,ignore
let groups: Vec<Group> = user.groups().exec(&mut db).await?;
let users: Vec<User> = group.users().exec(&mut db).await?;
```

These accessors support the same query operations as other relation queries:

```rust,ignore
let rust_groups = user
    .groups()
    .filter(Group::fields().name().eq("Rust"))
    .order_by(Group::fields().name().asc())
    .exec(&mut db)
    .await?;
```

A derived `via` relation returns distinct targets. If multiple join rows reach
the same group, `user.groups()` returns that group once.

## Filtering endpoints

Use `.any()` on the derived relation when the condition only references the
opposite endpoint:

```rust,ignore
let rust_users = User::filter(
    User::fields()
        .groups()
        .any(Group::fields().name().eq("Rust")),
)
.exec(&mut db)
.await?;
```

The expression reads as: select users for whom at least one related group is
named `Rust`.

Use `.any()` on the direct join-model relation when the condition needs fields
from `Membership`:

```rust,ignore
let owners = User::filter(
    User::fields()
        .memberships()
        .any(Membership::fields().role().eq("owner")),
)
.exec(&mut db)
.await?;
```

The predicate can also traverse from the join model to the opposite endpoint:

```rust,ignore
let rust_users = User::filter(
    User::fields()
        .memberships()
        .any(Membership::fields().group().name().eq("Rust")),
)
.exec(&mut db)
.await?;
```

Choose the derived relation for endpoint fields and the direct join relation for
join metadata.

## Preloading endpoints

Preload the derived relation when a query returns multiple endpoint records:

```rust,ignore
let users = User::all()
    .include(User::fields().groups())
    .exec(&mut db)
    .await?;

for user in &users {
    let groups: &[Group] = user.groups.get();
    println!("{} belongs to {} groups", user.name, groups.len());
}
```

`.include()` loads and groups the distinct targets for every parent in the
result. Accessing `user.groups.get()` reads the preloaded collection without
issuing another query.

## Updating and removing a link

Update fields on the join model directly:

```rust,ignore
membership.update().role("owner").exec(&mut db).await?;
```

Delete the join-model record to remove the connection:

```rust,ignore
membership.delete().exec(&mut db).await?;
```

Deleting a membership leaves the `User` and `Group` records intact because the
membership belongs to both endpoints; neither endpoint belongs to the
membership.

Deleting an endpoint removes its required membership rows but leaves records on
the opposite endpoint intact. Deleting a user therefore removes that user's
memberships without deleting any groups.

## Self-referential many-to-many relationships

A self-referential join model points both foreign keys at the same endpoint
model. A follower graph uses one foreign key for the follower and one for the
followed user:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[has_many(pair = follower)]
    outgoing_follows: toasty::Deferred<Vec<Follow>>,

    #[has_many(pair = followed)]
    incoming_follows: toasty::Deferred<Vec<Follow>>,

    #[has_many(via = outgoing_follows.followed)]
    following: toasty::Deferred<Vec<User>>,

    #[has_many(via = incoming_follows.follower)]
    followers: toasty::Deferred<Vec<User>>,
}

#[derive(Debug, toasty::Model)]
#[key(follower_id, followed_id)]
struct Follow {
    #[index]
    follower_id: u64,

    #[belongs_to(key = follower_id, references = id)]
    follower: toasty::Deferred<User>,

    #[index]
    followed_id: u64,

    #[belongs_to(key = followed_id, references = id)]
    followed: toasty::Deferred<User>,
}
```

`pair` tells Toasty which `Follow` relation matches each direct `HasMany`; model
types alone cannot distinguish two `BelongsTo<User>` fields. The two `via` paths
then name the direction explicitly: outgoing rows reach followed users, and
incoming rows reach followers.

## Backend support

Queries that traverse `has_many(via = ...)`, including relation accessors,
`.any()`, `.include()`, and `.select()`, require a SQL backend. They are supported
on SQLite, PostgreSQL, and MySQL and are not available on DynamoDB. Creating,
updating, querying, and deleting the join model use its ordinary model APIs.

See [HasMany](./has-many.md#multi-step-relations-via) for the general `via`
rules, [Filtering with Expressions](./filtering-with-expressions.md#filtering-on-associations)
for association predicates, and [Preloading Associations](./preloading-associations.md)
for loaded-state behavior.
