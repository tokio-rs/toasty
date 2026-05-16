# Indexes and Unique Constraints

Toasty supports two field-level attributes for indexing: `#[unique]` and
`#[index]`. Both create database indexes, but they differ in what gets
generated.

## Unique fields

Add `#[unique]` to a field to create a unique index. Toasty enforces uniqueness
on all supported databases. SQL databases (SQLite, PostgreSQL, MySQL) use a
native unique index. DynamoDB uses a separate index table keyed on the unique
attribute; inserts and updates write to both tables in a single
`TransactWriteItems` call with an `attribute_not_exists` condition that rejects
duplicates.

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[unique]
    email: String,
}
```

This generates a unique index on the `email` column:

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_users_email ON users (email);
```

Attempting to insert a duplicate value returns an error:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[unique]
#     email: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
toasty::create!(User {
    name: "Alice",
    email: "alice@example.com",
})
.exec(&mut db)
.await?;

// This fails — email must be unique
let result = toasty::create!(User {
    name: "Bob",
    email: "alice@example.com",
})
.exec(&mut db)
.await;

assert!(result.is_err());
# Ok(())
# }
```

### Generated methods for unique fields

Because a unique field identifies at most one record, Toasty generates a
`get_by_*` method that returns a single record:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[unique]
#     email: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Get a single user by email (errors if not found)
let user = User::get_by_email(&mut db, "alice@example.com").await?;
# Ok(())
# }
```

Toasty also generates `filter_by_*`, `update_by_*`, and `delete_by_*` methods:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[unique]
#     email: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// Filter — returns a query builder
let user = User::filter_by_email("alice@example.com")
    .get(&mut db)
    .await?;

// Update by email
User::update_by_email("alice@example.com")
    .name("Alice Smith")
    .exec(&mut db)
    .await?;

// Delete by email
User::delete_by_email(&mut db, "alice@example.com").await?;
# Ok(())
# }
```

## Indexed fields

Add `#[index]` to a field to tell Toasty that this field is a query target. On
SQL databases, Toasty creates a database index on the column, which lets the
database find matching rows without scanning the entire table. On DynamoDB, the
attribute maps to a secondary index.

Unlike `#[unique]`, `#[index]` does not enforce uniqueness — multiple records
can share the same value.

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[index]
    country: String,
}
```

This generates a non-unique index:

```sql
CREATE INDEX idx_users_country ON users (country);
```

### Generated methods for indexed fields

Because an indexed field may match multiple records, the generated methods work
with collections rather than single records:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[index]
#     country: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// filter_by_country returns a query builder (may match many records)
let users = User::filter_by_country("US")
    .exec(&mut db)
    .await?;

// Update all records matching the index
User::update_by_country("US")
    .country("United States")
    .exec(&mut db)
    .await?;

// Delete all records matching the index
User::delete_by_country(&mut db, "US").await?;
# Ok(())
# }
```

Toasty also generates a `get_by_*` method for indexed fields. It returns the
matching record directly, but errors if no record matches or if more than one
record matches:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# struct User {
#     #[key]
#     #[auto]
#     id: u64,
#     name: String,
#     #[index]
#     country: String,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
let user = User::get_by_country(&mut db, "US").await?;
# Ok(())
# }
```

## Multi-column indexes

Struct-level `#[index]` lets you define a composite index spanning multiple
fields. This is useful when you frequently query by a combination of fields
rather than a single one.

### Simple mode

List the fields in order — the first field is the leading key, and the remaining
fields extend it:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
#[index(game_title, top_score)]
struct GameScore {
    #[key]
    #[auto]
    id: u64,
    user_id: String,
    game_title: String,
    top_score: i64,
}
```

On SQL databases this creates a composite index with columns in the order
specified:

```sql
CREATE INDEX idx_game_scores_game_title_top_score
    ON game_scores (game_title, top_score);
```

On DynamoDB, the first field becomes the HASH key and the remaining fields
become RANGE keys of a Global Secondary Index (GSI).

Toasty generates a method for each valid prefix of the index fields:

| Method | Description |
|---|---|
| `GameScore::filter_by_game_title(game_title)` | All scores for a game |
| `GameScore::filter_by_game_title_and_top_score(game_title, top_score)` | Scores for a game with a specific score |

You can use these the same way as single-column index methods:

```rust
# use toasty::Model;
# #[derive(Debug, toasty::Model)]
# #[index(game_title, top_score)]
# struct GameScore {
#     #[key]
#     #[auto]
#     id: u64,
#     user_id: String,
#     game_title: String,
#     top_score: i64,
# }
# async fn __example(mut db: toasty::Db) -> toasty::Result<()> {
// All scores for "chess"
let scores: Vec<GameScore> = GameScore::filter_by_game_title("chess")
    .exec(&mut db)
    .await?;

// Scores for "chess" with a top score of exactly 1400
let scores: Vec<GameScore> = GameScore::filter_by_game_title_and_top_score("chess", 1400)
    .exec(&mut db)
    .await?;
# Ok(())
# }
```

For a three-column index, Toasty generates three prefix methods. Given
`#[index(country, city, zip_code)]`:

| Method | Columns matched |
|---|---|
| `filter_by_country(country)` | `country` |
| `filter_by_country_and_city(country, city)` | `country`, `city` |
| `filter_by_country_and_city_and_zip_code(country, city, zip_code)` | `country`, `city`, `zip_code` |

### Named mode

Use `partition = ...` and `local = ...` to explicitly assign fields to key
roles. This is required when you need multiple fields in the DynamoDB partition
key:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
#[index(partition = [tournament_id, region], local = [round])]
struct Match {
    #[key]
    #[auto]
    id: u64,
    tournament_id: String,
    region: String,
    round: String,
    player1_id: String,
    player2_id: String,
}
```

On DynamoDB, `partition` fields map to `KeyType::Hash` entries and `local`
fields map to `KeyType::Range` entries in the GSI KeySchema. This allows the
DynamoDB index to carry a composite identifier — here, a tournament is uniquely
identified by both `tournament_id` and `region`.

The generated methods require all partition fields:

| Method | Description |
|---|---|
| `Match::filter_by_tournament_id_and_region(tournament_id, region)` | All rounds for a tournament+region |
| `Match::filter_by_tournament_id_and_region_and_round(tournament_id, region, round)` | A specific round |

On SQL databases, the `partition`/`local` distinction is ignored — all fields
are placed in the composite index in the order they appear, producing
`CREATE INDEX ... ON matches (tournament_id, region, round)`.

### Custom index names

Toasty generates an index name from the table and field list (e.g.,
`idx_users_email`). Override it with `name = "..."` inside `#[index(...)]`
or `#[key(...)]`:

```rust
# use toasty::Model;
#[derive(Debug, toasty::Model)]
#[index(name = "scores_by_game", game_title, top_score)]
struct GameScore {
    #[key]
    #[auto]
    id: u64,
    user_id: String,
    game_title: String,
    top_score: i64,
}
```

This becomes `CREATE INDEX scores_by_game ON game_scores (...)` on SQL
drivers and is used as the GSI name on DynamoDB. The name must be
non-empty and may only appear once per attribute. Use a custom name
when a migration tool or external query references it by name, or to
keep generated names within a database's identifier-length limit.

### SQL vs DynamoDB behavior

| Behavior | SQL | DynamoDB |
|---|---|---|
| Index structure | `CREATE INDEX` with all columns in order | GSI with HASH and RANGE key entries |
| Partition / local distinction | Ignored — all columns form a flat composite index | `partition` = `KeyType::Hash`, `local` = `KeyType::Range` |
| Query matching | Database uses leftmost-prefix matching | All `partition` fields required; `local` fields optional left-to-right |
| Column limits | No artificial limits | Up to 4 partition and 4 local attributes per index |

## Indexing newtype fields

Newtype embedded structs (single unnamed field, e.g., `struct Email(String)`)
support `#[unique]` and `#[index]` on the model field. The newtype maps to a
single column, so the index works the same as on a primitive:

```rust,ignore
#[derive(Debug, toasty::Embed)]
struct Email(String);

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,

    name: String,

    #[unique]
    email: Email,
}
```

This generates `User::get_by_email()`, `User::filter_by_email()`, and the other
index methods. The argument type is the newtype itself:

```rust,ignore
let user = User::get_by_email(&mut db, Email("alice@example.com".into())).await?;
```

Multi-field embedded structs do not support `#[unique]` or `#[index]` on the
parent field because the column ordering within the index is ambiguous. Index
individual fields inside the embedded struct instead (see
[Embedded Types — Indexing embedded fields](./embedded-types.md#indexing-embedded-fields)).

## Choosing between `#[unique]` and `#[index]`

Both attributes tell Toasty that a field is a query target and generate the same
set of methods: `get_by_*`, `filter_by_*`, `update_by_*`, and `delete_by_*`.

The difference is in the constraint they express:

| Attribute | Meaning | Database effect (SQL) |
|---|---|---|
| `#[unique]` | Each record has a distinct value | `CREATE UNIQUE INDEX` — the database rejects duplicates |
| `#[index]` | Multiple records may share a value | `CREATE INDEX` — no uniqueness enforcement |

Use `#[unique]` for fields that identify a single record — email addresses,
usernames, slugs. Use `#[index]` for fields you query frequently but that
naturally repeat — country, status, category.

## What gets generated

For a model with `#[unique]` on `email` and `#[index]` on `country`:

| Method | Description |
|---|---|
| `User::get_by_email(&mut db, email)` | One record by unique field |
| `User::filter_by_email(email)` | Query builder for unique field |
| `User::update_by_email(email)` | Update builder for unique field |
| `User::delete_by_email(&mut db, email)` | Delete by unique field |
| `User::get_by_country(&mut db, country)` | One record by indexed field |
| `User::filter_by_country(country)` | Query builder for indexed field |
| `User::update_by_country(country)` | Update builder for indexed field |
| `User::delete_by_country(&mut db, country)` | Delete by indexed field |

These methods follow the same patterns as key-generated methods. See
[Querying Records](./querying-records.md),
[Updating Records](./updating-records.md), and
[Deleting Records](./deleting-records.md) for details on terminal methods and
builders.
