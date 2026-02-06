# Toasty ORM - MVP Roadmap

This roadmap tracks the remaining features and improvements needed to reach MVP status for the Toasty ORM.

## Overview

Toasty aims to be an easy-to-use ORM for Rust that supports both SQL and NoSQL databases. To reach MVP status, we need to implement core features that web developers expect from a modern ORM.

> **📖 User Documentation:** See the [`guide/`](../guide/) directory for complete API documentation and usage examples.

## Status Key

- 🔴 Not Started
- 🟡 In Progress
- 🟢 Complete
- 🔵 Needs Review

## Core Feature Areas

### Query Capabilities
- [**Query Ordering, Limits & Pagination**](./order_limit_pagination.md) 🟡 | [📖 Guide](../guide/pagination.md)
  - Order by single/multiple columns
  - Cursor-based pagination with bidirectional navigation
  - Page<T> return type with navigation cursors
  - Cursor serialization for web APIs
- [**Query Constraints & Filtering**](./query-constraints.md) 🟡
  - OR, NOT, IS NULL constraints (AST exists, needs user API)
  - String operations: contains, starts with, ends with, LIKE
  - Case-insensitive matching
  - BETWEEN / range queries
  - Relation filtering (filter by associated model fields)
  - Aggregate queries and GROUP BY / HAVING

### Data Types & Validation
- **Extended Data Types** 🔴
  - JSON/JSONB support
  - Array types
  - Enum support
  - UUID support
  - Date/Time types with timezone

### Relationships & Loading
- **Advanced Relationships** 🔴
  - Many-to-many relationships
  - Self-referential relationships
  - Polymorphic associations
  - Eager loading (N+1 prevention)
  - Lazy loading configuration

### Query Building
- **Advanced Queries** 🔴
  - Complex WHERE conditions (OR, NOT)
  - Subqueries
  - Raw SQL escape hatch
  - Query builder pattern
  - Aggregations (COUNT, SUM, AVG, etc.)
  - GROUP BY and HAVING

### Schema Management
- **Migrations** 🔴
  - Schema migration system
  - Migration generation
  - Rollback support
  - Schema versioning

### Performance
- **Optimization Features** 🔴
  - Connection pooling configuration
  - Query caching
  - Batch operations
  - Bulk inserts/updates
  - Transaction management

### Developer Experience
- **Tooling & Debugging** 🔴
  - Query logging
  - Performance monitoring
  - Better error messages
  - CLI tools for schema management
  - Documentation generation

### Data Integrity
- **Validations & Callbacks** 🔴
  - Field validations
  - Model validations
  - Soft deletes
  - Optimistic locking

## Next Steps

We are currently focusing on:
1. **Query Ordering, Limits & Pagination** - Essential for any data listing functionality

## Documentation Structure

This roadmap works alongside the user documentation:

- **Roadmap docs** (this directory): Technical implementation details, current state analysis, and development priorities
- **User guide** ([`guide/`](../guide/)): API documentation and usage examples for the target API (including unimplemented features)

Each roadmap document includes:
- Current state analysis
- Missing functionality
- Implementation roadmap
- Technical design decisions

Each guide document shows:
- Complete API examples (including future APIs marked as "work in progress")
- Usage patterns and best practices
- Integration examples