# ExprColumn Refactor Fallout Analysis

## Summary

This document analyzes the actual fallout from converting `ExprColumn` from an enum with two variants to a struct with only alias-based fields. The mechanical change reveals the true scope of work needed for proper alias-based column references.

## Change Made

```rust
// Before: Enum with two variants
pub enum ExprColumn {
    Column(ColumnId),           // Direct reference - REMOVED
    Alias { nesting, table, column },  // Scoped reference
}

// After: Struct with only scoped fields
pub struct ExprColumn {
    pub nesting: usize,    // Query nesting level
    pub table: usize,      // Table index in source
    pub column: usize,     // Column index in table
}
```

## Compilation Errors Found

### 1. Missing Methods (15+ errors)

**Lost `references()` method** - Used throughout planner:
- `crates/toasty/src/engine/planner/index.rs:336`
- `crates/toasty/src/engine/planner/insert.rs:342`
- `crates/toasty/src/engine/planner/key.rs:104`
- `crates/toasty/src/engine/planner/lower.rs:76,77,82`

**Lost `try_to_column_id()` method** - Used for type inference:
- `crates/toasty/src/engine/planner/select.rs:204`
- `crates/toasty/src/engine/ty.rs:18`

### 2. Pattern Matching Issues (5+ errors)

**`ExprColumn::Alias` no longer valid** since it's now a struct:
- `crates/toasty/src/engine/planner/update.rs:302,307,325,331,341`

**`ExprColumn::Column` no longer exists**:
- `crates/toasty/src/engine/planner/update.rs:227`

### 3. Construction Issues (5+ errors)

**No more `From<ColumnId>` trait**:
- `crates/toasty-core/src/schema/builder/table.rs:581,593,599`
- `crates/toasty/src/engine/planner/lower.rs:289`
- `crates/toasty-sql/src/stmt/create_index.rs:32`
- `crates/toasty-sql/src/stmt/create_table.rs:31`

### 4. Context Loss Issues (8+ errors)

**Type inference needs table context**:
- `crates/toasty-core/src/stmt/infer.rs:213` - Can't determine column type without table

**Schema builder needs table context**:
- `crates/toasty-core/src/schema/builder/table.rs` - Multiple locations

**Driver needs table context**:
- `crates/toasty-driver-dynamodb/src/lib.rs:330` - Can't resolve column without table

## Analysis by Component

### Core Statement Layer
- **Impact**: Medium
- **Issues**: Lost helper methods, construction from ColumnId
- **Files**: `expr_column.rs`, `infer.rs`, `schema/builder/table.rs`

### Planner/Engine
- **Impact**: HIGH
- **Issues**: 15+ compilation errors across all planner modules
- **Root Cause**: Heavy reliance on `references()` and `try_to_column_id()`
- **Files**: `index.rs`, `insert.rs`, `key.rs`, `lower.rs`, `select.rs`, `update.rs`, `ty.rs`

### SQL Generation
- **Impact**: Medium
- **Issues**: CREATE statements need table context
- **Files**: `create_index.rs`, `create_table.rs`
- **Note**: Expression serialization simplified (positive)

### Database Drivers
- **Impact**: Medium
- **Issues**: Lost ability to resolve column names from IDs
- **Files**: DynamoDB driver affected, others likely similar

### Tests
- **Impact**: Unknown (blocked by planner errors)
- **Expected**: Pattern matching assertions will fail

## Key Insights

### 1. Heavy Dependency on `references()` Method
The planner extensively uses `expr_column.references(column_id)` to check if a column expression references a specific database column. This was only meaningful for the `Column(ColumnId)` variant.

### 2. Type Inference Requires Schema Context
The `try_to_column_id()` method was used to get the column ID for type lookup in the schema. With alias-based references, type inference now requires both table context and schema.

### 3. Pattern Matching Throughout Planner
Many planner modules pattern match on `ExprColumn::Alias` to extract the `(nesting, table, column)` tuple. All of these need to change to struct field access.

### 4. Construction Sites Pervasive
Creating `ExprColumn` from `ColumnId` happens throughout the codebase, especially in:
- Schema builder (model-to-table lowering)
- Planner lowering phase
- SQL DDL statement generation

## Next Steps Priority

### Phase 1: Restore Basic Functionality
1. **Add back helper methods** with table context:
   ```rust
   impl ExprColumn {
       fn references_in_table(&self, table_id: TableId, column_id: ColumnId, schema: &Schema) -> bool
       fn resolve_column_id(&self, tables: &[TableId], schema: &Schema) -> Option<ColumnId>
   }
   ```

2. **Fix pattern matching** - Convert to struct field access

3. **Add table context tracking** to planner

### Phase 2: Systematic Fixes
1. **Update planner modules** one by one
2. **Fix schema builder** with table context
3. **Update SQL generation**
4. **Fix drivers** with table context

### Phase 3: Test and Validate
1. **Fix test assertions**
2. **Run full test suite**
3. **Validate query correctness**

## Mitigation Strategies Needed

### 1. Table Context Propagation
Need to thread table information through:
- Schema builder
- Planner lowering
- SQL DDL generation
- Driver operations

### 2. Helper Methods with Context
Replace lost methods with context-aware versions:
- `references()` → `references_in_context()`
- `try_to_column_id()` → `resolve_column_id()`

### 3. Source Tracking Infrastructure
Need systematic source table tracking in planner to map `(table_index, column_index)` back to schema entities.

## Conclusion

The mechanical change successfully demonstrates that removing direct column references forces proper scoping, but reveals the significant architectural work needed:

- **15+ compilation errors** across the planner
- **Table context required** throughout the stack
- **Helper methods need replacement** with context-aware versions
- **Source tracking infrastructure** needed in planner

This validates the design document's prediction of "significant fallout" while proving the change is necessary for correctness. The next phase should focus on adding the infrastructure needed to support alias-based references properly.