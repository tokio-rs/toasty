# toasty Component Context  

## Purpose
The main runtime library providing the user-facing API, query engine, execution pipeline, and coordination between all components. This is where statements become database operations.

## Architecture Overview

### Query Execution Pipeline
1. **User API** → Statement builders
2. **Simplification** → Query optimization  
3. **Planning** → Convert to execution plan
4. **Execution** → Run against driver
5. **Results** → Stream of values

## Key Components

### Engine (`src/engine/`)
The heart of query execution:

#### Simplification (`simplify/`) - 15+ Optimization Rules
**Expression Optimizations:**
- AND/OR flattening and constant folding
- Binary operation optimization (cast removal, key rewriting)
- IN list optimization (empty→false, single→equality)
- NULL check optimization for ID fields
- String concatenation constant folding
- Record/List/Map expression simplification

**Advanced Query Optimizations:**
- Subquery lifting to joins (BelongsTo/HasOne)
- Primary key select extraction
- Association to filter conversion
- Root path expression rewriting
- Union flattening and optimization

#### Planner (`planner/`)
**Index Selection Algorithm:**
- Multi-phase cost-based index selection
- Supports equality, inequality, and range predicates
- OR/AND combination handling
- Partition key constraint enforcement
- Unique index prioritization

**Planning Components:**
- **select.rs**: Query planning with filter partitioning
- **insert.rs**: Insert with auto-increment handling
- **update.rs**: Read-modify-write transaction support
- **delete.rs**: Cascading delete planning
- **index.rs**: Cost-based index selection
- **relation.rs**: Relationship join planning
- **key.rs**: Key expression detection
- **lower.rs**: Model→table transformation

#### Executor (`exec/`)
**Variable Store System:**
- Slot-based variable management
- Stream semantics with move/copy operations
- Lazy evaluation support

**Execution Patterns:**
- Pipeline-based async execution
- Streaming result processing
- Transaction lifecycle management
- In-memory association resolution
- Batch operation grouping (future optimization)

### Statement Builders (`src/stmt/`)
High-level, type-safe statement construction:
- **select.rs**: Query builder implementation
- **insert.rs**: Insert builder with associations
- **update.rs**: Update builder with conditions
- **expr.rs**: Expression construction helpers
- **path.rs**: Model field path navigation

### Relations (`src/relation/`)
Relationship abstractions and loading:
- **has_many.rs**: One-to-many relationships
- **belongs_to.rs**: Many-to-one relationships  
- **has_one.rs**: One-to-one relationships
- Lazy loading and eager loading support

## Common Change Patterns

### Adding a Query Feature
1. Add builder method in `stmt/select.rs`
2. Implement simplification in `engine/simplify/`
3. Add planning logic in `engine/planner/select.rs`
4. Update execution in `engine/exec/`
5. Verify in `engine/verify.rs`

### Adding an Operation Type
1. Define plan node in `engine/plan/`
2. Implement planner in `engine/planner/`
3. Add executor in `engine/exec/`
4. Update `engine/plan/action.rs` enum

### Optimization Rules
1. Add simplification in `engine/simplify/`
2. Pattern match on statement structure
3. Return simplified version
4. Ensure correctness with tests

## Execution Flow Examples

### Simple Query
```
User::find_by_id(id)
→ Select with id filter
→ Simplify to direct key lookup
→ Plan as GetByKey operation
→ Execute against driver
→ Load single record
```

### Complex Query with Relations
```
User::all().todos().filter(...)
→ Select with join and filter
→ Lift subqueries, optimize filters
→ Plan with index selection
→ Execute as QueryPk or scan
→ Stream results
```

### Insert with Associations
```
User::create().todo(...)
→ Insert with nested inserts
→ Plan as batch operation
→ Execute transactionally
→ Return created entities
```

## Important Files

- `engine.rs`: Top-level execution entry point
- `engine/planner.rs`: Planning orchestration
- `engine/exec.rs`: Execution orchestration
- `db.rs`: Database connection and configuration
- `model.rs`: Model trait and ID generation

## Design Principles

1. **Lazy Evaluation**: Build up operations, execute on demand
2. **Stream Processing**: Return results as async streams
3. **Index Awareness**: Use indexes when available
4. **Driver Abstraction**: Work with any driver uniformly
5. **Type Safety**: Catch errors at compile time

## Performance Considerations

- Simplification eliminates unnecessary work
- Index selection avoids full scans
- Batch operations reduce round trips
- Variable store minimizes allocations
- Streaming prevents memory bloat

## Recent Changes Analysis

From recent commits:
- Added pagination support (LIMIT/OFFSET)
- Implemented ORDER BY functionality
- Support for more primitive types (i8, i16, i32, unsigned)
- Improved test infrastructure for concurrent execution
- Reduced glob imports throughout
- MySQL driver completion
- Read-modify-write (RMW) support for updates