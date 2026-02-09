# Toasty ORM - Development Roadmap

This roadmap outlines potential enhancements and missing features for the Toasty ORM.

## Overview

Toasty is an easy-to-use ORM for Rust that supports both SQL and NoSQL databases. This roadmap documents potential future work and feature gaps.

> **User Documentation:** See the [`guide/`](../guide/) directory for complete API documentation and usage examples.

## Feature Areas

### Query Capabilities

**[Query Ordering, Limits & Pagination](./order_limit_pagination.md)** | [User Guide](../guide/pagination.md)
- Multi-column ordering convenience method (`.then_by()`)
- Direct `.limit()` method for non-paginated queries
- `.last()` convenience method

**[Query Constraints & Filtering](./query-constraints.md)**
- OR, NOT, IS NULL (core AST exists, needs user API)
- String operations: contains, starts with, ends with, LIKE (partial AST support)
- NOT IN
- Case-insensitive matching
- BETWEEN / range queries
- Relation filtering (filter by associated model fields)
- Field-to-field comparison
- Aggregate queries and GROUP BY / HAVING

### Data Types & Validation

**Extended Data Types**
- Enum support (partial implementation)
- JSON/JSONB support
- Array types
- Full date/time types with timezone

### Relationships & Loading

**Relationships**
- Many-to-many relationships
- Self-referential relationships
- Polymorphic associations
- N+1 prevention strategies

### Query Building

**Query Features**
- Subquery improvements
- Raw SQL escape hatch
- Better conditional/dynamic query building ergonomics

### Schema Management

**Migrations**
- Schema migration system
- Migration generation
- Rollback support
- Schema versioning

### Performance

**Optimization Features**
- Batch operations
- Bulk inserts/updates
- Transaction management
- Query caching

### Developer Experience

**Tooling & Debugging**
- Query logging
- Performance monitoring
- Better error messages
- CLI tools for schema management

### Data Integrity

**Validations & Features**
- Soft deletes
- Optimistic locking
- Model-level validations

## Documentation Structure

This roadmap works alongside the user documentation:

- **Roadmap docs** (this directory): Technical implementation details, gaps analysis, and potential enhancements
- **User guide** ([`guide/`](../guide/)): API documentation and usage examples

Each roadmap document includes:
- Core AST support without user API
- Potential future work
- Technical design considerations
- Implementation approaches

Each guide document shows:
- Complete API examples
- Usage patterns and best practices
- Integration examples

## Notes

The roadmap documents describe potential enhancements and missing features. For information about what's currently implemented, refer to the user guide or test the API directly.
