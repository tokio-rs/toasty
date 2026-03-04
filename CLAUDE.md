# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build everything
cargo build

# Run all tests (SQLite only, no external services needed)
cargo test

# Run tests for a specific crate
cargo test -p toasty
cargo test -p toasty-core
cargo test -p toasty-driver-integration-suite

# Run a single test by name
cargo test -p tests <test_name>

# Run tests with specific database drivers (requires running external services)
cargo test -p tests --features mysql
cargo test -p tests --features postgresql
cargo test -p tests --features dynamodb

# Lint
cargo clippy

# Format
cargo fmt
```

## Architecture

Toasty is a Rust ORM supporting SQL (SQLite, PostgreSQL, MySQL) and NoSQL (DynamoDB) databases. It is a Cargo workspace.

### Crate Overview

| Crate | Purpose |
|---|---|
| `toasty` | User-facing API: `Db`, query engine, runtime |
| `toasty-core` | Shared types: schema representations, statement AST, driver interface |
| `toasty-macros` | Proc-macro entry points (`#[derive(Model)]`, `#[derive(Embed)]`) |
| `toasty-codegen` | Code generation logic called by `toasty-macros` |
| `toasty-sql` | SQL serialization (statement AST → SQL string), used by SQL drivers |
| `toasty-driver-sqlite/postgresql/mysql/dynamodb` | Database driver implementations |
| `toasty-driver-integration-suite` | Shared integration test suite run against all drivers |
| `toasty-driver-integration-suite-macros` | Proc-macros for the test suite (`#[driver_test]`, `generate_driver_tests!`) |
| `tests` | Workspace-level integration tests (invokes the suite per driver) |
| `std-util` | Internal utilities |

### Schema Layers

The schema has two distinct layers bridged by a mapping:

- **App schema** (`toasty-core/src/schema/app/`): Model-level definitions — fields, relations, constraints. What the Rust code sees.
- **DB schema** (`toasty-core/src/schema/db/`): Table/column-level definitions. What the database sees.
- **Mapping** (`toasty-core/src/schema/mapping/`): Connects app fields to db columns, supporting non-1-1 mappings.

### Code Generation (`toasty-codegen`)

`#[derive(Model)]` on a user struct triggers:
1. `toasty-macros` delegates to `toasty-codegen::generate_model()`
2. `codegen/src/schema/` parses macro attributes into an internal model representation
3. `codegen/src/expand/` generates implementations: `Model` trait, query builders (`find_by_*`, `filter_by_*`), create/update builders, relation methods, field accessors, and the runtime schema registration

### Query Engine (`toasty/src/engine/`)

Queries go through a multi-phase compilation pipeline:

```
Statement AST → [Simplify] → [Lower to HIR] → [Plan to MIR] → [Execution Plan] → [Execute]
```

1. **Simplify** (`simplify.rs`): Normalizes the AST — rewrites relationship navigation into explicit subqueries, flattens expressions
2. **Lower** (`lower.rs`): Converts model-level statements to HIR (High-level IR); resolves model fields to table columns; expands `INCLUDE` associations into subqueries; tracks dependency graph between statements
3. **Plan** (`plan.rs`): Converts HIR dependency graph (which may have cycles) into a MIR DAG of operations. Breaks cycles by introducing `NestedMerge` operations
4. **Exec** (`exec.rs`): Interpreter that runs the compiled action sequence; assigns variable slots for intermediate results; the only phase that calls the database driver

The final compiled form is a mini-program: a sequence of actions operating on numbered variable slots (e.g., `$0 = ExecSQL(...)`, `$1 = NestedMerge($0, ...)`).

### Driver Interface

Drivers implement `Driver` + `Connection` traits from `toasty-core/src/driver.rs`. The single `Connection::exec()` method receives an `Operation` enum covering both SQL operations (`QuerySql`, `Insert`) and key-value operations (`GetByKey`, `QueryPk`, etc.). The planner uses `driver.capability()` to decide which operation types to generate.

### Integration Test Suite

Tests live in `crates/toasty-driver-integration-suite/src/tests/`. Each test function is annotated with `#[driver_test(id(ID))]` where `ID` is the key type (e.g., `uuid::Uuid`, `i64`). The `generate_driver_tests!` macro in `tests/tests/<driver>.rs` instantiates all suite tests for that driver's `Setup` implementation.

To add a new integration test, add a file to `crates/toasty-driver-integration-suite/src/tests/` — it is automatically discovered via the `generate_test_registry!` proc-macro at compile time.

### Further Reading

- `docs/ARCHITECTURE.md` — architectural overview
- `docs/architecture/query-engine.md` — detailed query engine documentation
- `docs/architecture/type-system.md` — type system design
