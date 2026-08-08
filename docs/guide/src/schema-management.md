# Migrations and Schema Management

Toasty provides two ways to manage your database schema: `push_schema` for
quick development, and a migration system for production databases.

## Quick setup with `push_schema`

`db.push_schema()` creates all tables and indexes based on your registered
models. It issues `CREATE TABLE` and `CREATE INDEX` statements directly against
the database.

```rust,ignore
let mut db = toasty::Db::builder()
    .models(toasty::models!(crate::*))
    .connect("sqlite::memory:")
    .await?;

db.push_schema().await?;
```

This works well for prototyping and tests. It does not track what has changed
between runs — it pushes the full schema every time. For a database that already
has data, use migrations instead.

## The migration system

The migration system compares your current model definitions against a stored
snapshot of the previous schema. It computes the diff and generates a SQL
migration file containing only the changes (new tables, altered columns, dropped
indexes, etc.).

Migrations are managed with the standalone `toasty` command-line tool.
Install it once:

```bash
cargo install toasty-cli
```

Run it from any Cargo package that uses Toasty. No project changes are
needed: for commands that need your schema, the CLI compiles your package
and reads the schema out of the build artifact, so it always sees exactly
the model types your application runs.

| Command | What it does |
|---|---|
| `migrate generate` | Diffs the current schema against the last snapshot and writes a SQL migration file |
| `migrate apply` | Runs pending migrations against the database |
| `migrate snapshot` | Prints the current schema as TOML |
| `migrate drop` | Removes a migration from history and deletes its files |
| `migrate reset` | Drops all tables and optionally re-applies all migrations |

In a workspace, the CLI operates on the workspace root package. Pass
`-p <package>` to select a different member, mirroring `cargo`:

```bash
toasty -p api migrate generate --flavor postgresql
```

Migration files and `Toasty.toml` live in the selected package's directory,
so each workspace member keeps its own migration history.

### How the CLI reads your schema

`migrate generate` and `migrate snapshot` build your package in the `dev`
profile — a bin target if the package has one, otherwise the lib compiled as
a `cdylib` — and run the artifact with the `TOASTY_DUMP_SCHEMA` environment
variable set. A constructor inside `toasty` sees the variable, writes the
schema derived from your `#[derive(Model)]` types to stdout, and exits
before your `main` runs. Release builds do not contain this constructor.

The other commands operate on saved migration files or connect to a
database URL directly; they do not build your package.

## Configuration options

The CLI writes a default `Toasty.toml` next to your package's `Cargo.toml`
on first use. The `[migration]` section controls migration behavior:

| Option | Default | Description |
|---|---|---|
| `path` | `"toasty"` | Base directory for migration files, snapshots, and history |
| `prefix_style` | `"Sequential"` | File naming: `"Sequential"` (0001_, 0002_) or `"Timestamp"` (20240112_153045_) |
| `flavor` | unset | Database flavor used when `--flavor` is not passed: `"sqlite"`, `"postgresql"`, `"mysql"`, or `"turso"` |

## Generating a migration

Run the generate command to create your first migration. The `--flavor`
flag names the target database, since each flavor maps model types to
different column types:

```bash
toasty migrate generate --flavor sqlite
```

Set `flavor` in `Toasty.toml` to omit the flag.

If there are schema changes since the last snapshot (or no snapshot exists yet),
the CLI creates three things inside the configured `path` directory:

```text
toasty/
├── history.toml
├── migrations/
│   └── 0000_migration.sql
└── snapshots/
    └── 0000_snapshot.toml
```

- **`migrations/0000_migration.sql`** — the SQL DDL statements for this
  migration. For a new project this contains all `CREATE TABLE` and `CREATE
  INDEX` statements.
- **`snapshots/0000_snapshot.toml`** — a TOML serialization of the full schema
  at this point. The next `generate` run diffs against this snapshot.
- **`history.toml`** — tracks all migrations by name and ID.

You can give a migration a descriptive name with `--name`:

```bash
toasty migrate generate --flavor sqlite --name add_posts_table
```

This produces `0001_add_posts_table.sql` instead of `0001_migration.sql`.

### Rename detection

When the diff contains a dropped table and an added table (or dropped and added
columns within a table), the CLI asks whether this is a rename. For example, if
you rename a `users` table to `accounts`, the CLI prompts:

```text
Table "users" is missing
> Drop "users" ✖
  Rename "users" → "accounts"
```

Choosing the rename option generates an `ALTER TABLE ... RENAME` statement
instead of a `DROP TABLE` followed by a `CREATE TABLE`.

## Applying migrations

Run pending migrations against the database named by `--url` (or the
`DATABASE_URL` environment variable):

```bash
toasty migrate apply --url sqlite:./my_app.db
```

The CLI reads `history.toml` to find all defined migrations, then queries the
database's `__toasty_migrations` tracking table to see which ones have already
been applied. It executes each pending migration in order inside a transaction
and records it in the tracking table.

If all migrations are already applied, the command prints a message and exits
without changes.

## Embedding migrations

Applications that ship as a single binary can compile generated migrations
into that binary. Enable the `migration` feature and call
`embed_migrations!` to embed the default `toasty/` migration directory:

```rust,ignore
static MIGRATIONS: toasty::migration::MigrationSet = toasty::embed_migrations!();

async fn migrate(db: &toasty::Db) -> toasty::Result<()> {
    let report = MIGRATIONS.apply(db).await?;

    println!("applied {} migrations", report.applied());
    Ok(())
}
```

Pass a path when the migrations live outside the default `toasty/` directory.
The path is relative to your crate's `Cargo.toml`. The macro embeds
`history.toml` and the files under `migrations/` that it names. Snapshot files
are not needed when applying migrations and are not embedded.

The compiler reports an error when the history file is invalid, two entries
use the same ID or name, or a referenced SQL file is missing. At runtime,
`MigrationSet::apply` checks the database's `__toasty_migrations` table and
skips migration IDs that are already present.

For multiple databases, embed and apply one migration set per database:

```rust,ignore
async fn migrate_all(primary_db: &toasty::Db, audit_db: &toasty::Db) -> toasty::Result<()> {
    let primary = toasty::embed_migrations!("toasty/primary");
    let audit = toasty::embed_migrations!("toasty/audit");

    primary.apply(primary_db).await?;
    audit.apply(audit_db).await?;
    Ok(())
}
```

Toasty does not associate migration sets with data source names. The
application decides which set applies to each `Db` and when to run it.

## Inspecting the current schema

Print the schema snapshot derived from your current model definitions:

```bash
toasty migrate snapshot --flavor sqlite
```

This compiles your package, extracts the schema, and outputs it as TOML,
showing all tables, columns, and indexes. It does not modify any files.

## Dropping a migration

Remove a migration from history and delete its files:

```bash
# Drop by name
toasty migrate drop --name 0001_add_posts_table.sql

# Drop the latest migration
toasty migrate drop --latest

# Interactive picker
toasty migrate drop
```

Dropping a migration removes its SQL file, its snapshot file, and its entry in
`history.toml`. It does not undo changes already applied to the database. To
undo applied changes, use `migrate reset` and re-apply.

## Resetting the database

Drop all tables and optionally re-apply migrations from scratch:

```bash
toasty migrate reset --url sqlite:./my_app.db
```

The CLI prompts for confirmation before proceeding (`-y` skips the prompt).
After dropping all tables, it re-applies every migration in the history. To
skip the re-apply step:

```bash
toasty migrate reset --url sqlite:./my_app.db --skip-migrations
```

## Generated SQL

A generated migration file contains standard SQL DDL. Toasty generates
database-specific SQL for the flavor you pass to `migrate generate`. Here is
an example for SQLite:

```sql
CREATE TABLE "users" (
    "id" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "email" TEXT NOT NULL,
    PRIMARY KEY ("id")
);
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "index_users_by_email" ON "users" ("email");
```

The `-- #[toasty::breakpoint]` comments mark boundaries where the driver splits
statements for execution. Some databases (like PostgreSQL) can execute multiple
statements in a single batch, while others require them one at a time. The
breakpoint markers handle this transparently.

## Migration tracking

Toasty tracks applied migrations in a `__toasty_migrations` table that it
creates automatically. Each row stores the migration's ID (a random 64-bit
integer from `history.toml`), its name, and a timestamp. The `migrate apply`
command checks this table to determine which migrations are pending.

## Typical workflow

A common development cycle looks like this:

1. Edit your model structs (add a field, change a type, add an index)
2. Run `toasty migrate generate --name describe_change`
3. Review the generated SQL file
4. Run `toasty migrate apply --url <database>` to update the database
5. Commit the migration files, snapshot, and updated history alongside your code

For early development when the schema changes frequently, `push_schema` is
simpler. Switch to migrations when your database has data you want to preserve
across schema changes.

> **Runnable example:** [`service-ops`] lays out a lib + binary project with connection pooling, tracing, and migrations managed with the `toasty` CLI.

[`service-ops`]: https://github.com/tokio-rs/toasty/tree/main/examples/service-ops
