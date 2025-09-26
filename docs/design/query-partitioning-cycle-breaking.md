# Query Dependency Resolution with Intelligent Materialization

## Problem Statement

The query planner needs to resolve correlation dependencies in complex queries while minimizing expensive network round-trips to the database. What initially appears as "cycles" in the dependency graph are actually correlation dependencies that can be resolved through intelligent materialization and batching strategies.

## Core Challenge: Network Cost Optimization

Unlike traditional SQL engines that operate in-process, Toasty faces:
- **High latency**: Each query involves network round-trips
- **Bandwidth constraints**: Large result sets are expensive to transfer
- **Connection overhead**: Database connections are limited resources
- **Concurrency**: Multiple queries may need similar materialization simultaneously

## Current Implementation Context

### Data Structures

```rust
struct State {
    stmts: Vec<StatementState>,
    edges: HashMap<(StmtId, StmtId), Edge>,  // (from, to) -> dependency info
    scopes: Vec<ScopeState>,
}

struct Edge {
    arg: usize,           // Parameter position in the dependent statement
    expr: stmt::Expr,     // Expression from the provider statement
    visited: bool,        // Used during graph traversal
}
```

### Current Approach

The existing code walks the query AST and builds a dependency graph by:
1. Creating new statements for nested subqueries
2. Recording dependencies when inner queries reference outer scope variables
3. Building a HashMap of edges representing these dependencies

## Key Insight: No True Cycles

Research into PostgreSQL's query engine reveals that what appears as "cycles" are actually **correlation dependencies**:

- **Correlation dependencies**: Inner queries referencing outer query variables
- **Not true execution cycles**: The execution flow remains uni-directional
- **Materialization opportunity**: Dependencies can be resolved by materializing correlation contexts

### Revised Problem Statement

The problem is better framed as:

**"How do we transform complex queries with correlation dependencies into a sequence of executable statements that minimizes network round-trips through intelligent batching?"**

## Dependency Resolution Example: From Cycles to Parameters

Let's work through the canonical example to understand how dependency resolution works in practice:

### Original "Cyclic" Query
```sql
SELECT * FROM users u
WHERE u.id IN (
  SELECT o.user_id FROM orders o
  WHERE o.total > (
    SELECT AVG(total) FROM orders o2
    WHERE o2.user_id = u.id  -- This creates the "cycle"
  )
)
```

### Dependency Analysis

**Statement Breakdown:**
- **S1 (Outer)**: `SELECT * FROM users u WHERE u.id IN (...)`
- **S2 (Middle)**: `SELECT o.user_id FROM orders o WHERE o.total > (...)`
- **S3 (Inner)**: `SELECT AVG(total) FROM orders o2 WHERE o2.user_id = u.id`

**Dependencies:**
- S2 depends on S3 (needs the AVG result)
- S3 depends on S1 (needs `u.id` from the outer query)
- S1 depends on S2 (needs the user_id list)

**The "Cycle"**: S1 → S2 → S3 → S1

### Toasty's Materialization Approach

Since Toasty operates over networks with expensive round-trips, materialization with intelligent batching is the optimal strategy:

## Filter Transformation Analysis

The key challenge is transforming correlation-dependent filters into materialized equivalents. Let's examine how this works with increasing complexity:

### Simple Case: Direct Column Reference

**Original correlated query:**
```sql
SELECT AVG(total) FROM orders o2 WHERE o2.user_id = u.id
```

**Transformation process:**
1. **Identify correlation variable**: `u.id` (references outer scope)
2. **Extract correlation constraint**: `o2.user_id = u.id`
3. **Replace with IN clause**: `o2.user_id IN ({materialized_user_ids})`
4. **Add GROUP BY**: `GROUP BY user_id` (to maintain per-user semantics)

**Result:**
```sql
SELECT user_id, AVG(total) as avg_total
FROM orders o2
WHERE o2.user_id IN ({user_ids_context})
GROUP BY user_id
```

### Complex Case 1: Multiple Correlation Variables

**Original correlated query:**
```sql
SELECT COUNT(*) FROM orders o2
WHERE o2.user_id = u.id
  AND o2.created_at > u.registration_date
  AND o2.status = 'completed'
```

**Transformation process:**
1. **Identify correlation variables**: `u.id`, `u.registration_date`
2. **Materialize correlation context**: `SELECT id, registration_date FROM users`
3. **Transform correlation conditions**:
   - `o2.user_id = u.id` → `o2.user_id = context.id`
   - `o2.created_at > u.registration_date` → `o2.created_at > context.registration_date`
4. **Keep non-correlation conditions**: `o2.status = 'completed'` (unchanged)

**Result:**
```sql
-- Materialization context
SELECT id, registration_date FROM users

-- Transformed query (using JOIN with materialized context)
SELECT context.id as user_id, COUNT(*) as order_count
FROM orders o2
JOIN user_context context ON o2.user_id = context.id
WHERE o2.created_at > context.registration_date
  AND o2.status = 'completed'
GROUP BY context.id, context.registration_date
```

### Complex Case 2: Nested Correlation with Complex Logic

**Original correlated query:**
```sql
SELECT product_id FROM order_items oi
WHERE oi.order_id IN (
  SELECT o.id FROM orders o
  WHERE o.user_id = u.id
    AND o.total > (u.credit_limit * 0.8)
    AND o.created_at BETWEEN u.registration_date AND u.last_login
)
AND oi.quantity > 1
```

**Transformation process:**
1. **Identify correlation variables**: `u.id`, `u.credit_limit`, `u.registration_date`, `u.last_login`
2. **Analyze correlation expressions**:
   - `o.user_id = u.id` → Direct reference
   - `o.total > (u.credit_limit * 0.8)` → Expression with correlation variable
   - `o.created_at BETWEEN u.registration_date AND u.last_login` → Range with correlation variables

**Result transformation algorithm:**
```rust
// Pseudocode for transformation logic
fn transform_correlated_filter(
    original_filter: &FilterExpr,
    correlation_vars: &[CorrelationVar]
) -> TransformedFilter {
    match original_filter {
        FilterExpr::Equals(left, right) => {
            if let Some(corr_var) = extract_correlation_var(right) {
                // Transform: col = corr_var → col = materialized_context.corr_var
                TransformedFilter::Join {
                    condition: JoinCondition::Equals(left.clone(), corr_var),
                    materialized_context: corr_var.source_table,
                }
            }
        }

        FilterExpr::GreaterThan(left, right) => {
            if let Some(expr) = extract_correlation_expression(right) {
                // Transform: col > (corr_var * 0.8) → col > (materialized_context.corr_var * 0.8)
                TransformedFilter::Join {
                    condition: JoinCondition::GreaterThan(
                        left.clone(),
                        rewrite_expression_with_context(expr)
                    ),
                    materialized_context: expr.source_table,
                }
            }
        }

        FilterExpr::Between(col, start, end) => {
            if has_correlation_vars(start) || has_correlation_vars(end) {
                // Transform range conditions with correlation variables
                TransformedFilter::Join {
                    condition: JoinCondition::Between(
                        col.clone(),
                        rewrite_expression_with_context(start),
                        rewrite_expression_with_context(end)
                    ),
                    materialized_context: extract_source_table(start, end),
                }
            }
        }

        FilterExpr::And(filters) => {
            // Recursively transform each filter in the AND clause
            let transformed = filters.iter()
                .map(|f| transform_correlated_filter(f, correlation_vars))
                .collect();
            TransformedFilter::And(transformed)
        }

        FilterExpr::In(col, subquery) => {
            // Recursively transform the subquery
            if subquery.has_correlation_dependencies() {
                let transformed_subquery = transform_correlated_subquery(subquery);
                TransformedFilter::In(col.clone(), transformed_subquery)
            }
        }

        // Non-correlation filters remain unchanged
        _ => TransformedFilter::Unchanged(original_filter.clone())
    }
}
```

**Final transformed query:**
```sql
-- Materialization context
SELECT id, credit_limit, registration_date, last_login FROM users

-- Transformed query
SELECT DISTINCT context.id as user_id, oi.product_id
FROM order_items oi
JOIN orders o ON oi.order_id = o.id
JOIN user_context context ON o.user_id = context.id
WHERE o.total > (context.credit_limit * 0.8)
  AND o.created_at BETWEEN context.registration_date AND context.last_login
  AND oi.quantity > 1
GROUP BY context.id
```

### Complex Case 3: Correlation in Aggregation Functions

**Original correlated query:**
```sql
SELECT SUM(CASE
  WHEN oi.unit_price > u.avg_purchase_amount THEN oi.quantity * oi.unit_price
  ELSE 0
END) as premium_total
FROM order_items oi
JOIN orders o ON oi.order_id = o.id
WHERE o.user_id = u.id
```

**Transformation challenges:**
1. **Correlation in CASE expression**: `u.avg_purchase_amount` used in comparison
2. **Aggregation with correlation**: `SUM()` depends on correlation context

**Result:**
```sql
-- Materialization context
SELECT id, avg_purchase_amount FROM users

-- Transformed query
SELECT context.id as user_id,
       SUM(CASE
         WHEN oi.unit_price > context.avg_purchase_amount
         THEN oi.quantity * oi.unit_price
         ELSE 0
       END) as premium_total
FROM order_items oi
JOIN orders o ON oi.order_id = o.id
JOIN user_context context ON o.user_id = context.id
GROUP BY context.id, context.avg_purchase_amount
```

## Real-World Example: Nested Correlation with Multiple Levels

Let's apply our materialization strategy to a complex real-world query:

**Original Query:**
```sql
SELECT name
FROM users
WHERE 5 < (
  SELECT count(*)
  FROM categories
  WHERE categories.id IN (
    SELECT posts.category_id
    FROM posts
    WHERE posts.user_id = users.id  -- Correlation dependency!
  )
)
```

### Step 1: Dependency Analysis

**Statement breakdown:**
- **S1 (Outer)**: `SELECT name FROM users WHERE 5 < (...)`
- **S2 (Middle)**: `SELECT count(*) FROM categories WHERE categories.id IN (...)`
- **S3 (Inner)**: `SELECT posts.category_id FROM posts WHERE posts.user_id = users.id`

**Dependencies:**
- S3 depends on S1 (needs `users.id` from outer query)
- S2 depends on S3 (needs the category_id list)
- S1 depends on S2 (needs the count result)

**Correlation variables:**
- `users.id` referenced in S3's WHERE clause

### Step 2: Correlation Context Materialization

**Identify what needs to be materialized:**
```sql
-- All users that could potentially satisfy the condition
SELECT id FROM users
```

### Step 3: Transform Inner Query (S3)

**Original correlated query:**
```sql
SELECT posts.category_id FROM posts WHERE posts.user_id = users.id
```

**Materialized transformation:**
```sql
-- For ALL users at once
SELECT user_id, category_id
FROM posts
WHERE user_id IN ({materialized_user_ids})
```

**Result structure:**
```
user_id | category_id
--------|------------
1       | 10
1       | 15
2       | 10
2       | 20
3       | 25
...
```

### Step 4: Transform Middle Query (S2)

**Original:**
```sql
SELECT count(*) FROM categories WHERE categories.id IN (
  SELECT posts.category_id FROM posts WHERE posts.user_id = users.id
)
```

**Materialized transformation:**
```sql
-- Count categories per user using materialized data
SELECT user_posts.user_id, COUNT(*) as category_count
FROM (
  SELECT user_id, category_id
  FROM posts
  WHERE user_id IN ({materialized_user_ids})
) user_posts
JOIN categories c ON c.id = user_posts.category_id
GROUP BY user_posts.user_id
```

**Result structure:**
```
user_id | category_count
--------|---------------
1       | 2
2       | 2
3       | 1
...
```

### Step 5: Transform Outer Query (S1)

**Original:**
```sql
SELECT name FROM users WHERE 5 < (SELECT count(*) FROM ...)
```

**Materialized transformation:**
```sql
-- Final query using materialized counts
SELECT u.name
FROM users u
JOIN user_category_counts ucc ON u.id = ucc.user_id
WHERE 5 < ucc.category_count
```

### Complete Materialized Execution Plan

```sql
-- Step 1: Materialize user context
CREATE TEMP TABLE user_context AS
SELECT id FROM users;

-- Step 2: Materialize user->category relationships
CREATE TEMP TABLE user_category_posts AS
SELECT user_id, category_id
FROM posts
WHERE user_id IN (SELECT id FROM user_context);

-- Step 3: Count categories per user
CREATE TEMP TABLE user_category_counts AS
SELECT ucp.user_id, COUNT(DISTINCT c.id) as category_count
FROM user_category_posts ucp
JOIN categories c ON c.id = ucp.category_id
GROUP BY ucp.user_id;

-- Step 4: Final result
SELECT u.name
FROM users u
JOIN user_category_counts ucc ON u.id = ucc.user_id
WHERE 5 < ucc.category_count;
```

### Optimization: Single Query Approach

For better performance, we can combine steps 2-3:

```sql
-- Step 1: Materialize user context (if needed for batching)
user_context = SELECT id FROM users;

-- Step 2: Combined materialization and counting
CREATE TEMP TABLE user_category_counts AS
SELECT p.user_id, COUNT(DISTINCT c.id) as category_count
FROM posts p
JOIN categories c ON c.id = p.category_id
WHERE p.user_id IN ({user_context})  -- or just remove this if scanning all users
GROUP BY p.user_id;

-- Step 3: Final result
SELECT u.name
FROM users u
JOIN user_category_counts ucc ON u.id = ucc.user_id
WHERE 5 < ucc.category_count;
```

### Key Transformation Patterns Identified

1. **Nested IN clause flattening**:
   - `categories.id IN (SELECT ...)` becomes a JOIN with materialized results

2. **Correlation variable propagation**:
   - `posts.user_id = users.id` becomes `posts.user_id IN ({user_context})`

3. **Aggregation preservation**:
   - `SELECT count(*)` becomes `COUNT(DISTINCT ...)` with GROUP BY

4. **Condition restructuring**:
   - `WHERE 5 < (subquery)` becomes `WHERE 5 < materialized_result`

### Performance Benefits

- **1 → 3 queries**: Instead of N+1 queries (one per user), we execute 3 total queries
- **Batch processing**: All users processed simultaneously
- **Set operations**: Leverages database JOIN optimizations
- **Index utilization**: Better index usage on `posts.user_id` and `categories.id`

### Memory Considerations

- **Temporary storage**: Need to store intermediate results
- **Batching strategy**: For large user sets, batch the materialization
- **Selective materialization**: Only materialize users that might satisfy outer conditions

This example demonstrates how complex nested correlations can be systematically transformed into efficient materialized queries while preserving semantics.

## Intelligent Materialization Strategy

The materialization approach is enhanced with batching optimizations inspired by PostgreSQL:

### 1. Query Fingerprinting for Batch Detection

```rust
/// Identifies queries that can share materialization work
#[derive(Hash, Eq, PartialEq)]
struct MaterializationFingerprint {
    tables: BTreeSet<TableId>,
    filters: Vec<CanonicalFilter>,
    projections: BTreeSet<ColumnId>,
    parameters: Vec<ParameterType>,
}

impl MaterializationFingerprint {
    /// Check if this fingerprint can satisfy another query's needs
    fn can_satisfy(&self, other: &Self) -> bool {
        self.tables.is_superset(&other.tables) &&
        self.projections.is_superset(&other.projections) &&
        self.filters_subsume(&other.filters)
    }

    /// Merge two fingerprints for batching
    fn merge(&self, other: &Self) -> Option<Self> {
        if self.tables != other.tables {
            return None; // Can't batch across different table sets
        }

        Some(MaterializationFingerprint {
            tables: self.tables.clone(),
            filters: self.merge_filters(&other.filters),
            projections: &self.projections | &other.projections,
            parameters: self.merge_parameters(&other.parameters),
        })
    }
}
```

### 2. Shared Execution with Leader/Follower Pattern

```rust
struct SharedMaterialization {
    /// Query that will drive the materialization
    leader_query: QueryId,

    /// All queries waiting for this materialization
    followers: Vec<QueryId>,

    /// Shared result cache
    results: Arc<RwLock<Option<MaterializedData>>>,

    /// Notification for completion
    completion_notify: Arc<Notify>,

    /// Execution state
    state: Arc<Mutex<MaterializationState>>,
}
```

### 3. Adaptive Materialization Strategy

```rust
enum MaterializationStrategy {
    FullMaterialize { cache_ttl: Duration, memory_limit: usize },
    PartialMaterialize { batch_size: usize, max_batches: usize },
    StreamOnly,
    BatchedExecution { batch_timeout: Duration, max_batch_size: usize },
}

impl MaterializationStrategy {
    fn choose(context: &MaterializationContext) -> Self {
        let memory_pressure = context.current_memory_usage as f64 / context.memory_limit as f64;
        let reference_count = context.waiting_queries.len();

        match (memory_pressure, reference_count) {
            (p, refs) if p < 0.6 && refs > 1 => MaterializationStrategy::FullMaterialize { ... },
            (p, _) if p > 0.8 => MaterializationStrategy::StreamOnly,
            (_, refs) if refs > 2 => MaterializationStrategy::BatchedExecution { ... },
            _ => MaterializationStrategy::PartialMaterialize { ... }
        }
    }
}
```

### 4. Integration with Existing Code

```rust
impl Planner<'_> {
    pub(crate) fn resolve_dependencies(&self, stmt: stmt::Query) -> Vec<stmt::Statement> {
        let dependency_graph = self.build_dependency_graph(stmt);

        // Identify correlation dependencies (not cycles)
        let correlations = self.extract_correlations(&dependency_graph);

        // Use materialization engine with batching optimization
        let materialization_plan = self.materialization_engine
            .plan_materializations(correlations)
            .await?;

        self.build_execution_plan(materialization_plan)
    }
}
```

## Implementation Strategy

### Phase 1: Basic Materialization
1. Implement `MaterializationFingerprint` for query analysis
2. Add shared result caching with simple LRU eviction
3. Implement basic leader/follower pattern for duplicate query elimination

### Phase 2: Intelligent Batching
1. Add query fingerprint merging for batch detection
2. Implement time-based batching with configurable delays
3. Add memory pressure monitoring and adaptive strategies

### Phase 3: Advanced Optimization
1. Implement parallel materialization for independent queries
2. Add predictive batching based on query patterns
3. Implement cross-query optimization (join ordering, index usage)

## Expected Performance Benefits

1. **Reduced Network Round-trips**: 30-70% reduction through batching
2. **Eliminated Duplicate Work**: Near 100% elimination for overlapping queries
3. **Memory Efficiency**: Adaptive strategies prevent memory exhaustion
4. **Improved Concurrency**: Shared execution reduces database connection pressure
5. **Predictable Performance**: Batching smooths out query latency spikes

## Key Design Principles

1. **Network-First Optimization**: Every decision prioritizes minimizing network round-trips
2. **Adaptive Behavior**: Strategies adapt to memory pressure and concurrent query load
3. **Shared Work**: Multiple queries collaborate to reduce total database load
4. **Correctness Preservation**: All optimizations maintain original query semantics