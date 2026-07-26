# Database Comments for Models and Fields

## Summary

Models and stored fields gain a `#[comment = "..."]` attribute for database
table and column comments. Migration generation applies these comments only
when `schema_comments` is enabled. PostgreSQL and MySQL use their native
catalog comments. `db.push_schema()` also creates supported comments, and
`migration reset` restores comments recorded in migration history. Backends
without native table or column comments ignore the attributes and do not
emulate them in Toasty-owned storage. Create, alter, rename, and table
reconstruction paths preserve the effective comments of every surviving
schema object.

## Motivation

Database operators often inspect a schema without the Rust source available.
Table and column comments provide the domain meaning, ownership, units, and
data-handling rules needed to understand that schema. Toasty users currently
have to edit generated migrations by hand to add this metadata. Those edits
are not represented in schema snapshots, so later model changes cannot update
or remove the comments consistently.

Comments also need to be optional. Teams may not want application descriptions
in a production catalog, and SQLite has no native table or column comment DDL.
The migration configuration therefore decides whether Toasty manages declared
comments. The model remains portable when comment management is disabled.

## User-facing API

### Declaring table and column comments

Place `#[comment = "..."]` on a model to describe its table. Place the same
attribute on a stored field to describe its column:

```rust
#[derive(toasty::Model)]
#[comment = "Accounts that can sign in to the service"]
struct User {
    #[key]
    #[auto]
    #[comment = "Stable account identifier"]
    id: uuid::Uuid,

    #[unique]
    #[comment = "Normalized address used for sign-in"]
    email: String,
}
```

The argument is a non-empty Rust string literal. Raw string literals and
multi-line literals are accepted. A comment is database metadata, not a SQL
`--` comment, and does not change generated Rust documentation.

A field comment must resolve to exactly one physical column. Primitive fields
meet this rule. A document field and a transparent newtype embed also meet it
because each has one column. A relation maps to no column, while a flattened
multi-field embed maps to several, so placing `#[comment]` on either is an
error.

Leaf fields declared by an embedded type may carry comments. Toasty applies
the comment to each physical column created from that leaf:

```rust
#[derive(toasty::Embed)]
struct Address {
    street: String,

    #[comment = "Postal city as supplied by the user"]
    city: String,
}

#[derive(toasty::Model)]
struct Customer {
    #[key]
    id: uuid::Uuid,
    billing_address: Address,
}
```

Here the comment belongs to the `billing_address_city` column. Commenting
`billing_address` itself is rejected because `Address` expands to more than
one column. `#[comment]` is not accepted on an `Embed` type itself because an
embedded type does not own a table.

### Enabling comments in migrations

Comments are not included in migrations by default. Enable them in
`Toasty.toml` when the connected database should contain the declared
metadata:

```toml
[migration]
schema_comments = true
```

The equivalent programmatic configuration is:

```rust
let migration = toasty_cli::MigrationConfig::new()
    .schema_comments(true);
```

`schema_comments` defaults to `false`. The configuration field also uses a
serde default, so an existing `Toasty.toml` without this key keeps loading with
comment management disabled.

Keep this setting stable for a migration history. Enabling it after earlier
migrations were generated is supported: the next generated migration fills
all declared comments that are absent from the latest managed snapshot.

## Behavior

With `schema_comments = true`, migration snapshots contain the table and
column comments supported by the connected driver. The schema diff treats
adding, changing, or removing a supported comment as a schema change. A
comment-only change therefore produces a migration and a new snapshot.
Removing the attribute removes the database comment.

If the driver does not support table comments, column comments, or both,
migration generation removes the unsupported values from both sides of the
comparison and from the generated snapshot. Those attributes do not produce
DDL and do not cause an error. A driver that supports only one kind still
manages the supported kind.

With `schema_comments = false`, migration generation ignores new, changed, and
removed comment declarations. For a table or column already present in the
previous snapshot, the next snapshot carries its previous managed comment
forward. New tables and columns receive no comment. This prevents an unrelated
alteration or table rebuild from erasing a comment that Toasty applied earlier,
without turning a declaration change into a comment-only migration. If comment
management is enabled again, Toasty compares the declarations with the carried
state and emits the required changes.

Comment statements follow the table or column DDL they describe. Rename
handling targets the new table or column name. Generated SQL quotes comment
text as a dialect string literal; quotes in the Rust string do not become SQL
syntax.

### Creation, alteration, and reconstruction

Comments are part of the final physical schema, not an optional cleanup step
after ordinary DDL. Every operation that materializes or replaces a supported
table or column uses the effective next-schema comment:

- Creating a table applies its table comment and every column comment.
- Adding a column applies the new column's comment.
- Altering a column preserves its unchanged comment when the dialect restates
  the full column definition, and applies a changed or removed comment.
- Renaming a table or column preserves its existing comment and applies new
  text against the new name when the declaration also changed.
- Reconstructing a table through a temporary table reapplies the table comment
  and all comments on surviving and added columns so the final table contains
  them. This includes unchanged comments because replacement catalog objects
  cannot rely on metadata from the dropped table.
- Replaying migrations during `migration reset` produces the same comments as
  applying that migration history to a new database.

The effective next schema contains current declarations when
`schema_comments = true`, carried previous comments when it is `false`, and no
comments for a capability the driver does not support. DDL generation consults
that schema during create, alter, and reconstruction operations; it cannot rely
only on comment-specific diff entries.

The built-in drivers behave as follows:

| Driver | Table comments | Column comments | Enabled migration behavior |
|---|---|---|---|
| PostgreSQL | Native | Native | Emits `COMMENT ON TABLE` and `COMMENT ON COLUMN` |
| MySQL | Native | Native | Emits the corresponding table or column `COMMENT` DDL |
| SQLite | Unsupported | Unsupported | Ignores declared comments |
| Turso | Unsupported | Unsupported | Ignores declared comments |
| DynamoDB | Unsupported | Unsupported | Ignores comments; schema migrations remain unsupported |

### Pushing and resetting schemas

`db.push_schema()` applies every supported model and field comment as part of
creating the corresponding table or column. It does not read
`MigrationConfig`; `schema_comments` controls generated migrations only.
SQLite, Turso, and any other driver without native comment support create the
schema normally and ignore the comments.

`migration reset` drops the database and re-applies the migration history, so
it restores every comment statement recorded in that history. Changing
`schema_comments` does not rewrite old migration files. A project that enabled
comments after its earlier migrations must generate the resulting comment
migration before `migration reset` can restore those comments.

`migration reset --skip-migrations` creates no tables and therefore no
comments. The lower-level `Db::reset_db()` likewise leaves an empty database;
calling `db.push_schema()` afterward recreates tables and their supported
comments from the current model schema.

Invalid attribute placement and invalid literals fail during model schema
construction. Database-specific limits, such as a maximum comment length,
surface as driver errors when comment DDL is executed. Toasty does not apply
one backend's length limit to other backends.

## Edge cases

An empty or whitespace-only string is rejected. Deleting the attribute is the
portable way to remove a comment; an empty string does not have consistent
catalog semantics across PostgreSQL and MySQL. Strings containing a NUL byte
are also rejected because the supported SQL catalogs cannot store them
portably.

When the same embedded type appears more than once in a model, each resulting
leaf column receives the embedded leaf's comment. The text is copied into the
database schema; it is not linked to one shared catalog object.

Enum variant fields that share one physical column through `#[shared(...)]`
must either declare the same comment or leave it to another member of the
shared group. Conflicting comments are a schema-construction error. A comment
on a data-carrying enum field is rejected because that field maps to a
discriminant and one or more data columns. A unit-only enum field may carry a
comment because it maps only to its discriminant column.

Changing `schema_comments` from `true` to `false` does not itself create a
migration. If another schema change creates a snapshot while comments are
disabled, that snapshot carries comments already managed by the preceding
snapshot but ignores declaration changes. Rename hints carry the comment from
the previous logical table or column to its new name.

## Driver integration

`Capability` reports native table-comment and column-comment support
separately. PostgreSQL and MySQL report both. SQLite, Turso, and DynamoDB report
neither. An out-of-tree driver must set both values and handle every comment
change it claims to support. Migration generation filters unsupported comment
metadata before calling the driver.

The database schema exposed to drivers includes an optional comment on each
table and column. These fields use serde defaults and omit absent values, so
existing snapshots remain readable without a snapshot format-version change.
Migration generation builds the effective next schema described above before
computing the diff and before asking the driver to serialize any DDL.

A driver that supports comments must handle all four cases: a comment on a
new object, adding a comment to an existing object, replacing its text, and
removing it. PostgreSQL removes comments with `IS NULL`. MySQL represents
removal with an empty native comment and must restate a column definition when
its dialect requires `MODIFY COLUMN`. That full definition includes the
effective comment even when an unrelated property triggered the alteration.
The driver must also preserve the column's type, nullability, default, and
auto-increment properties.

Table reconstruction is a full comment restore, not just a sequence of column
diffs. The driver reads the final table and all its columns from the effective
next schema. It may attach comments to the temporary objects when the dialect
guarantees that rename preserves them; otherwise it applies them after the
final table name is in place.

The same capability governs `push_schema()`. A supporting driver applies the
table and column comments before `push_schema()` returns, either in the create
statement or with subsequent native DDL. A comment failure makes
`push_schema()` fail rather than returning with a partially described schema.
`migration reset` needs no separate driver operation because it executes the
generated migration SQL.

Comment DDL is emitted as separate migration statements when the dialect
cannot attach it to the create statement. Drivers that require one statement
per execution receive the existing migration breakpoint markers between these
statements.

Adding comment capability fields is a source change for out-of-tree drivers
that construct `Capability` directly. Reporting a capability as unsupported
preserves existing behavior and causes migration generation to ignore the
corresponding comment attributes.

## Alternatives considered

### Reusing Rust documentation comments

Toasty could copy `///` text into the database. Rust documentation often
contains Markdown, links, examples, and implementation details that do not
belong in a database catalog. An explicit attribute keeps the two audiences
separate and prevents documentation edits from creating migrations.

### Always managing declared comments in migrations

Always including supported comments in migration diffs would create catalog
metadata and comment-only migrations for every project. The migration setting
makes that ownership explicit and defaults to the current behavior.
`push_schema()` is intentionally different: it creates the current model
schema directly and has no migration history or migration configuration.

### Erroring on unsupported comments

Toasty could return `Error::unsupported_feature` when comment management is
enabled for SQLite, Turso, or another backend without native catalog comments.
Comments are descriptive metadata and should not block an otherwise valid
structural migration. Capability-based filtering keeps the backend difference
explicit in the documentation without requiring users to remove portable
model attributes.

### A per-command `--comments` flag

A one-off flag makes successive snapshots depend on whether the caller
remembered the option. A persistent migration setting gives the history one
policy and still supports changing that policy deliberately.

### Storing comments in a Toasty metadata table

An auxiliary table could emulate comments on SQLite, but database inspection
tools would not show that metadata as table or column comments. This design
uses only each backend's native catalog feature.

## Open questions

None block acceptance. The attribute syntax, default-disabled migration
policy, capability-based ignore behavior, and single-column mapping rule are
part of this design.

## Out of scope

- Comments on indexes, constraints, enum variants, and named database types:
  the requested model and field attributes cover only tables and columns.
- Importing comments from an existing database: Toasty compares model-derived
  snapshots and does not introspect catalog text.
- Emulating catalog comments on SQLite, Turso, or DynamoDB: non-native storage
  would not be visible through standard schema inspection.
- Localized comments: each model or field has one database comment string.
