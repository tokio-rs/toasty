# Embedded Migrations

## Summary

`embed_migrations!` compiles a Toasty migration directory into an application
binary. Applications apply the resulting `MigrationSet` to a `Db` without
shipping `history.toml` or SQL files beside the executable.

## Motivation

`toasty-cli migration apply` reads migrations from the filesystem at runtime.
This works for a project-local migration binary, but it requires deployment
systems to copy the migration directory with that binary. Single-file
deployments, containers with read-only application filesystems, and services
that migrate during startup need the same migrations available from the
compiled program.

Applications with multiple databases also need to select a migration set per
`Db`. Toasty should provide the single-database operation while leaving data
source naming and startup ordering to the application.

## User-facing API

Enable Toasty's `migration` feature, generate migrations with `toasty-cli`,
then embed the generated `toasty/` directory:

```rust
static MIGRATIONS: toasty::migration::MigrationSet = toasty::embed_migrations!();

let report = MIGRATIONS.apply(&db).await?;
println!("applied {} migrations", report.applied());
```

Pass a path when the migrations live outside the default `toasty/`
directory:

```rust
let migrations = toasty::embed_migrations!("db/primary");
let report = migrations.apply(&db).await?;

println!("applied {} migrations", report.applied());
```

Paths are relative to the invoking crate's `CARGO_MANIFEST_DIR`. The macro
reads the same directory layout as `toasty-cli`:

```text
toasty/
├── history.toml
└── migrations/
    ├── 0000_initial.sql
    └── 0001_add_posts.sql
```

The macro embeds `history.toml` and every SQL file named by its migration
entries. Snapshots remain development inputs and are not embedded.

An application with multiple databases creates one set per migration
directory and applies each set to the corresponding `Db`:

```rust
let primary = toasty::embed_migrations!("toasty/primary");
let audit = toasty::embed_migrations!("toasty/audit");

primary.apply(&primary_db).await?;
audit.apply(&audit_db).await?;
```

Toasty does not register data source names or apply migrations automatically.
The application controls which set applies to each database and when it runs.

## Behavior

`embed_migrations!` defaults to the `toasty/` directory when called without an
argument. It validates the history file and referenced SQL files while compiling
the application. A missing directory, unsupported history version, duplicate
migration ID, duplicate migration name, missing SQL file, invalid UTF-8 history
file, or migration name containing path traversal produces a compiler error at
the macro invocation.

`MigrationSet::apply` acquires a pooled connection from the `Db` and asks it
for applied migration IDs. It processes embedded entries in `history.toml` order, skips
IDs already present in the database, and calls the existing driver migration
operation for each pending entry. The returned `MigrationReport` contains the
number applied and skipped by that call.

An empty history succeeds with both report counts set to zero. If a migration
fails, `apply` returns the driver error and does not attempt later entries.
Drivers retain their existing transaction and migration-recording behavior.

## Edge cases

Changing `history.toml` or a referenced SQL file causes Cargo to rebuild the
macro invocation because the expansion contains `include_str!` references to
those files.

Migration IDs remain the identity used for idempotency. Renaming an already
applied migration without changing its ID does not run it again.

Two concurrent calls against the same database are not coordinated by
`MigrationSet`. The driver's migration tracking table remains the authority;
applications should serialize startup migration calls when they share a
database.

The same `MigrationSet` can be applied to multiple `Db` values. Each database
uses its own applied-migration records.

## Driver integration

Driver implementors do not add a capability flag or operation. Embedded
migrations use the existing `Connection::applied_migrations` and
`Connection::apply_migration` methods.

Backends that already support migrations work without changes. Backends that
reject migrations, including DynamoDB, continue to return their existing
unsupported error when `MigrationSet::apply` reaches the driver.

## Alternatives considered

Adding a `rust-embed` dependency would make Toasty depend on a general asset
library and would expose that library's debug-versus-release loading behavior.
The process macro only needs Toasty's generated directory layout and always
uses compile-time `include_str!` data.

A runtime `MigrationSource` trait would support arbitrary storage, but
filesystem migrations already exist through `toasty-cli`. The requested use
case needs a static compile-time source, so a source trait adds an abstraction
without another implementation.

Listing every migration in user code with `include_str!` avoids a process
macro but duplicates IDs, names, and ordering from `history.toml`. That creates
two migration manifests that can diverge.

## Open questions

There are no blocking open questions.

## Out of scope

- Migration generation remains a filesystem operation provided by
  `toasty-cli`; applications embed generated output.
- Snapshots are not embedded because applying migrations does not read them.
- Toasty does not add a multi-data-source registry; applications associate
  migration sets with `Db` values.
- This design does not add rollback or down migrations.
