# Toasty User Guide Plan

## Research Summary

Key patterns:

1. **Consistent example domain** — use the same models across all pages so readers never re-orient
2. **Progressive derive macro introduction** — introduce attributes one at a time, when they're needed
3. **"What gets generated" sections** — since `#[derive(Model)]` generates query builders, show users the API they get
4. **One concept per page** — focused examples, no concept bleed
5. **Separate "define" from "use"** — teach model definition, then teach how to query it
6. **SQL alongside ORM code** — where helpful, show what SQL Toasty generates

## Example Domain

Use a **blog application** throughout (User, Post, Comment, Profile). This domain is:
- Familiar to all developers
- Exercises all relationship types (HasMany, BelongsTo, HasOne)
- Has natural opportunities for indexes, unique constraints, embedded types
- Familiar to developers coming from other ORMs

Each chapter introduces models incrementally. Chapter 1 starts with just `User`. Later chapters add `Post`, `Comment`, `Profile` as needed.

## Chapter Outline

### Part 1: Foundations

#### 1. Introduction
- What Toasty is (async Rust ORM, SQL + NoSQL)
- How it works: derive macro generates query builders at compile time
- Supported databases (SQLite, PostgreSQL, MySQL, DynamoDB)
- What this guide covers

#### 2. Getting Started
- Add toasty to Cargo.toml (plus tokio, uuid)
- Define a single `User` model with `#[derive(Model)]`
- Connect to SQLite in-memory
- Create a record, query it back
- Full working `main.rs` — copy-paste-run
- **Key teaching moment**: "When you write `#[derive(Model)]`, Toasty generates methods like `User::create()`, `User::get_by_id()`, and `User::all()`. The rest of this guide shows you everything that gets generated."

#### 3. Defining Models
- The `#[derive(Model)]` macro
- Struct fields become database columns
- Supported field types: String, i64, bool, Option<T>, uuid::Uuid, etc.
- Table naming (auto-pluralized, or `#[table = "custom"]`)
- Column naming (auto snake_case, or `#[column("custom")]`)
- **What gets generated**: the Model trait impl, field accessors, basic query methods

#### 4. Keys and Auto-Generation
- `#[key]` marks the primary key
- `#[auto]` auto-generates values: `uuid(v4)`, `uuid(v7)`, `increment`
- `#[auto]` with no argument on UUID fields defaults to v4
- Composite keys: `#[key(partition = field1, local = field2)]`
- **What gets generated**: `get_by_id()`, `get_by_pk()` methods

### Part 2: CRUD Operations

#### 5. Creating Records
- `Model::create().field(value).exec(&mut db).await?`
- Setting fields on the create builder
- Creating with default/auto values (fields you don't set)
- Nested creation: creating a User with Todos in one call
- `Model::create_many()` for bulk inserts
- **What gets generated**: the `UserCreate` builder struct with a method per field

#### 6. Querying Records
- `Model::get_by_id(&mut db, &id)` — get one by primary key
- `Model::all(&mut db)` — get a cursor over all records
- Cursors: `while let Some(item) = cursor.next().await { ... }`
- `.collect::<Vec<_>>(&mut db)` — collect to a Vec
- `.first(&mut db)` — get first or None
- `.get(&mut db)` — get exactly one (error if not found)
- **What gets generated**: the `UserQuery` struct

#### 7. Updating Records
- `model.update().field(new_value).exec(&mut db).await?`
- The update builder pattern
- Updating via query: `Model::update_by_field(value)`
- **What gets generated**: the `UserUpdate` builder struct

#### 8. Deleting Records
- `model.delete().exec(&mut db).await?`
- Deleting by query: `Model::delete_by_field(&mut db, value)`
- Deleting via query builder: `.delete()`

### Part 3: Schema Features

#### 9. Indexes and Unique Constraints
- `#[unique]` on a field
- `#[index]` on a field
- **What gets generated**: `get_by_email()`, `filter_by_email()`, `update_by_email()`, `delete_by_email()` (for each indexed/unique field)
- Difference between `#[unique]` and `#[index]`: unique generates `get_by_*` (returns one), index generates `filter_by_*` (returns many)

#### 10. Field Options
- `#[column("name")]` — custom column name
- `#[column(type = ...)]` — explicit column type (varchar, int, timestamp, etc.)
- `#[default(expr)]` — default value expression on create
- `#[update(expr)]` — expression applied on create and update
- `#[serialize(json)]` — store complex types as JSON
- Timestamps: `created_at` and `updated_at` auto-behavior

### Part 4: Relationships

#### 11. Relationships: BelongsTo
- Add a foreign key field + `#[belongs_to]` relation field
- `#[belongs_to(key = user_id, references = id)]`
- The `BelongsTo<User>` type
- Accessing the related record: `post.user().get(&mut db).await?`
- Setting on create: `Post::create().user(&user)`
- **What gets generated**: relation accessor, `.get()` method

#### 12. Relationships: HasMany
- `#[has_many]` on the parent model
- The `HasMany<Post>` type
- Querying children: `user.posts().all(&mut db).await?`
- Creating through the relation: `user.posts().create().title("...").exec(&mut db).await?`
- Linking/unlinking: `.insert()` and `.remove()`
- Scoped queries: `user.posts().query(filter)`
- **What gets generated**: `Many` struct with `.all()`, `.create()`, `.insert()`, `.remove()`, `.query()`, `.collect()`

#### 13. Relationships: HasOne
- `#[has_one]` for single-child relations
- `HasOne<Profile>` (required) vs `HasOne<Option<Profile>>` (optional)
- Accessing: `user.profile().get(&mut db).await?`
- Creating: `user.profile().create().bio("...").exec(&mut db).await?`

#### 14. Preloading Associations
- The N+1 problem: why preloading matters
- `.include(Model::fields().relation())` on queries
- Preloading HasMany, BelongsTo, HasOne
- Multiple includes in one query
- Accessing preloaded data without additional queries

### Part 5: Advanced Queries

#### 15. Filtering with Expressions
- `Model::fields().field_name()` — field accessors for expressions
- Comparisons: `.eq()`, `.ne()`, `.gt()`, `.gte()`, `.lt()`, `.lte()`
- `.in_list([...])` — IN clause
- `.like("pattern")` — LIKE
- `.is_null()`, `.is_not_null()`
- Combining: `.and()`, `.or()`
- Using with `.filter()`: `User::filter(User::fields().name().eq("Alice"))`

#### 16. Sorting, Limits, and Pagination
- `.order_by(expr)` — sort results
- `.limit(n)` and `.offset(n)`
- Cursor-based pagination: `.paginate(per_page)`
- Page struct: items, next_cursor, has_next

### Part 6: Advanced Features

#### 17. Embedded Types
- `#[derive(Embed)]` for structs — inline fields in parent table
- `#[derive(Embed)]` for enums — unit and data variants
- `#[column(variant = N)]` for enum discriminants
- Filtering on embedded fields
- Indexing embedded fields

#### 18. Batch Operations
- `toasty::batch((query1, query2))` — multiple queries in one round-trip
- Batch create with `create_many()`
- Batch filter: `filter_by_field_batch([values])`

#### 19. Transactions
- `db.transaction().await?` — begin a transaction
- Interactive transactions: run queries inside a transaction
- Commit and rollback
- Auto-rollback on drop

#### 20. Database Setup
- Connection URLs for each database
- SQLite: `sqlite::memory:`, `sqlite:./path.db`
- PostgreSQL: `postgresql://user:pass@host/db`
- MySQL: `mysql://user:pass@host/db`
- DynamoDB: `dynamodb+region://...`
- `db.push_schema()` — create tables from models
- `db.reset_db()` — drop and recreate
- Registering models: `Db::builder().register::<Model>()`

## Writing Conventions

Per the write-docs skill:
- Fact-focused, no buzzwords or fluff
- Active voice, present tense
- Concrete examples over explanation
- Every sentence conveys information
- Start with what the thing is, then show how to use it

## Implementation Order

Write chapters in order (1-20). Each chapter is a separate `.md` file in `docs/guide/src/`. Update `SUMMARY.md` as chapters are added.
