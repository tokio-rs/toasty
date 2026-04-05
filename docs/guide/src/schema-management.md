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

Migrations are managed through a small CLI binary that you create in your
project using the `toasty-cli` library crate. Toasty cannot ship a ready-made
CLI tool because the tool needs access to your model types to compute the
schema. The `toasty-cli` crate provides `ToastyCli`, which handles argument
parsing and all migration subcommands:

| Command | What it does |
|---|---|
| `migration generate` | Diffs the current schema against the last snapshot and writes a SQL migration file |
| `migration apply` | Runs pending migrations against the database |
| `migration snapshot` | Prints the current schema as TOML |
| `migration drop` | Removes a migration from history and deletes its files |
| `migration reset` | Drops all tables and optionally re-applies all migrations |

## Setting up the CLI

Add `toasty-cli` to your project:

```toml
[dependencies]
toasty = { version = "0.1", features = ["sqlite"] }
toasty-cli = "0.1"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
```

Create a CLI binary in `src/bin/cli.rs`:

```rust,ignore
use toasty_cli::{Config, ToastyCli};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;

    let db = toasty::Db::builder()
        .models(toasty::models!(crate::*))
        .connect("sqlite:./my_app.db")
        .await?;

    let cli = ToastyCli::with_config(db, config);
    cli.parse_and_run().await?;

    Ok(())
}
```

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

Run the generate command to create your first migration:

```bash
cargo run --bin my-cli -- migration generate
```

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
cargo run --bin my-cli -- migration generate --name add_posts_table
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
cargo run --bin my-cli -- migration apply
```

The CLI reads `history.toml` to find all defined migrations, then queries the
database's `__toasty_migrations` tracking table to see which ones have already
been applied. It executes each pending migration in order inside a transaction
and records it in the tracking table.

If all migrations are already applied, the command prints a message and exits
without changes.

## Inspecting the current schema

Print the schema snapshot derived from your current model definitions:

```bash
cargo run --bin my-cli -- migration snapshot
```

This outputs the full schema as TOML, showing all tables, columns, and indexes.
It does not modify any files — it reads directly from the registered models.

## Dropping a migration

Remove a migration from history and delete its files:

```bash
# Drop by name
cargo run --bin my-cli -- migration drop --name 0001_add_posts_table.sql

# Drop the latest migration
cargo run --bin my-cli -- migration drop --latest

# Interactive picker
cargo run --bin my-cli -- migration drop
```

Dropping a migration removes its SQL file, its snapshot file, and its entry in
`history.toml`. It does not undo changes already applied to the database. To
undo applied changes, use `migration reset` and re-apply.

## Resetting the database

Drop all tables and optionally re-apply migrations from scratch:

```bash
cargo run --bin my-cli -- migration reset
```

The CLI prompts for confirmation before proceeding. After dropping all tables,
it re-applies every migration in the history. To skip the re-apply step:

```bash
cargo run --bin my-cli -- migration reset --skip-migrations
```

## Generated SQL

A generated migration file contains standard SQL DDL. Toasty generates
database-specific SQL based on the driver you connect with. Here is an example
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

## Typical workflow

A common development cycle looks like this:

1. Edit your model structs (add a field, change a type, add an index)
2. Run `migration generate --name describe_change`
3. Review the generated SQL file
4. Run `migration apply` to update the database
5. Commit the migration files, snapshot, and updated history alongside your code

For early development when the schema changes frequently, `push_schema` is
simpler. Switch to migrations when your database has data you want to preserve
across schema changes.
