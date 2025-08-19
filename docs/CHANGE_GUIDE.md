# Toasty Change Guide

This guide explains how to make common changes to the Toasty codebase based on patterns observed in recent development.

## Quick Reference: Where Changes Go

| Change Type | Primary Locations |
|------------|------------------|
| New primitive type | toasty-core → codegen → sql → drivers |
| Query feature | core/stmt → codegen/expand → engine → sql |
| Model attribute | codegen/schema → codegen/expand |
| Database operation | core/driver → engine/plan → drivers |
| Optimization | engine/simplify → engine/planner |
| SQL generation | toasty-sql/serializer |

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

3. **toasty-codegen/src/expand/query.rs**
   - Add builder methods to query struct

4. **toasty/src/engine/simplify/**
   - Add simplification rules if applicable

5. **toasty/src/engine/planner/select.rs**
   - Handle in query planning

6. **toasty-sql/src/serializer/stmt.rs**
   - Generate SQL for feature

7. **Integration tests**
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
2. **Implement Driver trait** from toasty-core
3. **Handle operations**:
   - GetByKey, Insert, Update, Delete
   - Query operations based on capability
4. **Value conversions** in value.rs
5. **Integration tests** in tests/src/db/
6. **CI configuration** for testing

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
```
User API → Statement Builder → Simplify → Plan → Execute → Results
                                   ↓
                              Optimize/Rewrite
```

### Testing Requirements
1. Unit tests in module
2. Integration tests across drivers
3. UI tests for compile errors
4. Concurrent execution support

## Recent Refactoring Trends

Based on the last 40 commits:

1. **Import Hygiene**: Moving from glob imports to specific imports
2. **Type System Simplification**: Removing unnecessary type methods
3. **Dead Code Elimination**: Removing unused features
4. **ID Generation**: Moving from const to dynamic IDs
5. **Test Infrastructure**: Better isolation and concurrency

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

1. **Enable debug assertions** to trigger verification
2. **Use `dbg!()` in simplification** to see transformations
3. **Print generated SQL** to debug serialization
4. **Check driver capabilities** for feature support
5. **Test with minimal examples** to isolate issues

## Common Pitfalls

1. **Forgetting visitor methods** when adding statement nodes
2. **Not handling NULL** in value conversions
3. **Missing flavor handling** in SQL generation
4. **Incomplete simplification** leading to inefficient plans
5. **Not updating all drivers** when adding features

## Commit Message Conventions

Based on recent history:
- `feat:` New features
- `refactor:` Code cleanup/restructuring  
- `fix:` Bug fixes
- `test:` Test improvements
- `chore:` Build/tooling changes

Include PR number: `feat: Add feature (#123)`