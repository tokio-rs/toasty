# Unified Column References Design Document

## Executive Summary

This document proposes refactoring `ExprColumn` in `toasty-core` to eliminate direct column ID references (`ExprColumn::Column(ColumnId)`) and use only alias-based references (`ExprColumn::Alias`). This change will ensure all column references are properly scoped to their source tables, improving correctness and consistency in the query engine.

## Problem Statement

### Current State

The `ExprColumn` enum currently has two variants:

```rust
pub enum ExprColumn {
    /// Directly reference a column by ID
    Column(ColumnId),

    /// Reference a column aliased in FROM or equivalent clause
    Alias {
        nesting: usize,  // Query nesting level
        table: usize,    // Index in FROM clause
        column: usize,   // Column index in table
    },
}
```

### Issues with Current Design

1. **Semantic Incorrectness**: `ExprColumn::Column(ColumnId)` allows direct column references without scope context. Expressions can only correctly reference columns that are in scope through the statement's `Source`.

2. **Ambiguity**: Direct column IDs don't indicate which table instance they belong to, problematic for:
   - Self-joins
   - Subqueries
   - CTEs
   - Multiple table references

3. **Inconsistent Usage**: The codebase mixes both patterns:
   - Planner/lowering often uses `Column(ColumnId)` for simplicity
   - Complex queries (UPDATE with subqueries) use `Alias` for correctness
   - This inconsistency makes the codebase harder to understand

## Proposed Solution

### Core Change

Remove the `Column(ColumnId)` variant entirely, leaving only the alias-based variant:

```rust
pub enum ExprColumn {
    /// Reference a column through its source table
    Alias {
        nesting: usize,  // Query nesting level (0 for current)
        table: usize,    // Table index in source list
        column: usize,   // Column index within table
    },
}
```

Consider renaming to better reflect the new semantics:
- `ExprColumn::Scoped`
- `ExprColumn::TableColumn`
- Or just make it a struct since there's only one variant

### Rationale

1. **Correctness**: All column references must go through a source table, matching SQL semantics
2. **Consistency**: Single way to reference columns throughout the codebase
3. **Clarity**: Explicit scope makes queries easier to understand and debug
4. **Safety**: Prevents invalid column references at the type level

## Impact Analysis

### Components Affected

#### 1. Core Statement Layer (`toasty-core`)
- **Files**: `expr_column.rs`, `expr.rs`
- **Impact**: API changes, removal of convenience constructors
- **Usage**: ~16 files use `.column()` constructor

#### 2. Planner/Lowering (`toasty/engine/planner`)
- **Files**: `lower.rs`, `update.rs`, `insert.rs`, `select.rs`
- **Impact**: Must track table indices during lowering
- **Complexity**: Medium - needs source tracking infrastructure

#### 3. SQL Serialization (`toasty-sql`)
- **Files**: `serializer/expr.rs`, `serializer/stmt.rs`
- **Impact**: Already handles both variants, simplifies to one
- **Complexity**: Low - removal of code

#### 4. Database Drivers
- **Files**: `toasty-driver-dynamodb/src/lib.rs`, others
- **Impact**: Currently only handle `Column(ColumnId)`
- **Complexity**: Medium - need table context

#### 5. Tests
- **Files**: `one_model_crud_basic_driver_ops.rs`, others
- **Impact**: Test assertions need updating
- **Complexity**: Low - mechanical changes

### Fallout Severity

| Component | Severity | Files | Mitigation Strategy |
|-----------|----------|-------|-------------------|
| Core API | High | ~20 | Helper functions |
| Planner | Medium | ~10 | Source tracker |
| SQL Gen | Low | 2 | Simplification |
| Drivers | Medium | ~5 | Context passing |
| Tests | Low | ~10 | Mechanical update |

## Migration Strategy

### Phase 1: Add Infrastructure
1. Add source tracking to planner context
2. Create helper functions for common patterns
3. Add deprecation warnings on `Column(ColumnId)`

### Phase 2: Update Generators
1. Modify lowering to generate alias references
2. Update planner to maintain table indices
3. Ensure all generated statements use aliases

### Phase 3: Update Consumers
1. Fix SQL serialization (simplifies)
2. Update database drivers
3. Fix test assertions

### Phase 4: Remove Old Variant
1. Delete `Column(ColumnId)` variant
2. Remove deprecated helpers
3. Clean up unnecessary code

## Mitigation Strategies

### 1. Helper Functions

```rust
impl ExprColumn {
    /// Create column reference for single-table context
    pub fn simple(column: usize) -> Self {
        Self::Alias { nesting: 0, table: 0, column }
    }

    /// Create from table and column indices
    pub fn from_table(table: usize, column: usize) -> Self {
        Self::Alias { nesting: 0, table, column }
    }
}
```

### 2. Source Context Tracker

```rust
struct SourceContext {
    tables: Vec<TableId>,
    current_nesting: usize,
}

impl SourceContext {
    fn column(&self, table_id: TableId, column_idx: usize) -> ExprColumn {
        let table = self.tables.iter()
            .position(|t| t == &table_id)
            .expect("table not in source");

        ExprColumn::Alias {
            nesting: 0,
            table,
            column: column_idx,
        }
    }
}
```

### 3. Builder Pattern

```rust
struct ColumnRef {
    nesting: usize,
    table: usize,
    column: usize,
}

impl ColumnRef {
    fn new(column: usize) -> Self {
        Self { nesting: 0, table: 0, column }
    }

    fn with_table(mut self, table: usize) -> Self {
        self.table = table;
        self
    }

    fn with_nesting(mut self, nesting: usize) -> Self {
        self.nesting = nesting;
        self
    }

    fn build(self) -> ExprColumn {
        ExprColumn::Alias {
            nesting: self.nesting,
            table: self.table,
            column: self.column,
        }
    }
}
```

### 4. Macro for Tests

```rust
macro_rules! col_ref {
    ($column:expr) => {
        ExprColumn::Alias { nesting: 0, table: 0, column: $column }
    };
    ($table:expr, $column:expr) => {
        ExprColumn::Alias { nesting: 0, table: $table, column: $column }
    };
}
```

## Alternative Approaches Considered

### 1. Keep Both Variants with Validation
- **Pros**: No breaking changes, gradual migration
- **Cons**: Doesn't solve fundamental correctness issue
- **Verdict**: Rejected - maintains problematic pattern

### 2. Convert to Struct with Optional Table
- **Pros**: Single type, flexible
- **Cons**: Allows invalid states (no table when required)
- **Verdict**: Rejected - weakens type safety

### 3. Different Types for Different Contexts
- **Pros**: Type-safe contexts
- **Cons**: Complex type proliferation
- **Verdict**: Possible future enhancement

## Implementation Checklist

- [ ] Create design document (this document)
- [ ] Get design approval
- [ ] Add source tracking infrastructure
- [ ] Create helper functions and builders
- [ ] Update lowering to generate aliases
- [ ] Update SQL serialization
- [ ] Update database drivers
- [ ] Fix test assertions
- [ ] Remove `Column(ColumnId)` variant
- [ ] Update documentation

## Open Questions

1. **Naming**: Should we rename `ExprColumn` since it will only have one variant?
   - Options: `ColumnRef`, `ScopedColumn`, `TableColumn`
   - Or convert to a struct?

2. **Default Values**: For single-table queries, should we provide defaults?
   - Pro: Reduces boilerplate
   - Con: Hides important context

3. **Migration Timeline**: How aggressive should deprecation be?
   - Gradual with warnings
   - Or single breaking change

4. **Performance**: Will tracking table indices add overhead?
   - Likely negligible for planning phase
   - Benefits of correctness outweigh costs

## Success Metrics

1. **Correctness**: No more ambiguous column references
2. **Simplicity**: Reduced code paths in serialization
3. **Maintainability**: Single consistent pattern
4. **Type Safety**: Invalid references impossible to construct

## Conclusion

Refactoring `ExprColumn` to only use alias-based references will improve the correctness and consistency of the Toasty query engine. While there will be significant fallout, the proposed mitigation strategies (helper functions, source tracking, builders) will minimize the pain of migration. The end result will be a more robust and maintainable codebase that correctly handles all column scoping scenarios.