# Toasty Change Guide

This guide explains how to make common changes to the Toasty codebase based on patterns observed in recent development.

## Quick Reference: Where Changes Go

| Change Type | Primary Crate | Also Update |
|------------|---------------|-------------|
| New primitive type | toasty-core | toasty-codegen, toasty-sql, all drivers |
| Query feature (ORDER BY, LIMIT) | toasty-core | toasty-codegen, toasty, toasty-sql |
| New model attribute | toasty-codegen | toasty-core (if new schema types needed) |
| Driver operation | toasty-core | toasty-driver-* |
| SQL syntax | toasty-sql | — |
| Relationship type | toasty | toasty-core, toasty-codegen |
| Optimization | toasty | — |

### Change Flow by Type

| Change Type | Flow |
|------------|------|
| New primitive type | toasty-core → codegen → sql → drivers |
| Query feature | core/stmt → codegen/expand → engine/simplify → engine/lower → engine/plan → sql |
| Model attribute | codegen/schema → codegen/expand |
| Database operation | core/driver → engine/plan → drivers |
| Optimization | engine/simplify → engine/lower → engine/plan |
| SQL generation | toasty-sql/serializer |

## Crate-Specific Patterns

### toasty-core

- **New data types**: Update `schema/app/field/primitive.rs`
- **Query features**: Add statement nodes in `stmt/`
- **Driver operations**: Define in `driver/operation.rs`
- **Visitor traits**: Update `stmt/visit.rs` and `stmt/visit_mut.rs`

### toasty-codegen

- New model features require updates to both schema parsing and expansion
- Query methods follow a builder pattern with method chaining
- Generated code must use fully qualified paths (e.g., `#toasty::`)

### toasty (engine)

- **Simplification**: Add rules in `engine/simplify/` (one file per expression type)
- **Lowering**: AST to HIR conversion in `engine/lower/`
- **Planning**: HIR to MIR in `engine/plan/`
- **Execution**: MIR to Actions in `engine/exec/`

### toasty-driver-*

- Drivers implement the `driver::Driver` trait from toasty-core
- SQL drivers use toasty-sql for query serialization
- NoSQL drivers (DynamoDB) have custom operation implementations
- Each driver defines capabilities that affect planning

### toasty-sql

- New SQL features require serializer updates
- Database-specific syntax handled via `Flavor` enum
- Parameter binding varies by database

## Common Change Patterns

### 1. Adding a New Primitive Type (e.g., i8, i16, i32)

**Example commits**: 054aa9b, c508bde, 08a9149, f68bf32, 740ec27

**Steps**:
1. **toasty-core/src/schema/app/field/primitive.rs**
   - Add to `Primitive` enum
   - Implement required traits

2. **toasty-core/src/stmt/ty.rs**
   - Add to `Type` enum
   - Update type methods

3. **toasty-core/src/stmt/value.rs**
   - Add to `Value` enum
   - Add conversion methods (`to_*`, `from_*`)

4. **toasty-codegen/src/schema/ty.rs**
   - Map Rust type to Primitive

5. **toasty-sql/src/serializer/ty.rs**
   - Map to SQL type per flavor

6. **Each driver's value.rs**
   - Add conversions to/from database types
   - Handle NULL cases

7. **tests/tests/tys.rs**
   - Add comprehensive tests

### 2. Adding a Query Feature (e.g., ORDER BY, LIMIT)

**Example commits**: 306b6e1, 417709c

**Steps**:
1. **toasty-core/src/stmt/**
   - Create new statement nodes (e.g., `order_by.rs`)
   - Update `Query` or `Select` struct

2. **toasty-core/src/stmt/visit*.rs**
   - Add visitor methods for new nodes
   - Update both `Visit` and `VisitMut` traits

3. **toasty-codegen/src/expand/query.rs**
   - Add builder methods to query struct

4. **toasty/src/engine/simplify/**
   - Add simplification rules if applicable
   - May need new module (e.g., `simplify/expr_*.rs`)

5. **toasty/src/engine/lower/**
   - Handle in lowering phase if needed
   - Consider how feature affects HIR construction

6. **toasty/src/engine/plan/statement.rs**
   - Handle in query planning (HIR → MIR)

7. **toasty/src/engine/mir/**
   - Add new MIR operations if needed
   - Update `operation.rs` enum

8. **toasty-sql/src/serializer/stmt.rs**
   - Generate SQL for feature

9. **Integration tests**
   - Test across all drivers

### 3. Refactoring/Cleanup

**Example commits**: a134613, 5a4974c, d75db43

**Common patterns**:
- **Import cleanup**: Replace `use module::*` with specific imports
- **Dead code removal**: Delete unused functions, traits, modules
- **Type consolidation**: Merge similar types (e.g., ExprField + ExprReference)
- **Macro extraction**: DRY up repetitive code with macros

**Process**:
1. Identify repetitive or unclear code
2. Make changes incrementally
3. Ensure all tests pass
4. Update dependent code
5. Remove old code

### 4. Driver Implementation

**Example commits**: a95a612, 8fef2e0

**Steps**:
1. **Create driver crate** if new database
2. **Implement Driver trait** from toasty-core (3 methods):
   - `capability()` - Return driver capabilities
   - `register_schema()` - Handle schema registration
   - `exec()` - Execute operations
3. **Handle operations** based on driver type:
   - **SQL operations**: `QuerySql`, `Insert`, `Update`, `Delete`
   - **Key-value operations**: `GetByKey`, `DeleteByKey`, `UpdateByKey`, `QueryPk`, `FindPkByIndex`
   - **Special**: `Transaction`
4. **Define capabilities** in `Capability` struct:
   - `auto_increment`, `returning`, `transactions`, `joins`, `subqueries`
   - Planner uses these to decide which operations to generate
5. **Value conversions** in value.rs
   - Convert between `toasty_core::stmt::Value` and database types
   - Handle NULL cases
6. **Integration tests** in tests/tests/
7. **CI configuration** for testing

### 5. Model/Schema Changes

**Example commits**: b197923, b91e7ef

**Steps**:
1. **toasty-codegen/src/schema/**
   - Parse new attributes
   - Update model building

2. **toasty-codegen/src/expand/**
   - Generate appropriate code
   - Use `#toasty::` qualified paths

3. **toasty/src/model.rs**
   - Update Model trait if needed

4. **Tests** for new functionality

## Architecture Rules

### Layer Dependencies
```
toasty-macros → toasty-codegen
                      ↓
toasty          → toasty-core ← toasty-driver-*
     ↓                ↓
toasty-sql ←─────────┘
```

### Code Generation Rules
1. Always use fully qualified paths: `#toasty::Type`
2. Generate minimal code - rely on trait implementations
3. Make generated code debuggable with clear names

### Statement Processing Pipeline

The engine uses multiple intermediate representations (IRs) to process statements:

```
User API → Statement Builder → AST
                                ↓
                            Simplify (normalize/optimize AST)
                                ↓
                            Lower (AST → HIR)
                                ↓
                            Plan (HIR → MIR)
                                ↓
                            Plan Execution (MIR → Actions)
                                ↓
                            Execute (interpreter loop with variable store)
                                ↓
                            Results
```

**Intermediate Representations:**
- **AST** - Abstract Syntax Tree (`toasty_core::stmt::Statement`)
- **HIR** - High-level IR tracking statement dependencies and argument flow (`engine/hir.rs`)
- **MIR** - Middle-level IR as operation DAG (`engine/mir/`)
- **Actions** - Executable instructions with input/output variables (`engine/exec/action.rs`)

**Key phases:**
- **Simplify**: Normalizes expressions, rewrites relationships to filters, lifts subqueries
- **Lower**: Converts AST to HIR, decomposes queries based on driver capabilities
- **Plan**: Converts HIR to MIR operation graph
- **Plan Execution**: Converts MIR to sequential actions with variable assignments
- **Execute**: Runs actions in interpreter loop, manages variable store lifecycle

### Testing Requirements
1. Unit tests in module
2. Integration tests across drivers
3. UI tests for compile errors
4. Concurrent execution support

## Recent Refactoring Trends

Based on recent development:

1. **Import Hygiene**: Moving from glob imports to specific imports
2. **Type System Simplification**: Removing unnecessary type methods
3. **Dead Code Elimination**: Removing unused features
4. **ID Generation**: Moving from const to dynamic IDs
5. **Test Infrastructure**: Better isolation and concurrency
6. **Intermediate Representations**: Introduction of HIR and MIR stages
7. **Modular Simplification**: Separate files per expression type (`simplify/expr_*.rs`)
8. **Typed Indices**: Using `StmtId`, `VarId`, `NodeId` instead of generic u32
9. **Stateful Visitors**: Visitors carry context for scope management

## Performance Patterns

### Zero-Cost Abstractions
- Generated code should have no runtime overhead
- Use const functions where possible
- Inline small functions

### Query Optimization
- Simplify before planning
- Use indexes when available
- Batch operations when possible

### Memory Management
- Stream results instead of collecting
- Reuse allocations via variable store
- Minimize string allocations in SQL generation

## Debugging Tips

1. **Enable debug assertions** to trigger verification (`engine/verify.rs`)
2. **Use `dbg!()` in simplification** to see AST transformations
3. **Use `dbg!()` in lowering** to see HIR construction
4. **Inspect MIR operation DAG** for planning issues
5. **Check variable store** for execution flow issues
6. **Print generated SQL** to debug serialization
7. **Check driver capabilities** for feature support
8. **Test with minimal examples** to isolate issues
9. **Use LoggingDriver** in tests to track operations

## Common Pitfalls

1. **Forgetting visitor methods** when adding statement nodes
   - Must update both `Visit` and `VisitMut` traits
2. **Not handling NULL** in value conversions
3. **Missing flavor handling** in SQL generation
4. **Incomplete simplification** leading to inefficient plans
5. **Not updating all drivers** when adding features
6. **Forgetting lowering transformations** for new AST nodes
7. **Not implementing MIR operations** for new features
8. **Not considering driver capabilities** during planning
   - Check `driver.capability()` to decide code paths
9. **Missing HIR dependency tracking** for multi-statement features

## Commit Message Conventions

Based on recent history:
- `feat:` New features
- `refactor:` Code cleanup/restructuring  
- `fix:` Bug fixes
- `test:` Test improvements
- `chore:` Build/tooling changes

Include PR number: `feat: Add feature (#123)`