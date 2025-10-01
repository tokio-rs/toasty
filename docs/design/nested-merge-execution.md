# Nested Merge Execution Plan Design

## Problem Statement

Toasty needs to efficiently execute eager loading of associations, including **recursive/nested eager loading** (e.g., `User.include(posts.include(tags))`). The query planner breaks statements into "materializations" - database queries that load rows. After materialization, we need to nest/associate these records according to the query structure.

**Example**: Load Users with Posts with Tags
- Query: `User.include(posts.include(tags))`
- Result structure: `[User { posts: [Post { tags: [Tag] }] }]`
- Materializations: (1) Users, (2) Posts for those Users, (3) Tags for those Posts
- Challenge: Efficiently construct deeply nested structure from flat materializations

Current limitations:
- Hard-coded to app-level schema patterns (BelongsTo/HasMany/HasOne)
- Only handles primary key associations
- Cannot handle composite keys or conditional associations
- No support for recursive nesting (multi-level includes)
- Bespoke implementation in `engine/exec/associate.rs` using HashMap indexing

Requirements:
- Work at db-level schema (tables/columns, not models/fields)
- Support arbitrary nesting conditions (composite keys, conditional filters)
- Support recursive/deep nesting (User -> Post -> Tag -> ...)
- Generate efficient execution plans for nested merges
- Support 1:1 and 1:N cardinality with different merge strategies
- Minimize allocations and iterations (inside-out nesting strategy)
- Parallelize materializations where possible

## PostgreSQL Join Execution Insights

From reviewing PostgreSQL's join executor (`nodeNestloop.c`, `nodeHashjoin.c`, `nodeMergejoin.c`):

1. **Multiple Join Strategies**: Postgres has three main join types:
   - **Nested Loop**: Simple iteration, good for small datasets or when outer has few rows
   - **Hash Join**: Build hash table on inner, probe with outer; optimal for equality joins
   - **Merge Join**: Requires sorted inputs, optimal when inputs already sorted

2. **Key Abstractions**:
   - **JoinState**: Base state with join type, join quals
   - **Join Clauses**: Separate representation of join conditions (equality, inequality)
   - **Tuple Slots**: Standardized way to hold current/matched tuples
   - **State Machine**: Track progress through join algorithm (need new outer, matched, etc.)

3. **Hash Join Specifics**:
   - Build phase: Hash inner relation into buckets
   - Probe phase: For each outer tuple, lookup matching inner tuples
   - Multi-batch support: Spill to disk when hash table exceeds memory
   - Skew handling: Optimize for non-uniform distributions

4. **Runtime Adaptivity**:
   - Can switch strategies based on actual row counts
   - Dynamic memory management (increase batches if needed)
   - Cost-based decisions at planning time, adaptive at runtime

## Key Insight: Inside-Out Nesting Strategy

For recursive nesting (User -> Posts -> Tags), we use an **inside-out** approach:

1. **Materialize all levels** (can parallelize independent branches):
   - Users (filtered by query conditions)
   - Posts (WHERE post_id IN users, using EXISTS subquery pattern)
   - Tags (WHERE tag_id IN posts, using EXISTS subquery pattern)

2. **Nest from deepest to shallowest**:
   - First: Merge Tags into Posts → `Posts-with-Tags`
   - Then: Merge Posts-with-Tags into Users → `Users-with-Posts-with-Tags`

3. **Benefits**:
   - Build each index once, use once, discard
   - Minimize allocations (each merge creates one new variable)
   - Natural topological ordering
   - Can optimize each merge independently

## Design Proposal: Nested Merge Plan

### Core Idea: Join-Like Merge Operations

Instead of treating association as a special case, model it as a **nested merge** operation similar to joins, but producing nested structures instead of flat tuples.

### 1. Plan Representation

Introduce new action types in the execution pipeline:

```rust
// In engine/plan/mod.rs

/// A nested merge operation that associates child records with parent records
pub(crate) struct NestedMerge {
    /// Source variable (parent records)
    source: VarId,

    /// Target variable (child records to merge in)
    target: VarId,

    /// Output variable (source with nested target)
    output: VarId,

    /// Field index in source record to populate
    field_index: usize,  // Index into the record's fields

    /// Merge strategy and configuration
    strategy: MergeStrategy,

    /// Cardinality of the association
    cardinality: MergeCardinality,
}

pub(crate) enum MergeStrategy {
    /// Build hash index on target, probe with source
    HashMerge {
        /// Columns in source to use as join keys (e.g., [user.id])
        source_keys: Vec<usize>,  // Column indices in source record

        /// Columns in target to use as join keys (e.g., [todo.user_id])
        target_keys: Vec<usize>,  // Column indices in target record

        /// Optional additional filter predicate
        filter: Option<eval::Func>,
    },

    /// Nested loop merge (for small datasets or complex predicates)
    NestedLoopMerge {
        /// Predicate to evaluate for each source/target pair
        /// Args: [source_record, target_record] -> bool
        predicate: eval::Func,
    },

    /// Merge sorted inputs (when both sides are pre-sorted)
    SortMerge {
        /// Source sort keys
        source_keys: Vec<usize>,

        /// Target sort keys
        target_keys: Vec<usize>,

        /// Sort direction (all ASC for now)
        direction: Vec<Direction>,
    },
}

pub(crate) enum MergeCardinality {
    /// 1:1 relationship (BelongsTo, HasOne)
    /// Target field receives single record or null
    One,

    /// 1:N relationship (HasMany)
    /// Target field receives list of records
    Many,
}
```

### 2. Planning Phase - Building the Merge DAG

In `engine/planner/partition.rs`, after materializations are computed:

```rust
/// Represents the dependency graph of nested merges
struct MergeGraph {
    /// All materializations (stmt_id -> materialization info)
    materializations: HashMap<StmtId, MaterializationInfo>,

    /// Merge operations to perform
    merges: Vec<MergeNode>,

    /// Topologically sorted merge order (inside-out)
    execution_order: Vec<usize>,  // Indices into `merges`
}

struct MergeNode {
    /// Unique ID for this merge operation
    id: usize,

    /// The merge action to execute
    action: plan::NestedMerge,

    /// Merge operations that must complete before this one
    /// (because they modify the target we're merging)
    dependencies: Vec<usize>,  // Indices into MergeGraph::merges
}

struct MaterializationInfo {
    /// The database query to execute
    query: stmt::Statement,

    /// Variable where results will be stored
    output_var: VarId,

    /// Nested substatements that depend on this materialization
    children: Vec<(usize, StmtId)>,  // (field_index, child_stmt_id)
}

impl Planner<'_> {
    pub(crate) fn plan_v2_stmt_query(&mut self, mut stmt: stmt::Statement, dst: plan::VarId) {
        // ... existing code to partition into statements ...

        // Build materialization graph
        let mut graph = MergeGraph::new();
        self.build_materialization_graph(&mut graph, StmtId(0));

        // Execute materializations (can parallelize independent ones)
        for mat_id in graph.materialization_order() {
            let mat = &graph.materializations[&mat_id];
            self.push_action(plan::ExecStatement {
                input: None,
                output: Some(plan::Output {
                    ty: /* ... */,
                    targets: vec![plan::OutputTarget {
                        var: mat.output_var,
                        project: /* ... */,
                    }],
                }),
                stmt: mat.query.clone(),
                conditional_update_with_no_returning: false,
            });
        }

        // Execute merges in topological order (inside-out)
        for merge_idx in &graph.execution_order {
            let merge_node = &graph.merges[*merge_idx];
            self.push_action(merge_node.action.clone());
        }

        // Final projection to output
        let final_var = graph.final_output_var();
        self.push_action(plan::Project {
            input: final_var,
            output: plan::Output {
                ty: /* ... */,
                targets: vec![plan::OutputTarget { var: dst, /* ... */ }],
            },
        });
    }

    fn build_materialization_graph(&mut self, graph: &mut MergeGraph, stmt_id: StmtId) {
        let stmt_state = &self.stmts[stmt_id.0];

        // Create materialization for this statement
        let output_var = self.var_table.register_var(/* ... */);
        graph.materializations.insert(stmt_id, MaterializationInfo {
            query: stmt_state.stmt.clone(),
            output_var,
            children: vec![],
        });

        // Recursively build materializations for children
        for (field_idx, child_stmt_id) in &stmt_state.children {
            self.build_materialization_graph(graph, *child_stmt_id);

            graph.materializations.get_mut(&stmt_id).unwrap()
                .children.push((*field_idx, *child_stmt_id));
        }

        // Create merge nodes for all children
        for (field_idx, child_stmt_id) in &stmt_state.children {
            self.build_merge_nodes(graph, stmt_id, *field_idx, *child_stmt_id);
        }
    }

    fn build_merge_nodes(
        &mut self,
        graph: &mut MergeGraph,
        parent_stmt: StmtId,
        field_idx: usize,
        child_stmt: StmtId,
    ) {
        // Analyze the join condition
        let join_analysis = self.analyze_join_condition(parent_stmt, child_stmt);

        // Determine source and target variables
        // For inside-out nesting:
        // - If child has no children: source = child_mat, target = child_mat
        // - If child has children: source = child_with_grandchildren, target = child_mat
        let (source_var, dependencies) = self.resolve_source_var(graph, child_stmt);
        let target_var = graph.materializations[&child_stmt].output_var;

        let merge_id = graph.merges.len();
        let output_var = self.var_table.register_var(/* ... */);

        let strategy = self.choose_merge_strategy(&join_analysis);

        graph.merges.push(MergeNode {
            id: merge_id,
            action: plan::NestedMerge {
                source: source_var,
                target: target_var,
                output: output_var,
                field_index: field_idx,
                strategy,
                cardinality: join_analysis.cardinality,
            },
            dependencies,
        });
    }

    fn resolve_source_var(
        &self,
        graph: &MergeGraph,
        stmt_id: StmtId,
    ) -> (VarId, Vec<usize>) {
        let children = &graph.materializations[&stmt_id].children;

        if children.is_empty() {
            // Leaf node - source is just the materialization
            (graph.materializations[&stmt_id].output_var, vec![])
        } else {
            // Has children - source is the result of merging all children
            // Find the last merge that produced this stmt with all its children
            let merge_idx = graph.merges.iter()
                .enumerate()
                .rev()
                .find(|(_, m)| {
                    // Find merge where source is this stmt
                    m.action.source == graph.materializations[&stmt_id].output_var
                })
                .map(|(idx, _)| idx)
                .expect("Children should have been merged already");

            (graph.merges[merge_idx].action.output, vec![merge_idx])
        }
    }

    fn choose_merge_strategy(&self, analysis: &JoinAnalysis) -> MergeStrategy {
        // Simple heuristic (can be sophisticated later)
        if analysis.is_equality_join() && analysis.estimated_target_rows > 10 {
            MergeStrategy::HashMerge {
                source_keys: analysis.source_columns.clone(),
                target_keys: analysis.target_columns.clone(),
                filter: analysis.additional_filter.clone(),
            }
        } else {
            MergeStrategy::NestedLoopMerge {
                predicate: analysis.to_predicate(),
            }
        }
    }
}

impl MergeGraph {
    fn new() -> Self {
        Self {
            materializations: HashMap::new(),
            merges: vec![],
            execution_order: vec![],
        }
    }

    /// Compute topological sort of merges (inside-out)
    fn compute_execution_order(&mut self) {
        // Kahn's algorithm for topological sort
        let mut in_degree: Vec<usize> = self.merges.iter()
            .map(|m| m.dependencies.len())
            .collect();

        let mut queue: Vec<usize> = in_degree.iter()
            .enumerate()
            .filter(|(_, &d)| d == 0)
            .map(|(idx, _)| idx)
            .collect();

        self.execution_order.clear();

        while let Some(merge_idx) = queue.pop() {
            self.execution_order.push(merge_idx);

            // Find merges that depend on this one
            for (idx, merge) in self.merges.iter().enumerate() {
                if merge.dependencies.contains(&merge_idx) {
                    in_degree[idx] -= 1;
                    if in_degree[idx] == 0 {
                        queue.push(idx);
                    }
                }
            }
        }

        assert_eq!(self.execution_order.len(), self.merges.len(), "Cycle in merge graph");
    }

    fn materialization_order(&self) -> Vec<StmtId> {
        // Can be parallelized, but for now just return all of them
        self.materializations.keys().cloned().collect()
    }

    fn final_output_var(&self) -> VarId {
        // The output of the last merge is the final result
        self.merges[*self.execution_order.last().unwrap()].action.output
    }
}

struct JoinAnalysis {
    // Join condition columns (indices into records)
    source_columns: Vec<usize>,
    target_columns: Vec<usize>,

    // Join type
    is_equality: bool,

    // Additional filter beyond join keys
    additional_filter: Option<eval::Func>,

    // Cardinality
    cardinality: MergeCardinality,

    // Statistics for strategy choice
    estimated_source_rows: usize,
    estimated_target_rows: usize,
}
```

### 2.1 Example: Planning User -> Posts -> Tags

Pseudocode for building the execution plan:

```
Given query: User.include(posts.include(tags))

Step 1: Partition into statements
  - Stmt0: SELECT * FROM users WHERE ...
  - Stmt1: SELECT * FROM posts WHERE EXISTS (SELECT 1 FROM [Stmt0] WHERE posts.user_id = users.id)
  - Stmt2: SELECT * FROM tags WHERE EXISTS (SELECT 1 FROM [Stmt1] WHERE tags.post_id = posts.id)

Step 2: Build materialization graph
  Materializations:
    - Mat0 (Stmt0): Users -> var_0
    - Mat1 (Stmt1): Posts -> var_1
    - Mat2 (Stmt2): Tags -> var_2

  Children:
    - Stmt0.children = [(field_idx: 5, Stmt1)]  // posts field
    - Stmt1.children = [(field_idx: 3, Stmt2)]  // tags field
    - Stmt2.children = []

Step 3: Build merge nodes (inside-out)
  Merge0: Tags into Posts
    - source: var_1 (Posts materialization)
    - target: var_2 (Tags materialization)
    - output: var_3 (Posts-with-Tags)
    - field_index: 3 (posts.tags)
    - strategy: HashMerge { source_keys: [0], target_keys: [1], ... }
    - dependencies: []

  Merge1: Posts-with-Tags into Users
    - source: var_3 (Posts-with-Tags from Merge0)
    - target: var_3 (reuse same var, it's the complete Posts now)
    - output: var_4 (Users-with-Posts-with-Tags)
    - field_index: 5 (users.posts)
    - strategy: HashMerge { source_keys: [0], target_keys: [2], ... }
    - dependencies: [0]  // Must run after Merge0

Step 4: Compute execution order
  Topological sort: [0, 1]

Step 5: Generate plan
  1. ExecStatement(query=Stmt0) -> var_0
  2. ExecStatement(query=Stmt1) -> var_1
  3. ExecStatement(query=Stmt2) -> var_2
  4. NestedMerge(Merge0) -> var_3
  5. NestedMerge(Merge1) -> var_4
  6. Project(var_4) -> dst
```

### 3. Execution Phase

In `engine/exec/nested_merge.rs` (new file):

```rust
impl Exec<'_> {
    pub(super) async fn action_nested_merge(
        &mut self,
        action: &plan::NestedMerge
    ) -> Result<()> {
        let mut source = self.vars.load(action.source).collect().await?;
        let target = self.vars.load(action.target).collect().await?;

        match &action.strategy {
            MergeStrategy::HashMerge { source_keys, target_keys, filter } => {
                self.exec_hash_merge(
                    &mut source,
                    &target,
                    action.field_index,
                    source_keys,
                    target_keys,
                    filter.as_ref(),
                    &action.cardinality,
                ).await?
            }
            MergeStrategy::NestedLoopMerge { predicate } => {
                self.exec_nested_loop_merge(
                    &mut source,
                    &target,
                    action.field_index,
                    predicate,
                    &action.cardinality,
                ).await?
            }
            MergeStrategy::SortMerge { .. } => {
                todo!("Sort merge - future optimization")
            }
        }

        self.vars.store(action.output, ValueStream::from_vec(source));
        Ok(())
    }

    async fn exec_hash_merge(
        &mut self,
        source: &mut Vec<stmt::Value>,
        target: &[stmt::Value],
        field_index: usize,
        source_keys: &[usize],
        target_keys: &[usize],
        filter: Option<&eval::Func>,
        cardinality: &MergeCardinality,
    ) -> Result<()> {
        // Build hash index on target
        let target_index = self.build_hash_index(target, target_keys)?;

        // Probe with source
        for source_item in source {
            let source_record = source_item.expect_record_mut();
            let key = self.extract_key(source_record, source_keys)?;

            // Lookup matching target records
            let matches = target_index.get(&key).map(|v| &**v).unwrap_or(&[]);

            // Apply optional filter
            let filtered_matches: Vec<_> = if let Some(filter) = filter {
                matches.iter()
                    .filter(|&&target_record| {
                        filter.eval_bool(&[source_item.clone(), target_record.clone()])
                            .unwrap_or(false)
                    })
                    .collect()
            } else {
                matches.iter().collect()
            };

            // Populate field based on cardinality
            match cardinality {
                MergeCardinality::One => {
                    source_record[field_index] = filtered_matches
                        .first()
                        .map(|&r| r.clone())
                        .unwrap_or(stmt::Value::Null);
                }
                MergeCardinality::Many => {
                    source_record[field_index] = stmt::Value::List(
                        filtered_matches.into_iter().cloned().collect()
                    );
                }
            }
        }

        Ok(())
    }

    fn build_hash_index(
        &self,
        records: &[stmt::Value],
        key_columns: &[usize],
    ) -> Result<HashMap<CompositeKey, Vec<stmt::Value>>> {
        let mut index = HashMap::new();

        for record in records {
            let record_inner = record.expect_record();
            let key = self.extract_key(record_inner, key_columns)?;
            index.entry(key).or_insert_with(Vec::new).push(record.clone());
        }

        Ok(index)
    }

    fn extract_key(
        &self,
        record: &stmt::ValueRecord,
        columns: &[usize],
    ) -> Result<CompositeKey> {
        let values: Vec<_> = columns
            .iter()
            .map(|&col_idx| record[col_idx].clone())
            .collect();
        Ok(CompositeKey(values))
    }

    async fn exec_nested_loop_merge(
        &mut self,
        source: &mut Vec<stmt::Value>,
        target: &[stmt::Value],
        field_index: usize,
        predicate: &eval::Func,
        cardinality: &MergeCardinality,
    ) -> Result<()> {
        // Simple nested loop - can optimize later with runtime heuristics
        for source_item in source {
            let source_record = source_item.expect_record_mut();
            let mut matches = Vec::new();

            for target_item in target {
                // Evaluate predicate(source, target)
                if predicate.eval_bool(&[source_item.clone(), target_item.clone()])? {
                    matches.push(target_item.clone());

                    // Early exit for One cardinality
                    if matches!(cardinality, MergeCardinality::One) {
                        break;
                    }
                }
            }

            // Populate field
            match cardinality {
                MergeCardinality::One => {
                    source_record[field_index] = matches
                        .into_iter()
                        .next()
                        .unwrap_or(stmt::Value::Null);
                }
                MergeCardinality::Many => {
                    source_record[field_index] = stmt::Value::List(matches);
                }
            }
        }

        Ok(())
    }
}

// Composite key type for multi-column joins
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CompositeKey(Vec<stmt::Value>);
```

### 3.1 Example: Executing User -> Posts -> Tags

Pseudocode for execution:

```
Given plan from section 2.1:
  1. ExecStatement(Users) -> var_0
  2. ExecStatement(Posts) -> var_1
  3. ExecStatement(Tags) -> var_2
  4. NestedMerge(Tags into Posts) -> var_3
  5. NestedMerge(Posts into Users) -> var_4

Execution trace:

Step 1: Execute materialization queries
  var_0 = [
    User { id: 1, name: "Alice", posts: NULL },
    User { id: 2, name: "Bob", posts: NULL },
  ]

  var_1 = [
    Post { id: 10, user_id: 1, title: "Post1", tags: NULL },
    Post { id: 11, user_id: 1, title: "Post2", tags: NULL },
    Post { id: 12, user_id: 2, title: "Post3", tags: NULL },
  ]

  var_2 = [
    Tag { id: 100, post_id: 10, name: "rust" },
    Tag { id: 101, post_id: 10, name: "async" },
    Tag { id: 102, post_id: 12, name: "performance" },
  ]

Step 4: NestedMerge(Tags into Posts)
  action = NestedMerge {
    source: var_1,
    target: var_2,
    field_index: 3,  // posts.tags field
    strategy: HashMerge {
      source_keys: [0],  // post.id
      target_keys: [1],  // tag.post_id
    },
    cardinality: Many,
  }

  Execution:
    1. Build hash index on Tags keyed by post_id:
       index = {
         10 -> [Tag{id:100}, Tag{id:101}],
         12 -> [Tag{id:102}],
       }

    2. Probe with Posts:
       For Post{id:10}: key=10, matches=[Tag{100}, Tag{101}]
         -> posts[0][3] = List([Tag{100}, Tag{101}])

       For Post{id:11}: key=11, matches=[]
         -> posts[1][3] = List([])

       For Post{id:12}: key=12, matches=[Tag{102}]
         -> posts[2][3] = List([Tag{102}])

    3. Store result:
       var_3 = [
         Post { id: 10, user_id: 1, title: "Post1", tags: [Tag{100}, Tag{101}] },
         Post { id: 11, user_id: 1, title: "Post2", tags: [] },
         Post { id: 12, user_id: 2, title: "Post3", tags: [Tag{102}] },
       ]

    4. Drop index (free memory)

Step 5: NestedMerge(Posts-with-Tags into Users)
  action = NestedMerge {
    source: var_0,
    target: var_3,  // Posts-with-Tags
    field_index: 5,  // users.posts field
    strategy: HashMerge {
      source_keys: [0],  // user.id
      target_keys: [1],  // post.user_id
    },
    cardinality: Many,
  }

  Execution:
    1. Build hash index on Posts-with-Tags keyed by user_id:
       index = {
         1 -> [Post{id:10, tags:[...]}, Post{id:11, tags:[]}],
         2 -> [Post{id:12, tags:[...]}],
       }

    2. Probe with Users:
       For User{id:1}: key=1, matches=[Post{10}, Post{11}]
         -> users[0][5] = List([Post{10, tags:[...]}, Post{11, tags:[]}])

       For User{id:2}: key=2, matches=[Post{12}]
         -> users[1][5] = List([Post{12, tags:[...]}])

    3. Store result:
       var_4 = [
         User {
           id: 1,
           name: "Alice",
           posts: [
             Post { id: 10, title: "Post1", tags: [Tag{100}, Tag{101}] },
             Post { id: 11, title: "Post2", tags: [] },
           ]
         },
         User {
           id: 2,
           name: "Bob",
           posts: [
             Post { id: 12, title: "Post3", tags: [Tag{102}] }
           ]
         },
       ]

    4. Drop index (free memory)

Final: Project var_4 to destination
```

### 4. Runtime Adaptivity (Future Enhancement)

```rust
impl Exec<'_> {
    fn choose_runtime_strategy(
        &self,
        planned: &MergeStrategy,
        source_count: usize,
        target_count: usize,
    ) -> MergeStrategy {
        // If dataset is tiny, skip hashing overhead
        if source_count * target_count < 20 {
            // Convert to nested loop
            return MergeStrategy::NestedLoopMerge { /* ... */ };
        }

        // Otherwise use planned strategy
        planned.clone()
    }
}
```

## Design Alternatives Considered

### Alternative 1: Outside-In Nesting

**Description**: Merge Posts into Users first, then merge Tags into the nested Posts.

**Pros**:
- More intuitive ordering (follows query structure)

**Cons**:
- Requires deep field path access (users[].posts[].tags)
- More complex implementation (need to traverse nested structures)
- Cannot easily reuse/share intermediate results

**Verdict**: Inside-out is simpler and more efficient.

### Alternative 2: Single-Pass Multi-Level Merge

**Description**: Build a single complex index structure and do all nesting in one pass.

**Pros**:
- Potentially fewer iterations over data

**Cons**:
- Complex index structure (multi-level hash maps)
- Hard to optimize each level independently
- Difficult to reason about and maintain
- Memory overhead of large index structures

**Verdict**: Multiple passes with simple indexes is clearer and more composable.

### Alternative 3: Implicit Association (Current Approach)

**Description**: Special-case association logic based on relationship types.

**Pros**:
- Simple for common cases
- Fewer abstractions

**Cons**:
- Hard to extend to complex join conditions
- Tightly coupled to app schema
- Cannot optimize based on data characteristics
- No support for recursive nesting

**Verdict**: Not scalable to requirements.

### Alternative 4: Correlated Subquery Execution (Chosen)

**Description**: Model nested selects as correlated subqueries, execute with join-like strategies, nest inside-out.

**Pros**:
- Matches Toasty's query model (nested selects in RETURNING)
- Flexible enough for arbitrary conditions
- Can apply database join optimizations
- Works at db-level schema
- Supports composite keys naturally
- Handles recursive nesting efficiently
- Simple, composable merge operations

**Cons**:
- More complex than current approach
- Need to implement multiple merge strategies
- Topological sort for execution order

**Verdict**: Best fit for requirements.

## Implementation Phases

### Phase 1: Basic Hash Merge (MVP)
- Implement `NestedMerge` action
- Support single-column equality joins
- Hash merge strategy only
- One and Many cardinality
- Single-level nesting only

**Validates**: Core architecture, db-level schema approach

### Phase 2: Recursive Nesting
- Build merge dependency graph
- Topological sort for inside-out execution
- Multi-level nesting (User -> Post -> Tag)

**Validates**: Handles complex nested structures

### Phase 3: Composite Keys
- Multi-column join keys
- Composite key hashing
- Update planning logic

**Validates**: Handles complex foreign keys

### Phase 4: Conditional Filters
- Additional filter predicates beyond join keys
- Filter evaluation in merge execution

**Validates**: Arbitrary nesting conditions

### Phase 5: Strategy Selection
- Nested loop merge implementation
- Planning-time strategy selection heuristics
- Cost estimation

**Validates**: Optimization framework

### Phase 6: Runtime Adaptivity
- Runtime strategy switching based on actual row counts
- Memory management for large datasets
- Performance monitoring

**Validates**: Production-ready execution

## Open Questions

1. **Parallel Materialization**:
   - Currently executing materializations sequentially
   - Can parallelize independent branches (e.g., User -> Posts AND User -> Comments)
   - Future optimization: build materialization dependency DAG

2. **Self-Referential Associations**: E.g., `Person.manager -> Person`
   - Same source and target materialization
   - Should work with current design (source and target can be same var)
   - Need test cases

3. **Many-to-Many (HasManyThrough)**:
   - User -> UserTags -> Tag (join through intermediate table)
   - Could model as: materialize UserTags, then two merges
   - Or: special case to skip materializing intermediate join table
   - Defer to later phase

4. **Multiple Associations on Same Model**:
   - User has_many Posts AND has_many Comments
   - Need multiple merge operations targeting different fields
   - Current design supports this (separate MergeNode for each)

5. **Ordering of Nested Results**:
   - Should nested lists be ordered?
   - Add ORDER BY to nested selects?
   - Could be separate feature

## Success Metrics

- [ ] Can handle single-level nesting (User -> Posts)
- [ ] Can handle recursive nesting (User -> Posts -> Tags)
- [ ] Works entirely at db-level schema
- [ ] Supports composite key associations
- [ ] Supports conditional filters on associations
- [ ] Inside-out execution minimizes allocations
- [ ] Performance comparable to current implementation for simple cases
- [ ] Can optimize based on data characteristics (runtime adaptivity)
- [ ] Extensible to new merge strategies

## References

- PostgreSQL join execution: `/Users/carllerche/Code/postgres/src/backend/executor/node{Nestloop,Hashjoin,Mergejoin}.c`
- Current Toasty association: `crates/toasty/src/engine/exec/associate.rs`
- Partition/materialization planning: `crates/toasty/src/engine/planner/partition.rs`
- EXISTS subquery pattern: `crates/toasty/src/engine/planner/partition.rs:450-466`