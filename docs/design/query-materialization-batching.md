# Query Materialization with Intelligent Batching

## Problem Statement

Toasty operates in a network-distributed environment where database queries are expensive due to network round-trips. When resolving correlation dependencies in complex queries, we need to:

1. **Materialize correlation contexts** to break dependencies
2. **Minimize network round-trips** through intelligent batching
3. **Avoid duplicate work** when multiple queries need overlapping data
4. **Share materialized results** across concurrent queries

This requires a sophisticated materialization strategy that goes beyond simple query partitioning.

## Core Challenge: Network Cost Optimization

Unlike PostgreSQL (in-process, shared memory), Toasty faces:
- **High latency**: Each query involves network round-trips
- **Bandwidth constraints**: Large result sets are expensive to transfer
- **Connection overhead**: Database connections are limited resources
- **Concurrency**: Multiple queries may need similar materialization simultaneously

## PostgreSQL-Inspired Algorithms

### 1. CTE-Style Shared Execution (Leader/Follower Pattern)

**PostgreSQL Pattern**: CTEs use shared tuplestores with multiple read pointers.

**Adaptation for Toasty**:
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

enum MaterializationState {
    Pending,        // No one has started yet
    InProgress,     // Leader is executing
    Completed,      // Results are available
    Failed,         // Execution failed
}
```

### 2. Query Fingerprinting for Batch Detection

**PostgreSQL Pattern**: Hash-based memoization in the Memoize node.

**Adaptation for Toasty**:
```rust
/// Identifies queries that can share materialization work
#[derive(Hash, Eq, PartialEq)]
struct MaterializationFingerprint {
    /// Tables being queried
    tables: BTreeSet<TableId>,

    /// WHERE conditions (normalized)
    filters: Vec<CanonicalFilter>,

    /// Required columns (superset for batching)
    projections: BTreeSet<ColumnId>,

    /// Parameter placeholders (for correlated queries)
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

### 3. Adaptive Materialization Strategy

**PostgreSQL Pattern**: Work memory-based decisions for materialization vs. streaming.

**Adaptation for Toasty**:
```rust
#[derive(Debug, Clone)]
enum MaterializationStrategy {
    /// Fully materialize and cache results
    FullMaterialize {
        cache_ttl: Duration,
        memory_limit: usize,
    },

    /// Partial materialization with streaming
    PartialMaterialize {
        batch_size: usize,
        max_batches: usize,
    },

    /// Stream-only (no caching under memory pressure)
    StreamOnly,

    /// Batch multiple queries into single database call
    BatchedExecution {
        batch_timeout: Duration,
        max_batch_size: usize,
    },
}

impl MaterializationStrategy {
    fn choose(context: &MaterializationContext) -> Self {
        let memory_pressure = context.current_memory_usage as f64 / context.memory_limit as f64;
        let reference_count = context.waiting_queries.len();
        let estimated_result_size = context.estimated_result_size;

        match (memory_pressure, reference_count, estimated_result_size) {
            // Low memory pressure, multiple consumers -> full materialization
            (p, refs, _) if p < 0.6 && refs > 1 => {
                MaterializationStrategy::FullMaterialize {
                    cache_ttl: Duration::from_secs(300),
                    memory_limit: (context.memory_limit as f64 * 0.1) as usize,
                }
            }

            // High memory pressure -> streaming
            (p, _, _) if p > 0.8 => MaterializationStrategy::StreamOnly,

            // Multiple similar queries -> batch execution
            (_, refs, size) if refs > 2 && size < 1_000_000 => {
                MaterializationStrategy::BatchedExecution {
                    batch_timeout: Duration::from_millis(100),
                    max_batch_size: refs.min(10),
                }
            }

            // Default to partial materialization
            _ => MaterializationStrategy::PartialMaterialize {
                batch_size: 1000,
                max_batches: 10,
            }
        }
    }
}
```

### 4. Work-Sharing Materialization Engine

**PostgreSQL Pattern**: Parallel hash joins with barrier synchronization.

**Adaptation for Toasty**:
```rust
struct MaterializationEngine {
    /// Currently active materializations
    active_materializations: DashMap<MaterializationFingerprint, SharedMaterialization>,

    /// Query batching coordinator
    batch_coordinator: Arc<BatchCoordinator>,

    /// Memory pressure monitor
    memory_monitor: Arc<MemoryMonitor>,

    /// Result cache with LRU eviction
    result_cache: Arc<RwLock<LruCache<MaterializationFingerprint, MaterializedData>>>,
}

impl MaterializationEngine {
    async fn materialize(&self, query: &Query) -> Result<MaterializedData> {
        let fingerprint = self.compute_fingerprint(query);

        // Check if we can reuse existing materialization
        if let Some(cached) = self.result_cache.read().get(&fingerprint) {
            return Ok(cached.filter_for_query(query));
        }

        // Check if we can join an in-progress materialization
        if let Some(shared) = self.active_materializations.get(&fingerprint) {
            return self.join_shared_materialization(shared, query).await;
        }

        // Look for overlapping queries we can batch with
        if let Some(batch_fingerprint) = self.find_batchable_queries(&fingerprint).await {
            return self.execute_batched_materialization(batch_fingerprint, query).await;
        }

        // Execute as new materialization
        self.execute_new_materialization(fingerprint, query).await
    }

    async fn find_batchable_queries(
        &self,
        fingerprint: &MaterializationFingerprint
    ) -> Option<MaterializationFingerprint> {
        // Wait briefly to see if similar queries arrive
        tokio::time::sleep(Duration::from_millis(10)).await;

        let mut batch_fingerprint = fingerprint.clone();
        let mut batch_size = 1;

        // Scan for queries we can merge with
        for (existing_fp, shared) in self.active_materializations.iter() {
            if let Ok(state) = shared.state.try_lock() {
                if matches!(*state, MaterializationState::Pending) {
                    if let Some(merged) = fingerprint.merge(existing_fp) {
                        batch_fingerprint = merged;
                        batch_size += shared.followers.len();

                        if batch_size >= 5 {
                            break; // Limit batch size
                        }
                    }
                }
            }
        }

        if batch_size > 1 {
            Some(batch_fingerprint)
        } else {
            None
        }
    }
}
```

### 5. Memory-Aware Batching Coordinator

**PostgreSQL Pattern**: Adaptive batching based on `work_mem` constraints.

**Adaptation for Toasty**:
```rust
struct BatchCoordinator {
    /// Pending queries waiting for batching
    pending_queries: Arc<Mutex<Vec<PendingQuery>>>,

    /// Batch execution timer
    batch_timer: Arc<Mutex<Option<Instant>>>,

    /// Current memory usage
    memory_usage: Arc<AtomicUsize>,

    /// Configuration
    config: BatchConfig,
}

#[derive(Debug, Clone)]
struct BatchConfig {
    /// Maximum time to wait for batching opportunities
    max_batch_delay: Duration,

    /// Maximum queries per batch
    max_batch_size: usize,

    /// Memory threshold for aggressive batching
    memory_pressure_threshold: f64,

    /// Maximum result set size for in-memory materialization
    max_materialization_size: usize,
}

impl BatchCoordinator {
    async fn submit_query(&self, query: Query) -> Result<MaterializedData> {
        let fingerprint = compute_fingerprint(&query);

        // Check if we should batch this query
        let should_batch = self.should_batch(&query, &fingerprint);

        if should_batch {
            self.add_to_batch(query, fingerprint).await
        } else {
            self.execute_immediately(query).await
        }
    }

    fn should_batch(&self, query: &Query, fingerprint: &MaterializationFingerprint) -> bool {
        // Don't batch if memory pressure is high
        let memory_pressure = self.memory_usage.load(Ordering::Relaxed) as f64 /
                              self.config.max_materialization_size as f64;

        if memory_pressure > self.config.memory_pressure_threshold {
            return false;
        }

        // Don't batch very large queries
        if query.estimated_result_size > self.config.max_materialization_size {
            return false;
        }

        // Look for similar pending queries
        let pending = self.pending_queries.lock().unwrap();
        pending.iter().any(|pending_query| {
            fingerprint.can_satisfy(&pending_query.fingerprint) ||
            pending_query.fingerprint.can_satisfy(fingerprint)
        })
    }
}
```

## Implementation Strategy for Toasty

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

### Integration with Existing Code

The materialization engine plugs into the existing `partition.rs` flow:

```rust
impl Planner<'_> {
    pub(crate) fn partition(&self, stmt: stmt::Query) -> Vec<stmt::Statement> {
        // Build dependency graph as before
        let dependency_graph = self.build_dependency_graph(stmt);

        // Instead of breaking cycles, identify materialization opportunities
        let correlations = self.extract_correlations(&dependency_graph);

        // Use materialization engine to optimize execution
        let materialization_plan = self.materialization_engine
            .plan_materializations(correlations)
            .await?;

        // Transform to executable statements
        self.build_execution_plan(materialization_plan)
    }
}
```

## Expected Performance Benefits

1. **Reduced Network Round-trips**: 30-70% reduction through batching
2. **Eliminated Duplicate Work**: Near 100% elimination for overlapping queries
3. **Memory Efficiency**: Adaptive strategies prevent memory exhaustion
4. **Improved Concurrency**: Shared execution reduces database connection pressure
5. **Predictable Performance**: Batching smooths out query latency spikes

## Monitoring and Observability

Track key metrics for tuning:
- Batch hit rate (queries batched vs. executed individually)
- Materialization cache hit rate
- Memory usage and pressure-triggered adaptations
- Network round-trip reduction percentage
- Query latency distribution (batched vs. individual)

This materialization strategy transforms Toasty from a simple query partitioner into an intelligent query optimization engine that minimizes network costs while preserving correctness.