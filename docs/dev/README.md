# Toasty Developer Documentation

Documentation for contributors working on Toasty itself. For user-facing
documentation, see the [Toasty Guide](../guide/).

Start with [`CONTRIBUTING.md`](../../CONTRIBUTING.md) — it describes how
to propose changes and land PRs.

## Architecture

High-level documentation of how Toasty is put together.

- [Architecture Overview](./architecture/README.md)
- [Query Engine](./architecture/query-engine.md)
- [Type System](./architecture/type-system.md)

## Design Documents

Guide-level design documents for specific features. Use
[`_template.md`](./design/_template.md) when starting a new one.

- [Design Overview](./design/README.md)
- [Batch Queries](./design/batch-queries.md)
- [DynamoDB OR Index Predicate](./design/ddb_or_index_predicate.md)
- [Embedded Enums Data Carrying Impl](./design/embedded-enums-data-carrying-impl.md)
- [Enums and Embedded Structs](./design/enums-and-embedded-structs.md)
- [Mapping Formalization](./design/mapping-formalization.md)
- [Pagination](./design/pagination.md)
- [Serialize Fields](./design/serialize-fields.md)
- [Static Assertions for `create!` Required Fields](./design/static-assertions-create-macro.md)
- [Database Enum Types](./design/postgresql-enum-type.md)

## Roadmap

Planned work and feature gaps.

- [Roadmap Overview](./roadmap/README.md)
- [Composite Keys](./roadmap/composite-keys.md)
- [Order, Limit, Pagination](./roadmap/order_limit_pagination.md)
- [Query Constraints](./roadmap/query-constraints.md)
- [Query Engine](./roadmap/query-engine.md)

## Project

- [Commit Guidelines](./COMMITS.md)
- [GitHub Labels](./labels.md)
