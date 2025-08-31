# Fix Include Lowering: Batch Lowering for Unified Type Handling

## Problem Statement

The current Toasty query engine has a fundamental **lowering mismatch** when handling queries with includes. The problem manifests as a type mismatch panic during execution, but the root cause is an architectural inconsistency in how different parts of the query pipeline handle type lowering.

### The Core Issue

**Include handling operates at the application level while the main query is lowered to the table level**, creating a boundary where data must be converted between representations at the wrong points in the pipeline.

Specifically:
1. **Main query**: Gets lowered from app-level to table-level (Model types → primitive types)
2. **Include queries**: Stay at app-level, expecting app-level inputs/outputs
3. **Data flow**: Table-level data from main query must be converted back to app-level for include queries
4. **Result**: Premature type conversions (Cast expressions) that break type validation

### Current Execution Flow (Broken)

```
1. Query with includes enters planner
   └─> Main query lowered (Model → String for IDs)
   
2. Main query executes
   └─> Database returns String values
   └─> Project function applies Cast(String → Id) 
   └─> Type validation expects String, gets Id
   └─> PANIC! ❌
   
3. Include queries (never reached)
   └─> Would expect app-level inputs (Id types)
   └─> Would return app-level outputs
```

### Why This Happens

The `table_to_model` expression in the schema includes Cast expressions to convert table-level types back to app-level types:

```rust
// In schema/builder/table.rs:606-608
stmt::Type::String if primitive.ty.is_id() => {
    stmt::Expr::cast(stmt::Expr::column(column.id), &primitive.ty)
}
```

These Cast expressions are necessary for the final result but are being evaluated too early - before include queries run and before type validation that expects lowered types.

## Solution: Batch Lowering

### Core Concept

Treat the main query and all include queries as a **single unit for lowering purposes**. All queries in the batch operate at the table level, with type conversions happening only at the very end of the pipeline.

### Key Principles

1. **Unified abstraction level**: All queries operate at table-level throughout execution
2. **Deferred conversion**: `table_to_model` conversions happen only after all queries complete
3. **Consistent type flow**: Data flows between queries without type conversions
4. **Batch planning**: The planner understands the full query tree before lowering

## Detailed Design

### 1. Query Tree Structure

First, we need to represent the full query tree including all includes:

```rust
/// Represents a complete query with all its includes
struct QueryTree {
    /// The main query
    main: stmt::Query,
    
    /// Include specifications from the main query
    includes: Vec<IncludeSpec>,
}

struct IncludeSpec {
    /// Path to the field being included (e.g., [2] for field index 2)
    path: stmt::Path,
    
    /// The model being queried
    target_model: ModelId,
    
    /// How to join back to parent (foreign key relationship)
    join_key: JoinKey,
    
    /// Nested includes for this level
    nested_includes: Vec<IncludeSpec>,
}

enum JoinKey {
    /// Parent has foreign key to child (BelongsTo)
    ParentFK { 
        parent_field: FieldId,
        child_pk: Vec<FieldId>,
    },
    /// Child has foreign key to parent (HasMany/HasOne)
    ChildFK {
        parent_pk: Vec<FieldId>,
        child_field: FieldId,
    },
}
```

### 2. Batch Lowering Process

The lowering process transforms the entire query tree at once:

```rust
/// Result of lowering a complete query tree
struct LoweredQueryBatch {
    /// Lowered main query
    main_query: LoweredQuery,
    
    /// Lowered include queries
    include_queries: Vec<LoweredInclude>,
    
    /// Final projection to convert table-level to app-level
    final_projection: eval::Func,
}

struct LoweredQuery {
    /// The lowered SQL statement
    stmt: stmt::Statement,
    
    /// Expected result type (table-level)
    result_type: stmt::Type,
}

struct LoweredInclude {
    /// Which field in parent this populates
    target_field: FieldId,
    
    /// The lowered query to fetch included data
    query: stmt::Statement,
    
    /// How to extract keys from parent records (table-level)
    parent_key_projection: Projection,
    
    /// How to extract keys from child records (table-level)
    child_key_projection: Projection,
    
    /// Nested includes for this level
    nested: Vec<LoweredInclude>,
}
```

### 3. Lowering Implementation

```rust
impl Planner {
    /// Lower a query with all its includes as a batch
    fn lower_query_batch(&mut self, query: stmt::Query) -> LoweredQueryBatch {
        // Step 1: Build the query tree
        let tree = self.build_query_tree(&query);
        
        // Step 2: Lower the main query WITHOUT table_to_model in returning
        let main_query = self.lower_main_query_for_batch(&tree.main);
        
        // Step 3: Lower all includes recursively
        let include_queries = self.lower_includes(&tree.includes, &main_query);
        
        // Step 4: Build final projection for table_to_model conversion
        let final_projection = self.build_final_projection(&tree);
        
        LoweredQueryBatch {
            main_query,
            include_queries,
            final_projection,
        }
    }
    
    /// Lower main query for batch execution (no table_to_model)
    fn lower_main_query_for_batch(&mut self, query: &stmt::Query) -> LoweredQuery {
        let mut lowered = query.clone();
        
        // Lower to table-level but keep returning as plain columns
        // NO Cast expressions for ID fields
        self.lower_to_table_level(&mut lowered);
        
        // Result type is pure table-level
        let result_type = self.compute_table_level_type(&lowered);
        
        LoweredQuery {
            stmt: stmt::Statement::Query(lowered),
            result_type,
        }
    }
    
    /// Lower include queries to operate on table-level data
    fn lower_includes(
        &mut self, 
        includes: &[IncludeSpec],
        parent_query: &LoweredQuery,
    ) -> Vec<LoweredInclude> {
        includes.iter().map(|spec| {
            // Generate query for this include
            let query = self.generate_include_query(spec, parent_query);
            
            // Lower it to table level
            let lowered = self.lower_to_table_level(query);
            
            // Determine key projections (at table level)
            let (parent_key, child_key) = self.compute_join_keys(spec, parent_query);
            
            // Recursively lower nested includes
            let nested = self.lower_includes(&spec.nested_includes, &lowered);
            
            LoweredInclude {
                target_field: spec.path.projection[0],
                query: lowered,
                parent_key_projection: parent_key,
                child_key_projection: child_key,
                nested,
            }
        }).collect()
    }
}
```

### 4. Batch Execution

Execute all queries with table-level types throughout:

```rust
impl Exec {
    async fn execute_batch(&mut self, batch: LoweredQueryBatch) -> Result<ValueStream> {
        // Step 1: Execute main query (returns table-level records)
        let mut main_results = self.execute_table_query(&batch.main_query).await?;
        
        // Step 2: Execute and associate includes (all at table-level)
        for include in &batch.include_queries {
            self.execute_and_associate_include(
                &mut main_results,
                include,
            ).await?;
        }
        
        // Step 3: Apply final projection (table_to_model conversion)
        let final_results = batch.final_projection.eval(&main_results)?;
        
        Ok(ValueStream::from_vec(final_results))
    }
    
    async fn execute_and_associate_include(
        &mut self,
        parent_records: &mut Vec<Value>,
        include: &LoweredInclude,
    ) -> Result<()> {
        // Extract keys from parent records (table-level)
        let parent_keys: Vec<Value> = parent_records
            .iter()
            .map(|record| include.parent_key_projection.apply(record))
            .collect();
        
        // Build and execute include query with parent keys
        let mut include_stmt = include.query.clone();
        include_stmt.substitute_keys(&parent_keys);
        
        // Execute (returns table-level records)
        let child_records = self.execute_table_query(&include_stmt).await?;
        
        // Associate at table level (no type conversion!)
        for parent in parent_records {
            let parent_key = include.parent_key_projection.apply(parent);
            
            let children: Vec<Value> = child_records
                .iter()
                .filter(|child| {
                    let child_key = include.child_key_projection.apply(child);
                    child_key == parent_key
                })
                .cloned()
                .collect();
            
            // Set the field (still at table level)
            parent.set_field(include.target_field, Value::List(children));
        }
        
        // Recursively handle nested includes
        for nested in &include.nested {
            // ... recursive execution
        }
        
        Ok(())
    }
}
```

### 5. Type Flow Through Pipeline

The types flow consistently through the entire pipeline:

```
Application Query
    ↓
[Batch Lowering]
    ↓
Table-Level Main Query → Execute → Table-Level Records
    ↓
Table-Level Include Queries → Execute → Table-Level Records
    ↓
Associate at Table Level
    ↓
[Final Projection]
    ↓
Application-Level Results
```

**Key Point**: No intermediate type conversions. All operations between lowering and final projection work with table-level types.

## Implementation Strategy

### Phase 1: Infrastructure
- Add `QueryTree` and `LoweredQueryBatch` structures
- Implement query tree building from includes

### Phase 2: Batch Lowering
- Implement `lower_query_batch` 
- Modify lowering to skip `table_to_model` in returning clause
- Build proper final projection

### Phase 3: Batch Execution
- Implement `execute_batch`
- Ensure all operations use table-level types
- Add table-level association logic

### Phase 4: Integration
- Route queries with includes through batch path
- Maintain backward compatibility for non-include queries
- Add comprehensive tests

## Benefits

1. **Type Consistency**: No type mismatches during execution
2. **Clear Boundaries**: Table-level operations are clearly separated from app-level
3. **Predictable Behavior**: Type conversions happen at one well-defined point
4. **Easier Debugging**: Can inspect data at each stage without type confusion
5. **Performance**: Potential for optimizing batch queries together

## Example

Consider a query with includes:

```rust
Person::filter_by_id(&id)
    .include(Person::FIELDS.children())
    .get(&db)
```

### Current (Broken) Flow:
1. Lower main query: Person fields become `[String, String, String, Null, Null]`
2. Execute main: Returns `[String("id"), String("name"), Null, Null, Null]`
3. Project applies Cast: `[Id("id"), String("name"), Null, Null, Null]` 
4. Type check fails: Expected String, got Id ❌

### New Batch Flow:
1. Build query tree with main + include for children
2. Lower both queries to table level
3. Execute main: Returns `[String("id"), String("name"), Null, Null, Null]`
4. Execute include: Returns children as `[String("id"), String("name"), ...]`
5. Associate at table level: `[String("id"), String("name"), Null, Null, List([...])]`
6. Apply final projection: Convert to `[Id("id"), String("name"), Null, Null, List([...])]`
7. Success! ✓

## Conclusion

Batch lowering solves the fundamental lowering mismatch by ensuring all queries in an include chain operate at the same abstraction level. This eliminates premature type conversions and ensures type validation happens at the correct level. The key insight is that **the entire query tree must be understood and lowered as a unit** rather than handling parts at different abstraction levels.