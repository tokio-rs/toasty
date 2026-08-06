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

Migrations are managed with the `toasty` command-line tool:

| Command | What it does |
|---|---|
| `migrate generate` | Diffs the current schema against the last snapshot and writes a SQL migration file |
| `migrate apply` | Runs pending migrations against the database |
| `migrate snapshot` | Prints the current schema as TOML |
| `migrate drop` | Removes a migration from history and deletes its files |
| `migrate reset` | Drops all tables and optionally re-applies all migrations |

## Installing the CLI

Install it once:

```bash
cargo install toasty-cli
```

Then run it from any Cargo package that depends on `toasty`. There is nothing
to add to your `Cargo.toml` and no code to write.

Computing a schema diff needs your model types, which only exist in a compiled
artifact. `toasty migrate generate` builds one — your package's binary, or its
library as a `cdylib` when the package has no binary — and reads the schema
back out of it. `toasty` contributes a constructor to that build which writes
the schema and exits when the CLI sets `TOASTY_DUMP_SCHEMA`. Your `main` never
runs. The constructor is compiled only under `cfg(debug_assertions)`, so
release builds do not carry it.

To see what the CLI sees, set the variable yourself:

```bash
TOASTY_DUMP_SCHEMA=1 cargo run
```

### Selecting a package

In a workspace, the CLI uses the root package. Use `-p` to pick a different
member, as with `cargo`:

```bash
toasty -p api migrate generate --flavor postgresql
```

A workspace with no root package (a virtual manifest) requires `-p`. A package
with more than one binary requires `--bin <name>`.

Migration files and `Toasty.toml` are resolved relative to the selected
package's directory, so the commands behave the same from anywhere in the
workspace.

## Configuration

Add a `Toasty.toml` configuration file in your project root:

```toml
[migration]
path = "toasty"
prefix_style = "Sequential"
checksums = false
statement_breakpoints = true
```

## Configuration options

The `[migration]` section in `Toasty.toml` controls migration behavior:

| Option | Default | Description |
|---|---|---|
| `path` | `"toasty"` | Base directory for migration files, snapshots, and history |
| `prefix_style` | `"Sequential"` | File naming: `"Sequential"` (0001_, 0002_) or `"Timestamp"` (20240112_153045_) |
| `checksums` | `false` | When true, stores MD5 checksums in history to detect modified migration files |
| `statement_breakpoints` | `true` | Adds `-- #[toasty::breakpoint]` comments between SQL statements so drivers can split them for execution |

## Generating a migration

Run the generate command to create your first migration. `--flavor` names the
SQL dialect to generate for — `sqlite`, `postgresql`, or `mysql`:

```bash
toasty migrate generate --flavor sqlite
```

The dialect is named rather than discovered from a connection, so migrations
can be generated for a database that is not running.

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

Run pending migrations against the database:

```bash
toasty migrate apply --url sqlite://my_app.db
```

Applying a migration runs saved SQL files, so it needs no models and does not
build your package.

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

This outputs the full schema as TOML, showing all tables, columns, and indexes.
It does not modify any files — it reads directly from the registered models.

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
toasty migrate reset --url sqlite://my_app.db
```

The CLI prompts for confirmation before proceeding. After dropping all tables,
it re-applies every migration in the history. To skip the re-apply step:

```bash
toasty migrate reset --url sqlite://my_app.db --skip-migrations
```

## Generated SQL

A generated migration file contains standard SQL DDL. Toasty generates
database-specific SQL for the flavor you pass to `--flavor`. Here is an example
for SQLite:

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
integer from `history.toml`), its name, and a timestamp. The `migration apply`
command checks this table to determine which migrations are pending.

## Running the commands from your own binary

The `toasty-cli` crate is also a library. `ToastyCli` exposes the same
migration subcommands over a `Db` you build yourself, which is useful when a
deployment runs migrations from a binary it already ships rather than from an
installed tool:

```rust,ignore
use toasty_cli::{Config, ToastyCli};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
    let db = toasty::Db::builder()
        .models(toasty::models!(crate::*))
        .connect("sqlite:./my_app.db")
        .await?;

    ToastyCli::with_config(db, config).parse_and_run().await?;
    Ok(())
}
```

Because this binary links your models and connects to a database, it takes
neither `--flavor` nor `--url`: both come from the `Db` you pass it.

## Typical workflow

A common development cycle looks like this:

1. Edit your model structs (add a field, change a type, add an index)
2. Run `toasty migrate generate --flavor <flavor> --name describe_change`
3. Review the generated SQL file
4. Run `toasty migrate apply --url <url>` to update the database
5. Commit the migration files, snapshot, and updated history alongside your code

For early development when the schema changes frequently, `push_schema` is
simpler. Switch to migrations when your database has data you want to preserve
across schema changes.

> **Runnable example:** [`service-ops`] lays out a lib + binaries project with connection pooling, tracing, and the `toasty-cli` migration workflow.

[`service-ops`]: https://github.com/tokio-rs/toasty/tree/main/examples/service-ops
