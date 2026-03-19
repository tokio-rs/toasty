# HasOne

A `HasOne` relationship connects a parent model to a single child record. Like
[HasMany](./has-many.md), the foreign key lives on the child model, but
`HasOne` enforces that at most one child exists per parent.

## Defining a HasOne relationship

Add a `#[has_one]` field of type `HasOne<T>` on the parent model. The child
model must have a corresponding `#[belongs_to]` field with a `#[unique]` foreign
key (since each parent maps to at most one child):

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[has_one]
    profile: toasty::HasOne<Option<Profile>>,
}

#[derive(Debug, toasty::Model)]
struct Profile {
    #[key]
    #[auto]
    id: u64,

    #[unique]
    user_id: Option<u64>,

    #[belongs_to(key = user_id, references = id)]
    user: toasty::BelongsTo<Option<User>>,

    bio: String,
}
```

The child's foreign key has `#[unique]` instead of `#[index]`, which guarantees
that only one profile can reference a given user. In the database:

```sql
CREATE TABLE profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER,
    bio TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_profiles_user_id ON profiles (user_id);
```

## Optional vs required HasOne

The type parameter on `HasOne` controls whether the parent must have a child.

### Optional: `HasOne<Option<Profile>>`

The parent may or may not have a child. Creating a parent without a child is
allowed:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_one]
#     profile: toasty::HasOne<Option<Profile>>,
# }
# #[derive(Debug, toasty::Model)]
# struct Profile {
#     #[key]
#     #[auto]
#     id: u64,
#     #[unique]
#     user_id: Option<u64>,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::BelongsTo<Option<User>>,
#     bio: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// A user without a profile — this is fine
let user = User::create().name("Alice").exec(&mut db).await?;

assert!(user.profile().get(&mut db).await?.is_none());
# Ok(())
# }
```

### Required: `HasOne<Profile>`

The parent must have a child. Creating a parent requires providing a child:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     #[has_one]
#     profile: toasty::HasOne<Profile>,
# }
# #[derive(Debug, toasty::Model)]
# struct Profile {
#     #[key]
#     #[auto]
#     id: u64,
#     #[unique]
#     user_id: Option<u64>,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::BelongsTo<Option<User>>,
#     bio: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Must provide a profile when creating the user
let user = User::create()
    .profile(Profile::create().bio("Hello"))
    .exec(&mut db)
    .await?;

let profile = user.profile().get(&mut db).await?;
assert_eq!(profile.bio, "Hello");
# Ok(())
# }
```

## Accessing the related record

Call the relation method on the parent instance to load the child:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_one]
#     profile: toasty::HasOne<Option<Profile>>,
# }
# #[derive(Debug, toasty::Model)]
# struct Profile {
#     #[key]
#     #[auto]
#     id: u64,
#     #[unique]
#     user_id: Option<u64>,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::BelongsTo<Option<User>>,
#     bio: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let user = User::create()
#     .name("Alice")
#     .profile(Profile::create().bio("A person"))
#     .exec(&mut db)
#     .await?;
// For HasOne<Option<Profile>> — returns Option<Profile>
let profile = user.profile().get(&mut db).await?;

if let Some(profile) = profile {
    println!("Bio: {}", profile.bio);
}
# Ok(())
# }
```

For a required `HasOne<Profile>`, `.get()` returns `Profile` directly (not
wrapped in `Option`).

Each call to `.profile().get()` executes a database query. To avoid this, use
[preloading](./preloading-associations.md).

## Creating through the relation

Create a child for an existing parent through the relation accessor:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_one]
#     profile: toasty::HasOne<Option<Profile>>,
# }
# #[derive(Debug, toasty::Model)]
# struct Profile {
#     #[key]
#     #[auto]
#     id: u64,
#     #[unique]
#     user_id: Option<u64>,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::BelongsTo<Option<User>>,
#     bio: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let user = User::create().name("Alice").exec(&mut db).await?;

let profile = user
    .profile()
    .create()
    .bio("A person")
    .exec(&mut db)
    .await?;

assert_eq!(profile.user_id, Some(user.id));
# Ok(())
# }
```

Or create the parent and child together:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_one]
#     profile: toasty::HasOne<Option<Profile>>,
# }
# #[derive(Debug, toasty::Model)]
# struct Profile {
#     #[key]
#     #[auto]
#     id: u64,
#     #[unique]
#     user_id: Option<u64>,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::BelongsTo<Option<User>>,
#     bio: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let user = User::create()
    .name("Alice")
    .profile(Profile::create().bio("A person"))
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

## Updating the relation

### Replacing with a new child

Create a new child and associate it with the parent in an update:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[has_one]
#     profile: toasty::HasOne<Option<Profile>>,
# }
# #[derive(Debug, toasty::Model)]
# struct Profile {
#     #[key]
#     #[auto]
#     id: u64,
#     #[unique]
#     user_id: Option<u64>,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::BelongsTo<Option<User>>,
#     bio: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
# let mut user = User::create()
#     .name("Alice")
#     .profile(Profile::create().bio("Old bio"))
#     .exec(&mut db)
#     .await?;
user.update()
    .profile(Profile::create().bio("New bio"))
    .exec(&mut db)
    .await?;

let profile = user.profile().get(&mut db).await?.unwrap();
assert_eq!(profile.bio, "New bio");
# Ok(())
# }
```

### Associating an existing child

Pass a reference to an existing child record:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     #[has_one]
#     profile: toasty::HasOne<Option<Profile>>,
# }
# #[derive(Debug, toasty::Model)]
# struct Profile {
#     #[key]
#     #[auto]
#     id: u64,
#     #[unique]
#     user_id: Option<u64>,
#     #[belongs_to(key = user_id, references = id)]
#     user: toasty::BelongsTo<Option<User>>,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let user = User::create().exec(&mut db).await?;
let profile = Profile::create().exec(&mut db).await?;

User::filter_by_id(user.id)
    .update()
    .profile(&profile)
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

### Unsetting the relation

For an optional HasOne, pass `None` to disassociate the child:

```rust,ignore
user.update().profile(None).exec(&mut db).await?;

// The profile no longer belongs to the user
assert!(user.profile().get(&mut db).await?.is_none());
```

What happens to the child when you unset the relation depends on the child's
foreign key:

| Child's foreign key type | Effect of unsetting |
|---|---|
| Required (`user_id: u64`) | Deletes the child record |
| Optional (`user_id: Option<u64>`) | Sets the foreign key to `NULL` |

## Deleting behavior

When you delete a parent, the behavior depends on the child's foreign key type:

- **Required foreign key** (`user_id: u64`): Toasty deletes the child record,
  since it cannot exist without a parent.
- **Optional foreign key** (`user_id: Option<u64>`): Toasty sets the foreign key
  to `NULL`, leaving the child record in place.

## What gets generated

For a `User` model with `#[has_one] profile: HasOne<Option<Profile>>`, Toasty
generates:

| Method | Returns | Description |
|---|---|---|
| `user.profile()` | Relation accessor | Accessor for the associated profile |
| `.get(&mut db)` | `Result<Option<Profile>>` | Load the associated profile |
| `.create()` | Create builder | Create a profile with the foreign key pre-filled |
| `User::create().profile(...)` | Create builder | Associate a profile on creation |
| `user.update().profile(...)` | Update builder | Replace or associate a profile |
| `user.update().profile(None)` | Update builder | Disassociate the profile |
| `User::fields().profile()` | Field path | Used with `.include()` for preloading |
