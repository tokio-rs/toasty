# Toasty Developer Documentation

Documentation for contributors working on Toasty itself. For user-facing
documentation, see the [Toasty Guide](../guide/).

Start with [`CONTRIBUTING.md`](../../CONTRIBUTING.md) — it describes how
to propose changes and land PRs.

Docs built from the latest commit on `main`:

- [Nightly user guide](https://tokio-rs.github.io/toasty/nightly/guide/)
- [Nightly API docs](https://tokio-rs.github.io/toasty/nightly/api/)

## Architecture

High-level documentation of how Toasty is put together.

- [Architecture Overview](./architecture/README.md)
- [Query Engine](./architecture/query-engine.md)
- [Type System](./architecture/type-system.md)

## Design Documents

Guide-level design documents for specific features. Use
[`_template.md`](./design/_template.md) when starting a new one.

- [Design Overview](./design/README.md)
- [Atomic Batches for DynamoDB](./design/atomic-batches.md)
- [Completing Column Projection](./design/column-projection.md)
- [Document and Collection Fields](./design/document-fields.md)
- [DynamoDB Driver Error-Handling Convention](./design/ddb-error-handling.md)
- [Enums and Embedded Structs](./design/enums-and-embedded-structs.md)
- [Filtering Included Relations](./design/include-filters.md)
- [Lower-then-Simplify Pipeline](./design/lower-then-simplify.md)
- [`query!` Macro](./design/query-macro.md)
- [Retry-Safe Recovery from Connection Loss](./design/retry-safe-recovery.md)
- [Static Assertions for `create!` Required Fields](./design/static-assertions-create-macro.md)

## Roadmap

- [Roadmap](./roadmap.md) — planned work and feature gaps

## Project

- [Commit Guidelines](./COMMITS.md)
- [GitHub Labels](./labels.md)
