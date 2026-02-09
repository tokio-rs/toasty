# Toasty ORM - Development Roadmap

This roadmap outlines the feature areas and potential enhancements for the Toasty ORM.

## Overview

Toasty is an easy-to-use ORM for Rust that supports both SQL and NoSQL databases. This roadmap documents core features, current implementation status, and potential future work.

> **User Documentation:** See the [`guide/`](../guide/) directory for complete API documentation and usage examples.

## Feature Areas

### Query Capabilities

**[Query Ordering, Limits & Pagination](./order_limit_pagination.md)** | [User Guide](../guide/pagination.md)
- Single column ordering (implemented)
- Multi-column ordering (manual construction available, convenience method potential future work)
- Cursor-based pagination with bidirectional navigation (implemented)
- Page<T> return type with navigation cursors (implemented)
- Direct `.limit()` method (potential future work)
- `.first()` convenience method (implemented)
- `.last()` convenience method (potential future work)

**[Query Constraints & Filtering](./query-constraints.md)**
- Basic comparisons: eq, ne, gt, ge, lt, le (implemented)
- AND operator (implemented)
- OR, NOT, IS NULL (core AST exists, needs user API)
- String operations: contains, starts with, ends with, LIKE (partial AST support)
- IN/NOT IN (IN exists, NOT IN needs implementation)
- Case-insensitive matching (needs implementation)
- BETWEEN / range queries (needs implementation)
- Relation filtering (filter by associated model fields, needs implementation)
- Field-to-field comparison (needs implementation)
- Aggregate queries and GROUP BY / HAVING (needs implementation)

### Data Types & Validation

**Extended Data Types**
- Primitive types (implemented)
- Embedded structs (implemented)
- Enums (implemented)
- JSON/JSONB support (potential future work)
- Array types (potential future work)
- Date/Time types with timezone (partial support via jiff)

### Relationships & Loading

**Relationships**
- Has many (implemented)
- Has one (implemented)
- Belongs to (implemented)
- Many-to-many (potential future work)
- Self-referential relationships (potential future work)
- Polymorphic associations (potential future work)
- Preloading (implemented)
- N+1 prevention strategies (potential future work)

### Query Building

**Query Features**
- Type-safe query builder (implemented)
- Filter methods for indexed fields (implemented)
- Generic `.filter()` for arbitrary conditions (implemented)
- Scoped queries on associations (implemented)
- Subqueries (partial support)
- Raw SQL escape hatch (potential future work)
- Conditional/dynamic query building (needs better ergonomics)

### Schema Management

**Migrations**
- Schema migration system (potential future work)
- Migration generation (potential future work)
- Rollback support (potential future work)
- Schema versioning (potential future work)

### Performance

**Optimization Features**
- Connection pooling (basic support exists)
- Async/await throughout (implemented)
- Stream-based result iteration (implemented)
- Batch operations (potential future work)
- Bulk inserts/updates (potential future work)
- Transaction management (potential future work)
- Query caching (potential future work)

### Developer Experience

**Tooling & Debugging**
- Compile-time type safety (implemented)
- Generated code from macro annotations (implemented)
- Query logging (potential future work)
- Performance monitoring (potential future work)
- Better error messages (ongoing improvement)
- CLI tools for schema management (potential future work)

### Data Integrity

**Validations & Features**
- Type-level validation via Rust's type system (implemented)
- Field constraints (index, auto, key annotations implemented)
- Soft deletes (potential future work)
- Optimistic locking (potential future work)
- Model-level validations (potential future work)

## Documentation Structure

This roadmap works alongside the user documentation:

- **Roadmap docs** (this directory): Technical implementation details, current state analysis, and potential enhancements
- **User guide** ([`guide/`](../guide/)): API documentation and usage examples

Each roadmap document includes:
- Current implementation
- Potential future work
- Technical design considerations
- Implementation approaches

Each guide document shows:
- Complete API examples
- Usage patterns and best practices
- Integration examples

## Notes

The roadmap documents describe capabilities and potential enhancements without tracking implementation status over time. For current implementation details, refer to the individual roadmap documents or the user guide.
