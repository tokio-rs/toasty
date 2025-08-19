# toasty-core Component Context

## Purpose
The foundation layer that defines all core abstractions, data structures, and interfaces used throughout Toasty. This crate has zero dependencies on other Toasty crates and serves as the contract between all components.

## Key Responsibilities

### Schema Definition (`src/schema/`)
- **Application Schema** (`app/`): High-level model definitions as users write them
  - Models, fields, relations, constraints, indexes
  - Primary keys and foreign keys
  - Field types and primitives
  - Auto-generation settings and constraints
- **Database Schema** (`db/`): Low-level database table structures  
  - Tables, columns, indexes
  - Database-specific types
- **Mapping** (`mapping/`): Bridges app models to database tables
  - Handles cases where multiple models map to one table
  - Column name transformations
  - Expression-based data transformations
- **Schema Builder** (`builder/`): Three-phase construction process
  - Validation against driver capabilities
  - Constraint generation
  - Mapping construction with placeholder support

### Statement AST (`src/stmt/`)
Comprehensive abstract syntax tree with 26+ expression types:
- **Logical Expressions**: `ExprAnd`, `ExprOr`, `ExprBinaryOp` (6 operators)
- **Data Access**: `ExprColumn`, `ExprReference`, `ExprKey`, `ExprProject`
- **String Operations**: `ExprLike`, `ExprBeginsWith`, `ExprConcat`, `ExprConcatStr`
- **Collection Operations**: `ExprInList`, `ExprInSubquery`, `ExprList`, `ExprMap`
- **Type Operations**: `ExprCast`, `ExprTy`, `ExprIsNull`, `ExprEnum`
- **Queries**: `Select`, `Query`, `Source`, `Join`, `CTE`, `With`
- **Mutations**: `Insert`, `Update`, `Delete` with returning clauses
- **Values**: 13 primitive types + complex types (Record, List, SparseRecord)
- **Streaming**: `ValueStream` with hybrid buffer+stream design

### Advanced Visitor Pattern (`src/stmt/visit*.rs`)
Dual visitor system for AST transformation:
- **Visit Trait**: Immutable traversal with 40+ methods
- **VisitMut Trait**: Mutable transformation support
- **Node Trait**: Universal interface for all AST nodes
- **Utility Functions**: `for_each_expr()` for functional-style traversal
- **Smart Delegation**: `impl<V: Visit> Visit for &mut V` pattern

### Expression Substitution System (`src/stmt/substitute.rs`)
Sophisticated parameterized expression evaluation:
- **Input Trait**: Allows multiple input sources (Vec, arrays, custom)
- **Lazy Resolution**: Arguments resolved during substitution
- **ExprArg Support**: Parameterized expressions with late binding
- **Smart Recursion**: Special handling to avoid infinite loops

### Driver Interface (`src/driver/`)
Abstract interface with 8 core operations:
- **Operations**: GetByKey, Insert, UpdateByKey, DeleteByKey, QueryPk, FindPkByIndex, QuerySql, Transaction
- **Capabilities**: Database feature advertisement system
- **Response**: Unified Count/Values response types
- **Streaming**: First-class support for large result sets

## Common Change Patterns

### Adding a New Primitive Type
1. Update `stmt/ty.rs::Type` enum (field primitives use `stmt::Type`)
2. Add conversion in `stmt/value.rs::Value`
3. Update `schema/app/field/primitive.rs::FieldPrimitive` if needed
4. Update Visit/VisitMut if needed

### Adding a Statement Node
1. Create new struct in `stmt/expr_*.rs` or similar
2. Add to `stmt/expr.rs::Expr` enum if expression
3. Implement Visit and VisitMut trait visiting
4. Add any necessary helper methods

### Schema Evolution
When modifying schema structures:
- App schema changes rarely need DB schema changes
- Maintain backward compatibility in schema structs
- Update schema builder if adding new concepts

## Important Files

- `lib.rs`: Public API exports
- `schema/app/model.rs`: Model definition structure
- `schema/app/field.rs`: Field types and attributes  
- `stmt/expr.rs`: Core expression enum
- `stmt/value.rs`: Runtime value representation
- `driver/operation.rs`: Driver operation trait

## Design Principles

1. **Zero Runtime Dependencies**: Core should define interfaces, not implementations
2. **Complete Type Information**: All type info needed for codegen must be here
3. **Visitor Pattern**: Use Visit/VisitMut for AST traversal operations
4. **Separation of Concerns**: App schema vs DB schema vs statements

## Recent Changes Analysis

From recent commits:
- Heavy refactoring to reduce glob imports for better clarity
- Removal of dead code and unused traits
- Unification of similar expression types (ExprField + ExprReference)
- Simplification of type system (removing unnecessary cast methods)
- Addition of new primitive types follows consistent patterns