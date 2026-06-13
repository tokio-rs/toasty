# Model comments

## Summary

Toasty accepts `#[comment("...")]` on models and scalar fields. SQL schema
migration stores those strings as database-native table and column comments on
PostgreSQL and MySQL. SQLite and Turso keep the comments in Toasty's schema but
do not emit database DDL because those databases do not have native table or
column comments.

## Motivation

Teams often use database comments as the source of truth for schema catalogs,
SQL consoles, and generated data dictionaries. Without model comments, Toasty
users must add comments through separate migrations, which can drift from the
Rust model definition.

## User-facing API

Add `#[comment("text")]` to a root model to describe the table:

```rust
#[derive(Debug, toasty::Model)]
#[comment("accounts that can sign in")]
struct User {
    #[key]
    #[auto]
    id: u64,

    #[comment("public display name")]
    name: String,
}
```

The `#[comment = "text"]` form is equivalent:

```rust
#[derive(Debug, toasty::Model)]
#[comment = "accounts that can sign in"]
struct User {
    #[key]
    #[auto]
    id: u64,

    #[comment = "public display name"]
    name: String,
}
```

Comments do not change the Rust API. Fields keep the same getters, setters,
queries, and serialization behavior. Comments only affect schema metadata and
SQL DDL on databases that have native comments.

## Behavior

PostgreSQL emits standalone comment statements after creating the table:

```sql
COMMENT ON TABLE "users" IS 'accounts that can sign in';
COMMENT ON COLUMN "users"."name" IS 'public display name';
```

Changing or removing a PostgreSQL comment emits another `COMMENT ON` statement.
A removed comment is represented as `IS NULL`.

MySQL emits table and column comments inline when creating a table:

```sql
CREATE TABLE `users` (
    `id` BIGINT NOT NULL,
    `name` TEXT NOT NULL COMMENT 'public display name',
    PRIMARY KEY (`id`)
) COMMENT='accounts that can sign in';
```

Changing a MySQL table comment emits `ALTER TABLE ... COMMENT = ...`. Changing a
MySQL column comment rebuilds the column definition with `CHANGE COLUMN`, which
is how MySQL changes column metadata.

SQLite and Turso keep comments in `Db::schema()` and migration snapshots, but
schema creation and schema migration emit no comment SQL.

Duplicate `#[comment]` attributes on the same model or field are compile-time
macro errors. Non-string comment values are compile-time macro errors.

## Edge cases

Empty strings are accepted. On MySQL, clearing a table comment emits an empty
string comment because MySQL does not use `NULL` for table comments.

Embedded fields can carry comments. When an embedded field maps to a database
column, the generated column receives that comment.

Comments are schema metadata only. Toasty does not expose query predicates or
runtime behavior based on comments.

## Driver integration

The `Driver` trait does not change. Drivers continue to receive schema mutation
operations through existing migration paths.

SQL drivers that use `toasty-sql` get serialization for PostgreSQL and MySQL.
Drivers for databases without native comments may ignore the metadata. Out-of-tree drivers that construct `toasty_core::schema::db::Table` or `Column` values
must initialize the new `comment` fields.

## Alternatives considered

Use `#[table(comment = "...")]` and `#[column(comment = "...")]`. This keeps
all database mapping metadata under existing attributes, but it makes comments
harder to scan and splits one concept across two syntaxes. A dedicated
`#[comment]` attribute works the same at the table and field levels.

Store comments only in generated SQL migrations. That would avoid schema field
additions, but comments would not survive schema snapshots or programmatic
schema inspection.

## Open questions

None for this MVP.

## Out of scope

- JSON comments, index comments, and enum type comments. The MVP only covers
  tables and columns.
- Databases without native comments. Toasty stores metadata but does not emulate
  comments with side tables or SQL text comments.
