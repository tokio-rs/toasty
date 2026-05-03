# Standalone CLI

## Summary

`toasty-cli` can be installed globally and used in any Toasty project without
writing a custom CLI binary. The `#[derive(Model)]` proc macro writes each
model's schema to disk when `TOASTY_SCHEMA_OUT` is set during compilation.
`toasty migration generate` sets this variable internally, triggers a targeted
recompilation, reads the written files, and generates migrations from them.
Users can remove their custom CLI binary.

## Motivation

Using `toasty-cli` today requires a project-specific binary that imports your
models, constructs a `Db`, and delegates to `ToastyCli`:

```rust
// src/bin/cli.rs
use my_app::create_db;
use toasty_cli::{Config, ToastyCli};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load()?;
    let db = create_db().await?;
    let cli = ToastyCli::with_config(db, config);
    cli.parse_and_run().await?;
    Ok(())
}
```

This boilerplate exists because `toasty-cli` has no way to discover models
without them being compiled into the binary. Every project that uses migrations
must maintain this file and keep it in sync as models are added or removed.

## User-facing API

### Setup

Install the CLI:

```sh
cargo install toasty-cli
```

No other setup is required. The custom `src/bin/cli.rs` binary can be deleted.

### Generating migrations

Run `toasty migration generate` from the project root. The dialect is inferred
from the URL scheme; no connection is established.

```sh
toasty migration generate
toasty migration generate --name add_email_index
toasty migration generate --url postgres://localhost/mydb
```

If `--url` is not given, the CLI falls back to `DATABASE_URL` from the
environment or a `.env` file.

### Applying migrations

`migration apply` connects to the database and always requires a URL:

```sh
toasty migration apply
toasty migration apply --url postgres://localhost/mydb
```

### Controlling which models are included

Models are opt-in via `[migration] include` in `Toasty.toml`. Each entry is
either `crate_name::*` to include all models from a crate, or
`crate_name::ModelName` to include a specific model, mirroring the syntax of
the `models!` macro.

When `Toasty.toml` does not exist, the first run of `toasty migration generate`
creates it with the current crate's models included by default:

```toml
[migration]
include = ["my-app::*"]

[migration]
path = "toasty"
```

Edit this file to add models from other crates or remove the default entry:

```toml
[migration]
include = ["my-app::*", "shared-models::User", "shared-models::Team"]
```

An empty `include` list is valid and produces an empty schema, which generates
no migration.

### Library usage

`ToastyCli::new(db)` and `ToastyCli::with_config(db, config)` continue to work
unchanged. Projects that already have a custom CLI binary do not need to change
anything.

## Behavior

### Schema extraction

When `toasty migration generate` runs in standalone mode (no `Db` is provided
via the library API), it:

1. Creates a temporary directory via the OS temp directory API.
2. Runs `cargo metadata` to build the full dependency graph.
3. Reads `[migration] include` from `Toasty.toml` to determine the set of models
   to capture. If `Toasty.toml` does not exist, creates it with
   `include = ["<current_crate>::*"]` before proceeding. Each entry is either
   `crate_name::*` (all models in a crate) or `crate_name::ModelName` (a
   specific model).
4. From the crates referenced in that set, finds all packages that directly or
   transitively depend on `toasty-macros`. These are the packages whose proc
   macro invocations will write schema files.
5. Triggers recompilation of those packages with minimal disruption to the
   rest of the build cache:
   - Workspace members: touches the mtime of their source files.
   - External packages: runs `cargo clean -p <pkg>`.
   - If the targeted approach fails, falls back to a full `cargo clean`.
6. Runs `cargo check` with two environment variables set:
   - `TOASTY_SCHEMA_OUT` — absolute path to the temporary directory.
   - `TOASTY_SCHEMA_MODELS` — a comma-separated list of `crate::Model` entries
     representing the exact set of models to capture (e.g.
     `my_app::User,my_app::Todo,shared_models::Team`). Wildcard entries are
     expanded to explicit model names by the CLI before being passed through.
     Wildcards that cannot be pre-expanded (models in external crates not yet
     compiled) are passed as `crate_name::*` and matched by the proc macro
     against the struct name at expansion time.
7. Each `#[derive(Model)]` invocation checks whether
   `{CARGO_PKG_NAME}::{StructName}` (or `{CARGO_PKG_NAME}::*`) appears in
   `TOASTY_SCHEMA_MODELS`. If it matches, it writes the model's app-level
   schema to `{TOASTY_SCHEMA_OUT}/{crate_name}__{table_name}.toml`.
8. After `cargo check`, the CLI reads all `.toml` files from the temporary
   directory, assembles a complete model set, and converts it to a db schema
   using the same mapping logic that `Db::builder()` uses at runtime.
9. The temporary directory is deleted.

This db schema replaces `db.schema().db`. From that point, migration generation
proceeds identically to today: diff against the previous snapshot, prompt about
potential renames, write the migration file and updated snapshot.

### URL resolution

Both commands resolve the database URL in this order:

1. `--url` flag.
2. `DATABASE_URL` environment variable.
3. `DATABASE_URL` in a `.env` file in the project root. Variables already set
   in the environment take precedence over values in the file.

If no URL is found, the command fails with an error listing the sources it
checked.

`migration generate` infers the SQL dialect from the URL scheme and does not
establish a connection. `migration apply` connects to the database using the
full URL.

### Driver selection

`migration generate` instantiates the appropriate driver from the URL scheme
without establishing a database connection, then calls `driver.generate_migration(&diff)`
as today. `migration apply` connects using the same URL.

## Edge cases

### Incremental compilation

Cargo caches proc macro expansions based on source file fingerprints. If a model
source file has not changed since the last build, `#[derive(Model)]` will not
re-expand and will not write a schema file.

The targeted recompilation step (touching mtimes for workspace crates,
`cargo clean -p` for external ones) addresses this by invalidating the
packages that use `toasty-macros`. `cargo check` then re-expands all
`#[derive(Model)]` invocations in those packages.

### Models in `#[cfg(test)]`

Models defined only inside `#[cfg(test)]` blocks are not compiled during
`cargo check` and will not appear in the schema output.

### Renaming a model

When a model is renamed (e.g. `User` → `Account`), the macro writes
`Account.toml` and nothing writes `User.toml`. The CLI assembles a schema with
`Account` and no `User`. The diff against the previous snapshot produces a
dropped table and an added table, which the existing interactive rename prompt
already handles — the user is asked whether the missing table was renamed rather
than dropped. No special handling is required.

### Two models with the same struct name in one crate

Two structs in different modules of the same crate can share a name
(`admin::User`, `customer::User`) but must map to different database tables.
Schema files are named `{crate_name}__{table_name}.toml` using the table name
rather than the struct name. Since table names must be unique within a schema,
files cannot collide. Two models that map to the same table name are a schema
error caught during assembly regardless of this mechanism.

## Open questions

**Schema file format for individual models (blocking implementation).** Each
`#[derive(Model)]` writes one file per model. The format must be stable enough
to survive minor crate version bumps and expressive enough to reconstruct the
full app schema. Candidates are a subset of the existing `SnapshotFile` TOML
structure reusing `toasty-core` app schema types, or a purpose-built JSON
format. This needs to be decided before the proc macro side is implemented.


