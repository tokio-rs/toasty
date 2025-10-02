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

### Core Idea: Per-Record Filtering and Projection

Instead of treating association as a special case, model it as a **nested merge** operation that:
1. Takes materialized root records and materialized nested records
2. For each root record, filters the nested materialization using merge qualifications
3. Stores the filtered nested records as the ExprArg input (referenced in returning clause)
4. Projects the final result using the returning clause projection

This produces nested structures by processing each root record individually.

### 1. Plan Representation

Introduce new action types in the execution pipeline:

```rust
// In engine/plan/mod.rs

/// A nested merge operation that associates child records with parent records
///
/// A single NestedMerge handles the ENTIRE nesting hierarchy for a root statement,
/// not just one level. This allows nested qualifications to reference any ancestor
/// in the context stack.
///
/// Example: User -> Posts -> Tags where Tags references both Post and User
/// The NestedMerge for Users contains the full hierarchy:
///   - Posts (references User)
///     - Tags (references Post AND User)
///
/// Execution is outside-in with ancestor context:
///   For each User:
///     For each Post belonging to this User:
///       For each Tag belonging to this Post AND this User:
///         Add Tag to Post
///     Add Post to User
pub(crate) struct NestedMerge {
    /// Root materialization variable (parent records)
    root: VarId,

    /// Nested hierarchy - children and their descendants
    /// Multiple entries at this level = siblings (e.g., User has Posts AND Comments)
    nested: Vec<NestedLevel>,

    /// Indexes to build upfront before execution
    /// Map from VarId (source data) to columns to index by
    /// Built during planning, used during execution
    indexes: HashMap<VarId, Vec<usize>>,

    /// Output variable (projected result with nested structure)
    output: VarId,

    /// Final projection to apply at root level
    /// Args: [root_record, filtered_collection_0, filtered_collection_1, ...]
    /// The filtered collections are bound to ExprArgs in the returning clause
    projection: eval::Func,
}

/// A single level in the nesting hierarchy
///
/// Each level represents one child relationship and contains:
/// - How to load the child data
/// - How to filter it for each parent (can reference ANY ancestor)
/// - How to project it (may include ExprArgs for its own children)
/// - Its own children (recursive nesting)
pub(crate) struct NestedLevel {
    /// Source data (from child's ExecStatement)
    source: VarId,

    /// Which ExprArg in parent's projection this binds to
    arg_index: usize,

    /// How to filter nested records for each parent record
    /// Can reference ANY ancestor in the context stack, not just immediate parent
    qualification: MergeQualification,

    /// Projection for this level (before passing to parent)
    /// Contains ExprArgs for this level's children
    projection: eval::Func,

    /// This level's children (recursive nesting)
    /// Empty for leaf nodes
    nested: Vec<NestedLevel>,
}

pub(crate) enum MergeQualification {
    /// Equality on specific columns (uses hash index)
    ///
    /// root_columns can reference ANY ancestor in the context stack.
    /// Each entry is (levels_up, column_index):
    ///   - levels_up: 0 = immediate parent, 1 = grandparent, 2 = great-grandparent, etc.
    ///   - column_index: which column from that ancestor record
    ///
    /// Example: Tags referencing both Post and User
    ///   root_columns: [(0, 0), (1, 0)]  // Post.id (0 up), User.id (1 up)
    ///   index_id: VarId for Tags data (indexes HashMap will have entry for this VarId)
    ///
    /// During planning: The nested_columns are collected and stored in NestedMerge.indexes
    /// During execution: Use the pre-built index from NestedMerge.indexes[index_id]
    Equality {
        /// Which ancestor levels and columns to extract for the lookup key
        /// Vec<(levels_up, column_index)>
        root_columns: Vec<(usize, usize)>,

        /// Which VarId's index to use (references NestedMerge.indexes)
        index_id: VarId,
    },

    /// General predicate evaluation (uses nested loop)
    /// Args: [ancestor_stack..., nested_record] -> bool
    ///
    /// The ancestor stack contains all ancestors from root to immediate parent:
    /// [root, child, grandchild, ..., immediate_parent, nested_record]
    ///
    /// Execution: For each parent record, evaluate predicate against all
    /// nested records. No indexing.
    Predicate(eval::Func),

    /// Future: More specialized patterns that enable different index strategies
    // SortedRange { ... },  // Could use binary search if pre-sorted
    // Prefix { ... },       // Could use trie/prefix index
}
```

### 2. Planning Phase - Building the Merge DAG

#### 2.0 Query Transformation for Materializations

Before planning merges, the partitioned statements must be **transformed** to batch-load ALL records that will appear in the final graph, not just those matching a specific parent record.

**Original nested statement** (correlated to parent):
```sql
-- In context of a specific user
SELECT * FROM posts WHERE posts.user_id = ?user_id
```

**Transformed materialization query** (loads all relevant posts):
```sql
-- Batch-loads all posts for ANY user that matched the root query
SELECT posts.*
FROM posts
WHERE EXISTS (
  SELECT 1
  FROM VALUES(?)  -- Runtime arg: projected results from parent query (e.g., list of user IDs)
  WHERE posts.user_id = column[0]  -- Correlation: match against the VALUES
  AND [other filter conditions from original query]
)
```

**Key implementation details** (from `partition.rs:525-528`):
1. The parent query results are **projected** to include only the necessary join columns (e.g., just `user.id`)
2. These projected values are passed as a **runtime argument** (`arg(0)`) to the child query
3. The child query selects `FROM VALUES(arg(0))` in the EXISTS subquery
4. The correlation condition compares child columns to `column[0]` (the VALUES)

This is **more efficient** than embedding the full parent query:
- Parent query executes once, results are materialized
- Child query receives a simple list of values (e.g., `[1, 2, 3]`)
- Database can optimize the VALUES lookup

```rust
// From partition.rs:525-528
let sub_select = stmt::Select::new(
    stmt::Values::from(stmt::Expr::arg(0)),  // FROM VALUES(arg(0))
    select.filter.take()
);
select.filter = stmt::Expr::exists(stmt::Query::builder(sub_select).returning(1));
```

**Key insight**: The materialization queries are **batch operations** - they load **exactly** the records that will appear in the final result graph, loading them all at once rather than per-parent. The association of which child records belong to which parent happens later during the NestedMerge execution.

**Three-Phase Execution**:

1. **Materialization Phase**: Execute queries in dependency order
   - Root query: `SELECT * FROM users WHERE users.active = true`
   - Returns: `[User{id:1}, User{id:2}]`

2. **Projection Phase**: Extract join columns for child queries
   - Project: `users.map(|u| u.id)` → `[1, 2]`
   - Child query: `SELECT * FROM posts WHERE EXISTS (SELECT 1 FROM VALUES([1,2]) WHERE posts.user_id = column[0])`
   - Returns: `[Post{id:10, user_id:1}, Post{id:11, user_id:1}, Post{id:12, user_id:2}]`

3. **Merge Phase**: Filter materialized records per parent in-memory
   - For User{id:1}: Filter posts where `post.user_id == 1` → `[Post{10}, Post{11}]`
   - For User{id:2}: Filter posts where `post.user_id == 2` → `[Post{12}]`
   - Efficient: Uses in-memory hash indexes built once

**Key efficiency gains**:
- Materialization queries receive **projected parent results** (just IDs), not full parent query
- Uses `VALUES(?)` with runtime args, not embedded subqueries
- Database sees: `WHERE EXISTS (SELECT 1 FROM VALUES([1,2]) ...)` instead of full parent query
- The correlation condition serves dual purposes:
  - In materialization: Batch-load using `VALUES([1,2])`
  - In merge: Filter per-parent using hash index

#### 2.1 Materialization Graph Construction

In `engine/planner/partition.rs`, after materializations are computed:

```rust
/// Represents the dependency graph of all operations needed to execute a query
///
/// This graph is built WITHOUT assigning VarIds. VarIds are assigned later when
/// converting the graph to the execution plan, allowing for variable reuse based
/// on liveness analysis.
///
/// The graph tracks data flow using abstract "slots" (OutputSlot, InputSlot) that
/// reference specific outputs from specific nodes. When converting to a plan, these
/// slots are mapped to concrete VarIds.
struct MaterializationGraph {
    /// All operations (database queries and post-processing)
    nodes: Vec<MaterializationNode>,

    /// Topologically sorted execution order (inside-out)
    execution_order: Vec<NodeId>,
}

type NodeId = usize;
type OutputSlot = usize;  // Index into a node's outputs array

struct MaterializationNode {
    /// Unique ID for this node
    id: NodeId,

    /// The operation to execute
    operation: Operation,

    /// Nodes that must complete before this one
    /// (because they produce data we need as input)
    dependencies: Vec<NodeId>,
}

enum Operation {
    /// Execute a database query
    ExecStatement {
        /// The database query to execute
        stmt: stmt::Statement,

        /// Input from another node's output
        /// None for root query, Some for child queries
        input: Option<DataRef>,

        /// Output projection expressions
        /// A single query can produce multiple outputs (each gets its own slot)
        /// Example: [full_record_expr, join_columns_expr]
        outputs: Vec<stmt::Expr>,
    },

    /// Nested merge operation - combines parent and child materializations
    /// Handles the ENTIRE nesting hierarchy, not just one level
    NestedMerge {
        /// Root data source
        root: DataRef,

        /// Nested hierarchy - children and their descendants
        nested: Vec<NestedLevel>,

        /// Indexes to build upfront: Map from DataRef to columns to index by
        /// Collected during planning from equality qualifications
        indexes: HashMap<DataRef, Vec<usize>>,

        /// Projection expression (contains ExprArgs for nested collections)
        projection: stmt::Expr,
    },

    /// Projection operation - transforms records
    Project {
        /// Input data source
        input: DataRef,

        /// Projection expression
        projection: stmt::Expr,
    },

    // Future operation types:
    // Union { sources: Vec<DataRef> },
    // PolymorphicMerge { ... },
    // Aggregate { ... },
}

/// Reference to data produced by a node
/// VarId is NOT assigned yet - this is abstract data flow tracking
#[derive(Debug, Clone, Copy)]
struct DataRef {
    /// Which node produces this data
    node: NodeId,

    /// Which output slot from that node (nodes can have multiple outputs)
    slot: OutputSlot,
}

/// A level in the nesting hierarchy (graph representation, before VarId assignment)
struct NestedLevel {
    /// Source data (from child's ExecStatement)
    source: DataRef,

    /// Which ExprArg in parent's projection this binds to
    arg_index: usize,

    /// How to filter nested records for each parent record
    /// Can reference ANY ancestor in the context stack
    qualification: MergeQualification,

    /// Projection for this level (before passing to parent)
    /// Contains ExprArgs for this level's children
    projection: stmt::Expr,

    /// This level's children (recursive nesting)
    nested: Vec<NestedLevel>,
}

pub(crate) enum MergeQualification {
    /// Equality on specific columns
    /// root_columns reference ancestors: Vec<(levels_up, column_index)>
    /// index_ref identifies which index to use from NestedMerge.indexes
    Equality {
        root_columns: Vec<(usize, usize)>,
        index_ref: DataRef,  // References NestedMerge.indexes[index_ref]
    },

    /// General predicate: Args = [ancestor_stack..., nested_record] -> bool
    Predicate(eval::Func),
}

// Example: User query with has_many Todos
//
// Two nodes created for the Users statement (VarIds NOT yet assigned):
//
// Node 0: ExecStatement
//   stmt: SELECT * FROM users WHERE users.active = true
//   input: None  (root query)
//   outputs: [
//     { id: users.id, name: users.name, email: users.email },  // Slot 0 - full records
//     users.id,  // Slot 1 - join columns for child queries
//   ]
//   Purpose: Load user records and extract join columns for child queries
//
// Node 1: NestedMerge
//   root: DataRef { node: 0, slot: 0 }  // References Node 0's first output (full records)
//   nested: [
//     NestedCollection {
//       source: DataRef { node: 3, slot: 0 },  // References todos merge output
//       arg_index: 0,
//       qualification: Equality { ... }
//     }
//   ]
//   projection: { id: users.id, name: users.name, todos: ExprArg(0) }
//   Purpose: Merge todos into users
//
// StatementState for Users:
//   exec_node: 0   (points to the ExecStatement node)
//   output_node: 1 (points to the NestedMerge node that produces final output)
//
// This separation allows:
// - Child queries reference exec_node to get join column DataRef (node: 0, slot: 1)
// - Parent merges reference output_node to get final merged DataRef (node: 1, slot: 0)
//
// VarId assignment happens LATER when converting graph to execution plan:
// - Liveness analysis determines when data is last used
// - VarIds are reused for non-overlapping data
// - DataRef { node: 0, slot: 0 } might become var_0
// - DataRef { node: 0, slot: 1 } might ALSO become var_0 if first output is no longer needed

/// Information about partitioned statements
/// This comes from the partitioning phase, before materialization planning
struct StatementState {
    /// The statement (with nested sub-statements replaced by ExprArg)
    stmt: stmt::Statement,

    /// Arguments to this statement
    args: Vec<Arg>, 

    /// Sub-statements (children) of this statement
    subs: Vec<StmtId>,

    /// Node ID of the ExecStatement that executes this statement's query
    /// Used to find input_var for child queries
    exec_node: Option<NodeId>,

    /// Node ID of the final operation that produces this statement's output
    /// (Usually a NestedMerge, or the ExecStatement itself for leaf statements)
    /// Used as source for parent merges
    output_node: Option<NodeId>,

    /// The projection to apply after merging (contains ExprArgs for children)
    projection: stmt::Expr,
}

enum Arg {
    /// A sub-statement argument (ExprArg that will be filled by child results)
    Sub(StmtId),

    /// A back-reference argument (ExprArg that references parent fields)
    Ref { stmt_id: StmtId, index: usize },
}

impl Planner<'_> {
    pub(crate) fn plan_v2_stmt_query(&mut self, mut stmt: stmt::Statement, dst: plan::VarId) {
        // ... existing code to partition into statements ...
        // During partitioning, sub-statements in returning are replaced with ExprArg

        // PHASE 1: Build materialization graph (NO VarIds assigned yet)
        let mut graph = MaterializationGraph::new();
        self.build_materialization_graph(&mut graph, StmtId(0));

        // PHASE 2: Transform queries to use VALUES(arg(0)) pattern
        self.compute_materializations(&mut graph);

        // PHASE 3: Add final projection node to graph
        let root_output_node = self.stmts[0].output_node.unwrap();
        let final_input = DataRef { node: root_output_node, slot: 0 };

        let final_node_id = graph.nodes.len();
        graph.nodes.push(MaterializationNode {
            id: final_node_id,
            operation: Operation::Project {
                input: final_input,
                projection: stmt::Expr::project_identity(),  // Just pass through
            },
            dependencies: vec![root_output_node],
        });

        // PHASE 4: Compute execution order for all nodes
        graph.compute_execution_order();

        // PHASE 5: Assign VarIds based on liveness analysis
        let var_assignments = self.assign_vars_to_graph(&graph);

        // PHASE 6: Convert graph to execution plan actions
        for node_id in &graph.execution_order {
            let node = &graph.nodes[*node_id];
            match &node.operation {
                Operation::ExecStatement { stmt, input, outputs } => {
                    let mut output_targets = Vec::new();
                    for (slot, output_expr) in outputs.iter().enumerate() {
                        let data_ref = DataRef { node: *node_id, slot };
                        let var = var_assignments[&data_ref];
                        output_targets.push(plan::OutputTarget {
                            var,
                            project: self.build_projection(output_expr, /* types */),
                        });
                    }

                    self.push_action(plan::ExecStatement {
                        input: input.map(|data_ref| plan::Input {
                            var: var_assignments[&data_ref],
                        }),
                        output: Some(plan::Output {
                            ty: /* materialized record type */,
                            targets: output_targets,
                        }),
                        stmt: stmt.clone(),
                        conditional_update_with_no_returning: false,
                    });
                }
                Operation::NestedMerge { root, nested, projection } => {
                    let nested_collections: Vec<_> = nested.iter().map(|nc| {
                        plan::NestedCollection {
                            source: var_assignments[&nc.source],
                            arg_index: nc.arg_index,
                            qualification: nc.qualification.clone(),
                        }
                    }).collect();

                    let output_var = var_assignments[&DataRef { node: *node_id, slot: 0 }];

                    self.push_action(plan::NestedMerge {
                        root: var_assignments[root],
                        nested: nested_collections,
                        output: output_var,
                        projection: self.build_projection(projection, /* types */),
                    });
                }
                Operation::Project { input, projection } => {
                    self.push_action(plan::Project {
                        input: var_assignments[input],
                        output: plan::Output {
                            ty: /* ... */,
                            targets: vec![plan::OutputTarget { var: dst, project: None }],
                        },
                    });
                }
            }
        }
    }

    fn build_materialization_graph(&mut self, graph: &mut MaterializationGraph, stmt_id: StmtId) {
        // Recursively build nodes for all children first (inside-out)
        let child_stmt_ids = self.stmts[stmt_id.0].subs.clone();
        for child_stmt_id in &child_stmt_ids {
            self.build_materialization_graph(graph, *child_stmt_id);
        }

        // Create ExecStatement node for this statement
        let exec_node_id = graph.nodes.len();
        graph.nodes.push(MaterializationNode {
            id: exec_node_id,
            operation: Operation::ExecStatement {
                stmt: self.stmts[stmt_id.0].stmt.clone(),  // Will be transformed later
                input: None,  // Will be set later based on parent
                outputs: vec![],  // Will be populated by compute_materializations()
            },
            dependencies: vec![],  // Will be set later based on parent exec node
        });
        self.stmts[stmt_id.0].exec_node = Some(exec_node_id);

        // Check if this statement has children (nested statements)
        let has_children = self.stmts[stmt_id.0].args.iter().any(|arg| matches!(arg, Arg::Sub(_)));

        if has_children {
            // IMPORTANT: Extract merge qualifications BEFORE transforming the statement
            // At this point, the correlation condition is still in original form (e.g., posts.user_id = users.id)
            // After transformation, it becomes EXISTS (SELECT 1 FROM VALUES(?) WHERE ...) which is harder to parse
            let merge_qualifications = self.extract_merge_qualifications_for_stmt(stmt_id);

            // Create NestedMerge node to combine children with this statement's results
            self.plan_merge_for_stmt(graph, stmt_id, merge_qualifications);
        } else {
            // Leaf statement - just project the exec results, no merging needed
            self.plan_project_for_stmt(graph, stmt_id);
        }
    }

    fn compute_materializations(&mut self, graph: &mut MaterializationGraph) {
        // Transform queries and compute outputs for all ExecStatement nodes
        for stmt_id in 0..self.stmts.len() {
            let stmt_state = &self.stmts[stmt_id];
            let exec_node_id = stmt_state.exec_node.unwrap();
            let node = &mut graph.nodes[exec_node_id];

            let Operation::ExecStatement { stmt, input, outputs } = &mut node.operation else {
                unreachable!()
            };

            // TRANSFORMATION STEP: Rewrite the query to use VALUES(arg(0)) pattern
            if let Some(parent_stmt_id) = self.find_parent_stmt(StmtId(stmt_id)) {
                let parent_exec_node_id = self.stmts[parent_stmt_id.0].exec_node.unwrap();

                // Transform child query to use VALUES(arg(0)) in EXISTS clause
                self.transform_to_values_pattern(stmt, &self.stmts[stmt_id]);

                // Set input to parent's join column output (slot 1)
                *input = Some(DataRef {
                    node: parent_exec_node_id,
                    slot: 1,  // Second output is join columns
                });

                // Add dependency on parent's ExecStatement
                node.dependencies.push(parent_exec_node_id);
            }

            // Build multiple outputs from this materialization
            // Example: One for full records, one for join columns
            *outputs = self.compute_outputs(stmt, &self.stmts[stmt_id]);
        }
    }

    fn transform_to_values_pattern(
        &mut self,
        query: &mut stmt::Statement,
        stmt_state: &StatementState,
    ) {
        // Transform: FROM table WHERE parent_correlation
        // Into: FROM table WHERE EXISTS (SELECT 1 FROM VALUES(arg(0)) WHERE correlation)

        let stmt::Statement::Query(stmt_query) = query else { return };
        let stmt::ExprSet::Select(select) = &mut stmt_query.body else { return };

        for (arg_idx, arg) in stmt_state.args.iter().enumerate() {
            let Arg::Ref { .. } = arg else { continue };

            // Build the VALUES(arg(0)) subquery
            let sub_select = stmt::Select::new(
                stmt::Values::from(stmt::Expr::arg(0)),  // FROM VALUES(arg(0))
                select.filter.take()  // Move filter into EXISTS
            );

            // Replace filter with EXISTS subquery
            select.filter = stmt::Expr::exists(
                stmt::Query::builder(sub_select).returning(1)
            );
        }
    }

    fn extract_merge_qualifications_for_stmt(
        &self,
        stmt_id: StmtId,
    ) -> HashMap<StmtId, MergeQualification> {
        let mut qualifications = HashMap::new();

        // Get all child statements
        let stmt_state = &self.stmts[stmt_id.0];
        let children: Vec<StmtId> = stmt_state.args.iter()
            .filter_map(|arg| match arg {
                Arg::Sub(child_stmt_id) => Some(*child_stmt_id),
                Arg::Ref { .. } => None,
            })
            .collect();

        // Extract qualification for each child from its ORIGINAL WHERE clause
        // (before VALUES(?) transformation)
        for child_stmt_id in children {
            let child_query = &self.stmts[child_stmt_id.0].stmt;

            // Try to extract equality condition on columns from the correlation
            if let Some(equality) = self.try_extract_equality(child_query) {
                // Use equality qualification -> will use hash index
                qualifications.insert(child_stmt_id, MergeQualification::Equality {
                    root_columns: equality.parent_columns,
                    nested_columns: equality.child_columns,
                });
            } else {
                // Fall back to general predicate -> will use nested loop
                // Build an Expr that evaluates the correlation condition
                let predicate = self.build_correlation_predicate(stmt_id, child_stmt_id);
                qualifications.insert(child_stmt_id, MergeQualification::Predicate(predicate));
            }
        }

        qualifications
    }

    fn plan_project_for_stmt(
        &mut self,
        graph: &mut MaterializationGraph,
        stmt_id: StmtId,
    ) {
        let stmt_state = &self.stmts[stmt_id.0];
        let exec_node_id = stmt_state.exec_node.unwrap();

        let project_node_id = graph.nodes.len();
        graph.nodes.push(MaterializationNode {
            id: project_node_id,
            operation: Operation::Project {
                input: DataRef {
                    node: exec_node_id,
                    slot: 0,  // First output is full records
                },
                projection: stmt_state.projection.clone(),
            },
            dependencies: vec![exec_node_id],  // Depends on its own ExecStatement
        });

        // Record this as the output node for this statement
        self.stmts[stmt_id.0].output_node = Some(project_node_id);
    }

    fn plan_merge_for_stmt(
        &mut self,
        graph: &mut MaterializationGraph,
        stmt_id: StmtId,
        merge_qualifications: HashMap<StmtId, MergeQualification>,
    ) {
        let stmt_state = &self.stmts[stmt_id.0];
        let exec_node_id = stmt_state.exec_node.unwrap();

        // Collect indexes needed for equality qualifications
        let mut indexes = HashMap::new();
        self.collect_indexes(stmt_id, &merge_qualifications, &mut indexes);

        // Build the entire nested hierarchy for this statement
        // This recursively builds NestedLevel for all descendants
        let nested_levels = self.build_nested_hierarchy(
            stmt_id,
            &merge_qualifications,
            0,  // Current nesting depth (for calculating levels_up)
        );

        // Collect all dependencies: this statement's exec + all descendant execs
        let mut dependencies = vec![exec_node_id];
        self.collect_all_exec_dependencies(stmt_id, &mut dependencies);

        let merge_node_id = graph.nodes.len();
        graph.nodes.push(MaterializationNode {
            id: merge_node_id,
            operation: Operation::NestedMerge {
                root: DataRef {
                    node: exec_node_id,
                    slot: 0,  // First output is full records
                },
                nested: nested_levels,
                indexes,  // Indexes to build upfront
                projection: stmt_state.projection.clone(),
            },
            dependencies,
        });

        // Record this as the output node for this statement
        self.stmts[stmt_id.0].output_node = Some(merge_node_id);
    }

    fn collect_indexes(
        &self,
        stmt_id: StmtId,
        merge_qualifications: &HashMap<StmtId, MergeQualification>,
        indexes: &mut HashMap<DataRef, Vec<usize>>,
    ) {
        let stmt_state = &self.stmts[stmt_id.0];

        // Get children from StatementState args
        for arg in &stmt_state.args {
            if let Arg::Sub(child_stmt_id) = arg {
                // If this child has an equality qualification, extract the columns to index
                if let Some(qual) = merge_qualifications.get(child_stmt_id) {
                    if let MergeQualification::Equality { nested_columns, .. } = qual {
                        let child_exec_node = self.stmts[child_stmt_id.0].exec_node.unwrap();
                        let data_ref = DataRef {
                            node: child_exec_node,
                            slot: 0,
                        };
                        indexes.insert(data_ref, nested_columns.clone());
                    }
                }

                // Recursively collect indexes for descendants
                self.collect_indexes(*child_stmt_id, merge_qualifications, indexes);
            }
        }
    }

    fn build_nested_hierarchy(
        &mut self,
        stmt_id: StmtId,
        merge_qualifications: &HashMap<StmtId, MergeQualification>,
        current_depth: usize,
    ) -> Vec<NestedLevel> {
        let stmt_state = &self.stmts[stmt_id.0];

        // Get children from StatementState args
        let children: Vec<_> = stmt_state.args.iter()
            .enumerate()
            .filter_map(|(arg_idx, arg)| {
                match arg {
                    Arg::Sub(child_stmt_id) => Some((arg_idx, *child_stmt_id)),
                    Arg::Ref { .. } => None,
                }
            })
            .collect();

        children.into_iter().map(|(arg_idx, child_stmt_id)| {
            let child_exec_node = self.stmts[child_stmt_id.0].exec_node.unwrap();

            // Recursively build this child's hierarchy
            let child_nested = self.build_nested_hierarchy(
                child_stmt_id,
                merge_qualifications,
                current_depth + 1,
            );

            NestedLevel {
                source: DataRef {
                    node: child_exec_node,
                    slot: 0,  // First output is full records
                },
                arg_index: arg_idx,
                qualification: merge_qualifications[&child_stmt_id].clone(),
                projection: self.stmts[child_stmt_id.0].projection.clone(),
                nested: child_nested,
            }
        }).collect()
    }

    fn collect_all_exec_dependencies(&self, stmt_id: StmtId, dependencies: &mut Vec<NodeId>) {
        let stmt_state = &self.stmts[stmt_id.0];

        for arg in &stmt_state.args {
            if let Arg::Sub(child_stmt_id) = arg {
                let child_exec_node = self.stmts[child_stmt_id.0].exec_node.unwrap();
                dependencies.push(child_exec_node);

                // Recursively collect child's dependencies
                self.collect_all_exec_dependencies(*child_stmt_id, dependencies);
            }
        }
    }

    fn try_extract_equality(&self, query: &stmt::Statement) -> Option<EqualityCondition> {
        // Parse the WHERE clause to find equality conditions like:
        // posts.user_id = users.id
        //
        // Returns column indices for both sides of the equality
        // Example: posts.user_id = users.id
        //   parent_columns: [0]  (users.id column index)
        //   nested_columns: [1]  (posts.user_id column index)

        todo!("Parse WHERE clause to extract equality condition")
    }

    fn build_correlation_predicate(
        &self,
        parent_stmt: StmtId,
        child_stmt: StmtId,
    ) -> eval::Func {
        // Build a function that evaluates the correlation condition
        // Args: [parent_record, child_record] -> bool

        todo!("Build predicate function from WHERE clause")
    }

struct EqualityCondition {
    parent_columns: Vec<usize>,
    child_columns: Vec<usize>,
}

    fn build_merge_projection(
        &self,
        returning: &[stmt::Expr],
        nested_arg_index: usize,
        /* types */
    ) -> eval::Func {
        // Build a function that:
        // Args: [root_record, filtered_nested_records]
        // - Binds ExprArg[nested_arg_index] to filtered_nested_records
        // - Evaluates the returning clause projection
        // Returns: projected record

        todo!("Build projection function from returning clause")
    }

    /// Assign VarIds to all DataRefs in the graph based on liveness analysis
    ///
    /// This allows variable reuse: if DataRef A is last used before DataRef B is produced,
    /// they can share the same VarId.
    fn assign_vars_to_graph(&mut self, graph: &MaterializationGraph) -> HashMap<DataRef, VarId> {
        let mut assignments = HashMap::new();
        let mut next_var = 0;

        // Track which variables are currently "live" (still needed)
        // Maps VarId -> set of DataRefs using that VarId
        let mut live_vars: HashMap<VarId, HashSet<DataRef>> = HashMap::new();

        // Compute liveness information: for each DataRef, find its last use
        let mut last_use: HashMap<DataRef, NodeId> = HashMap::new();

        for (node_id, node) in graph.nodes.iter().enumerate() {
            // Mark all inputs as used at this node
            match &node.operation {
                Operation::ExecStatement { input, .. } => {
                    if let Some(data_ref) = input {
                        last_use.insert(*data_ref, node_id);
                    }
                }
                Operation::NestedMerge { root, nested, .. } => {
                    last_use.insert(*root, node_id);
                    for nc in nested {
                        last_use.insert(nc.source, node_id);
                    }
                }
                Operation::Project { input, .. } => {
                    last_use.insert(*input, node_id);
                }
            }
        }

        // Process nodes in execution order, assigning VarIds
        for &node_id in &graph.execution_order {
            let node = &graph.nodes[node_id];

            // Free variables that are no longer needed after this node
            let mut vars_to_free = Vec::new();
            for (var_id, data_refs) in &live_vars {
                for data_ref in data_refs {
                    if last_use.get(data_ref) == Some(&node_id) {
                        vars_to_free.push(*var_id);
                        break;
                    }
                }
            }
            for var_id in vars_to_free {
                live_vars.remove(&var_id);
            }

            // Assign VarIds to this node's outputs
            match &node.operation {
                Operation::ExecStatement { outputs, .. } => {
                    for slot in 0..outputs.len() {
                        let data_ref = DataRef { node: node_id, slot };

                        // Try to reuse a free variable
                        let var = if let Some(free_var) = live_vars.iter()
                            .find(|(_, refs)| refs.is_empty())
                            .map(|(v, _)| *v)
                        {
                            free_var
                        } else {
                            // Allocate new variable
                            let var = VarId(next_var);
                            next_var += 1;
                            var
                        };

                        assignments.insert(data_ref, var);
                        live_vars.entry(var).or_insert_with(HashSet::new).insert(data_ref);
                    }
                }
                Operation::NestedMerge { .. } | Operation::Project { .. } => {
                    // Single output
                    let data_ref = DataRef { node: node_id, slot: 0 };

                    // Try to reuse a free variable
                    let var = if let Some(free_var) = live_vars.iter()
                        .find(|(_, refs)| refs.is_empty())
                        .map(|(v, _)| *v)
                    {
                        free_var
                    } else {
                        // Allocate new variable
                        let var = VarId(next_var);
                        next_var += 1;
                        var
                    };

                    assignments.insert(data_ref, var);
                    live_vars.entry(var).or_insert_with(HashSet::new).insert(data_ref);
                }
            }
        }

        assignments
    }
}

impl MaterializationGraph {
    fn new() -> Self {
        Self {
            nodes: vec![],
            execution_order: vec![],
        }
    }

    /// Compute topological sort of all nodes (inside-out execution)
    fn compute_execution_order(&mut self) {
        // Kahn's algorithm for topological sort
        let mut in_degree: Vec<usize> = self.nodes.iter()
            .map(|node| node.dependencies.len())
            .collect();

        let mut queue: Vec<NodeId> = in_degree.iter()
            .enumerate()
            .filter(|(_, &d)| d == 0)
            .map(|(idx, _)| idx)
            .collect();

        self.execution_order.clear();

        while let Some(node_id) = queue.pop() {
            self.execution_order.push(node_id);

            // Find nodes that depend on this one
            for (idx, node) in self.nodes.iter().enumerate() {
                if node.dependencies.contains(&node_id) {
                    in_degree[idx] -= 1;
                    if in_degree[idx] == 0 {
                        queue.push(idx);
                    }
                }
            }
        }

        assert_eq!(self.execution_order.len(), self.nodes.len(), "Cycle in materialization graph");
    }
}
```

### 2.2 Key Design Decision: Late VarId Assignment

**VarIds are NOT assigned during graph construction.** Instead:

1. **Graph construction** uses abstract `DataRef { node, slot }` to track data flow
2. **Liveness analysis** determines when each DataRef is last used
3. **VarId assignment** happens when converting graph to execution plan, allowing reuse

**Benefits:**
- Variables can be reused when data is no longer needed
- Reduces memory pressure during execution
- Clean separation: graph is pure data flow, VarIds are execution detail
- Example: If `DataRef { node: 0, slot: 0 }` is last used before `DataRef { node: 1, slot: 0 }` is produced, they can share the same VarId

### 2.3 Example: Planning User -> Posts -> Tags

Pseudocode for building the execution plan:

```
Given query: User.include(posts.include(tags))

Original statement structure (before partitioning):
  SELECT * FROM users
  RETURNING {
    id: users.id,
    name: users.name,
    posts: (SELECT * FROM posts WHERE posts.user_id = users.id
            RETURNING {
              id: posts.id,
              title: posts.title,
              tags: (SELECT * FROM tags WHERE tags.post_id = posts.id
                     RETURNING { id: tags.id, name: tags.name })
            })
  }

Step 1: Partition into statements
During partitioning, nested sub-statements are replaced with ExprArg references:

  - Stmt0 (Users): SELECT * FROM users WHERE ...
    RETURNING { id: users.id, name: users.name, posts: ExprArg(0) }
    args: [Sub(Stmt1)]
    subs: [Stmt1]

  - Stmt1 (Posts): SELECT * FROM posts WHERE posts.user_id = users.id
    RETURNING { id: posts.id, title: posts.title, tags: ExprArg(0) }
    args: [Ref(Stmt0, user_id), Sub(Stmt2)]
    subs: [Stmt2]

  - Stmt2 (Tags): SELECT * FROM tags WHERE tags.post_id = posts.id
    RETURNING { id: tags.id, name: tags.name }
    args: [Ref(Stmt1, post_id)]
    subs: []

Step 2: Extract merge qualifications (BEFORE transformation)
At this point, correlation conditions are in original form, easy to parse:

  Stmt1 (Posts): WHERE posts.user_id = users.id
    → Extract: Equality { root_columns: [(0, 0)], nested_columns: [1] }
    → Meaning: Post.user_id (col 1) matches User.id (0 levels up, col 0)

  Stmt2 (Tags): WHERE tags.post_id = posts.id
    → Extract: Equality { root_columns: [(0, 0)], nested_columns: [1] }
    → Meaning: Tag.post_id (col 1) matches Post.id (0 levels up, col 0)

Step 3: Build graph nodes (inside-out for ExecStatements)

  Node 0: ExecStatement (Tags)
    stmt: SELECT * FROM tags WHERE ...  // Original, not yet transformed
    input: None  // Will be set to DataRef{node:2, slot:1} after transformation
    outputs: []  // Will be populated during transformation
    dependencies: []  // Will be set to [2] after transformation

  Node 1: Project (Tags - leaf, no children)
    input: DataRef { node: 0, slot: 0 }  // Tags exec, first output
    projection: { id: tags.id, name: tags.name }
    dependencies: [0]

  Node 2: ExecStatement (Posts)
    stmt: SELECT * FROM posts WHERE ...  // Original, not yet transformed
    input: None  // Will be set to DataRef{node:4, slot:1} after transformation
    outputs: []  // Will be populated during transformation
    dependencies: []  // Will be set to [4] after transformation

  Node 3: NestedMerge (Posts - has Tags as child)
    root: DataRef { node: 2, slot: 0 }  // Posts exec, first output
    nested: [
      NestedLevel {
        source: DataRef { node: 0, slot: 0 },  // Tags exec output
        arg_index: 0,
        qualification: Equality { root_columns: [(0, 0)], nested_columns: [1] },
        projection: { id: tags.id, name: tags.name },
        nested: [],  // Tags has no children
      }
    ]
    projection: { id: posts.id, title: posts.title, tags: ExprArg(0) }
    dependencies: [0, 2]  // Needs both Tags exec and Posts exec

  Node 4: ExecStatement (Users)
    stmt: SELECT * FROM users WHERE ...  // Original
    input: None  // Root query
    outputs: []  // Will be populated during transformation
    dependencies: []

  Node 5: NestedMerge (Users - has Posts as child, which has Tags as grandchild)
    root: DataRef { node: 4, slot: 0 }  // Users exec, first output
    nested: [
      NestedLevel {
        source: DataRef { node: 2, slot: 0 },  // Posts exec output
        arg_index: 0,
        qualification: Equality { root_columns: [(0, 0)], nested_columns: [1] },
        projection: { id: posts.id, title: posts.title, tags: ExprArg(0) },
        nested: [
          NestedLevel {
            source: DataRef { node: 0, slot: 0 },  // Tags exec output
            arg_index: 0,
            qualification: Equality { root_columns: [(0, 0)], nested_columns: [1] },
            projection: { id: tags.id, name: tags.name },
            nested: [],
          }
        ],
      }
    ]
    projection: { id: users.id, name: users.name, posts: ExprArg(0) }
    dependencies: [0, 2, 4]  // Needs Tags, Posts, and Users execs

  Node 6: Project (Final output)
    input: DataRef { node: 5, slot: 0 }
    projection: identity
    dependencies: [5]

Step 4: Transform queries to VALUES(arg(0)) pattern
Now we transform the queries and populate outputs:

  Node 0 (Tags):
    stmt: SELECT * FROM tags WHERE EXISTS (
            SELECT 1 FROM VALUES(?) WHERE tags.post_id = column[0]
          )
    input: DataRef { node: 2, slot: 1 }  // Posts join columns
    outputs: [
      { id: tags.id, name: tags.name }  // Slot 0
    ]
    dependencies: [2]

  Node 2 (Posts):
    stmt: SELECT * FROM posts WHERE EXISTS (
            SELECT 1 FROM VALUES(?) WHERE posts.user_id = column[0]
          )
    input: DataRef { node: 4, slot: 1 }  // Users join columns
    outputs: [
      { id: posts.id, user_id: posts.user_id, title: posts.title },  // Slot 0 - full records
      posts.id,  // Slot 1 - join columns for Tags
    ]
    dependencies: [4]

  Node 4 (Users):
    stmt: SELECT * FROM users WHERE users.active = true  // No transformation for root
    input: None
    outputs: [
      { id: users.id, name: users.name },  // Slot 0 - full records
      users.id,  // Slot 1 - join columns for Posts
    ]
    dependencies: []

Step 5: Compute execution order
  Topological sort: [4, 2, 0, 1, 3, 5, 6]

  Execution sequence:
    4: ExecStatement(Users) → outputs to DataRef{4,0} and DataRef{4,1}
    2: ExecStatement(Posts, input=DataRef{4,1}) → outputs to DataRef{2,0} and DataRef{2,1}
    0: ExecStatement(Tags, input=DataRef{2,1}) → outputs to DataRef{0,0}
    1: Project(DataRef{0,0}) → leaf projection (no merging)
    3: NestedMerge(Posts) → hierarchical merge (but Posts has no merge, just Tags does)
    5: NestedMerge(Users) → hierarchical merge of entire tree
    6: Project(final) → output to dst

Step 6: Assign VarIds based on liveness
  After topological sort, liveness analysis determines:
    DataRef{4,0} → var_0  (users full records)
    DataRef{4,1} → var_1  (users join columns, freed after Posts exec)
    DataRef{2,0} → var_1  (REUSED! posts full records)
    DataRef{2,1} → var_2  (posts join columns, freed after Tags exec)
    DataRef{0,0} → var_2  (REUSED! tags full records)
    ... etc

StatementState tracking:
  Stmt0 (Users): exec_node=4, output_node=5
  Stmt1 (Posts): exec_node=2, output_node=3
  Stmt2 (Tags): exec_node=0, output_node=1

Key design points:
- **ExecStatement nodes built inside-out** (Tags → Posts → Users)
- **Output nodes reference entire hierarchy** (Users' NestedMerge contains the full Posts→Tags tree)
- **Qualifications extracted before transformation** (easier to parse)
- **Graph uses DataRef** (no VarIds yet)
- **VarIds assigned after liveness analysis** (allows reuse)
```

### 3. Execution Phase

**Key Design: Hierarchical NestedMerge with Ancestor Context and Pre-Planned Indexes**

A single `NestedMerge` operation handles the ENTIRE nesting hierarchy for a root statement, not just one level. This is necessary because:

1. **Deep qualifications**: Tags can reference both Posts AND Users (e.g., `tags.post_id = posts.id AND tags.user_id = users.id`)
2. **Ancestor context required**: When filtering Tags, we need access to both the Post record AND the User record
3. **Outside-in execution**: Process User, then for each User process Posts, then for each Post process Tags
4. **Pre-planned indexing**: During planning, we determine which indexes are needed and store this in `NestedMerge.indexes`. During execution, we build all indexes ONCE upfront before iteration, then reference them via `qualification.index_id`. This avoids rebuilding indexes for each parent record.

**Algorithm:**
```
For each User:
  context_stack = [User]
  For each Post WHERE post.user_id = User.id:
    context_stack = [User, Post]
    For each Tag WHERE tag.post_id = Post.id AND tag.user_id = User.id:
      // Can access both Post and User from context_stack
      Add Tag to Post
    Add Post to User
```

In `engine/exec/nested_merge.rs` (new file):

```rust
impl Exec<'_> {
    pub(super) async fn action_nested_merge(
        &mut self,
        action: &plan::NestedMerge
    ) -> Result<()> {
        // Load root materialization
        let root_records = self.vars.load(action.root).collect().await?;

        // Load all data needed for nested levels
        let mut all_data = HashMap::new();
        self.load_nested_data(&action.nested, &mut all_data).await?;

        // Build all indexes upfront using the pre-planned index specifications
        let mut all_indexes = HashMap::new();
        for (var_id, index_columns) in &action.indexes {
            let data = all_data.get(var_id)
                .expect("Data should be loaded for all indexed VarIds");
            let index = self.build_hash_index(data, index_columns)?;
            all_indexes.insert(*var_id, index);
        }

        // Execute the hierarchical nested merge with pre-built indexes
        let results = self.execute_nested_levels(
            root_records,
            &action.nested,
            &action.projection,
            &[],  // Empty ancestor stack for root level
            &all_data,
            &all_indexes,
        ).await?;

        // Store output
        self.vars.store(action.output, ValueStream::from_vec(results));
        Ok(())
    }

    /// Load all data from nested tree
    async fn load_nested_data(
        &self,
        nested_levels: &[plan::NestedLevel],
        all_data: &mut HashMap<VarId, Vec<stmt::Value>>,
    ) -> Result<()> {
        for level in nested_levels {
            if !all_data.contains_key(&level.source) {
                let data = self.vars.load(level.source).collect().await?;
                all_data.insert(level.source, data);
            }

            if !level.nested.is_empty() {
                self.load_nested_data(&level.nested, all_data).await?;
            }
        }
        Ok(())
    }

    /// Recursively execute nested merges at all levels
    ///
    /// This processes one level of nesting, then recursively processes children.
    /// Execution is outside-in to provide ancestor context.
    /// All data and indexes are pre-loaded to avoid rebuilding during iteration.
    async fn execute_nested_levels(
        &self,
        parent_records: Vec<stmt::Value>,
        nested_levels: &[plan::NestedLevel],
        projection: &eval::Func,
        ancestor_stack: &[stmt::Value],
        all_data: &HashMap<VarId, Vec<stmt::Value>>,
        all_indexes: &HashMap<VarId, HashMap<CompositeKey, Vec<stmt::Value>>>,
    ) -> Result<Vec<stmt::Value>> {
        // Prepare loaded data for this level
        let mut loaded_levels = Vec::with_capacity(nested_levels.len());

        for level in nested_levels {
            let nested_data = all_data.get(&level.source)
                .expect("Data should be loaded for all VarIds");

            loaded_levels.push(LoadedNestedLevel {
                data: nested_data,
                level_info: level,
            });
        }

        // Process each parent record
        let mut results = Vec::with_capacity(parent_records.len());

        for parent_record in parent_records {
            // Build ancestor context stack: [grandparents..., parent]
            let mut context_stack = ancestor_stack.to_vec();
            context_stack.push(parent_record.clone());

            // Filter and process all nested collections for this parent
            let mut filtered_collections = Vec::new();

            for loaded_level in &loaded_levels {
                // Filter using ancestor context
                let filtered = self.filter_hierarchical(
                    &loaded_level.data,
                    &context_stack,
                    &loaded_level.level_info.qualification,
                    all_indexes,
                )?;

                // If this level has children, recursively merge them
                let processed = if !loaded_level.level_info.nested.is_empty() {
                    self.execute_nested_levels(
                        filtered,
                        &loaded_level.level_info.nested,
                        &loaded_level.level_info.projection,
                        &context_stack,  // Pass down ancestor context
                        all_data,        // Pass through pre-loaded data
                        all_indexes,     // Pass through pre-built indexes
                    ).await?
                } else {
                    // Leaf level - just apply projection to each record
                    filtered.iter()
                        .map(|rec| loaded_level.level_info.projection.eval(&[rec.clone()]))
                        .collect::<Result<Vec<_>>>()?
                };

                filtered_collections.push(stmt::Value::List(processed));
            }

            // Apply projection at this level: [parent_record, filtered_0, filtered_1, ...]
            // Collections may not be in arg_index order, so build a sparse array
            let max_arg = loaded_levels.iter()
                .map(|l| l.level_info.arg_index)
                .max()
                .unwrap_or(0);
            let mut projection_args = vec![stmt::Value::Null; max_arg + 2];  // +1 for parent, +1 for 0-indexing
            projection_args[0] = parent_record;

            for (loaded_level, filtered) in loaded_levels.iter().zip(filtered_collections) {
                projection_args[loaded_level.level_info.arg_index + 1] = filtered;
            }

            let projected = projection.eval(&projection_args)?;
            results.push(projected);
        }

        Ok(results)
    }

    /// Filter nested records using ancestor context
    ///
    /// The qualification can reference ANY ancestor in the context stack,
    /// not just the immediate parent.
    fn filter_hierarchical(
        &self,
        nested_records: &[stmt::Value],
        ancestor_stack: &[stmt::Value],  // [root, child, grandchild, ..., parent]
        qualification: &MergeQualification,
        all_indexes: &HashMap<VarId, HashMap<CompositeKey, Vec<stmt::Value>>>,
    ) -> Result<Vec<stmt::Value>> {
        match qualification {
            MergeQualification::Equality { root_columns, index_id } => {
                // Build composite key from ancestor stack
                // root_columns = [(levels_up, col_idx), ...]
                let mut key_values = Vec::new();

                for (levels_up, col_idx) in root_columns {
                    // levels_up: 0 = immediate parent, 1 = grandparent, etc.
                    let ancestor_idx = ancestor_stack.len() - 1 - levels_up;
                    let ancestor_record = ancestor_stack[ancestor_idx].expect_record();
                    key_values.push(ancestor_record[*col_idx].clone());
                }

                let key = CompositeKey(key_values);

                // Lookup in pre-built index using index_id
                let index = all_indexes.get(index_id)
                    .expect("Hash index should exist for Equality qualification");

                Ok(index
                    .get(&key)
                    .cloned()
                    .unwrap_or_default())
            }
            MergeQualification::Predicate(predicate) => {
                // Evaluate predicate for each nested record
                // Args: [ancestor_stack..., nested_record] -> bool
                let mut matches = Vec::new();

                for nested_record in nested_records {
                    let mut args = ancestor_stack.to_vec();
                    args.push(nested_record.clone());

                    if predicate.eval_bool(&args)? {
                        matches.push(nested_record.clone());
                    }
                }

                Ok(matches)
            }
        }
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
            index
                .entry(key)
                .or_insert_with(Vec::new)
                .push(record.clone());
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
}

// Helper struct for loaded nested levels
struct LoadedNestedLevel<'a> {
    data: &'a Vec<stmt::Value>,
    level_info: &'a plan::NestedLevel,
}

// Composite key type for multi-column equality
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CompositeKey(Vec<stmt::Value>);
```

### 3.1 Example: Executing User -> Posts -> Tags (Hierarchical)

This example shows the **hierarchical execution** where a single NestedMerge operation handles the entire nesting tree.

```
Given plan from section 2.3:
  1. ExecStatement(Users) -> [var_0_full, var_0_ids]
  2. ExecStatement(Posts, input=var_0_ids) -> [var_1_full, var_1_ids]
  3. ExecStatement(Tags, input=var_1_ids) -> [var_2_full]
  4. Project(Tags) -> var_3  // Leaf projection
  5. NestedMerge(Users - entire hierarchy) -> var_4
  6. Project(final) -> dst

Execution trace:

Step 1-3: Execute materialization queries (batch-load all data)
  var_0_full = [User{id:1, name:"Alice"}, User{id:2, name:"Bob"}]
  var_1_full = [Post{id:10, user_id:1}, Post{id:11, user_id:1}, Post{id:12, user_id:2}]
  var_2_full = [Tag{id:100, post_id:10}, Tag{id:101, post_id:10}, Tag{id:102, post_id:12}]

Step 4: Project(Tags) - Simple leaf projection
  var_3 = var_2_full.map(|tag| { id: tag.id, name: tag.name })
  var_3 = [Tag{id:100, name:"rust"}, Tag{id:101, name:"async"}, Tag{id:102, name:"perf"}]

Step 5: NestedMerge(Users - ENTIRE HIERARCHY)
  This is a SINGLE operation that handles the full nesting tree!

  action = NestedMerge {
    root: var_0_full,
    nested: [
      NestedLevel {  // Posts level
        source: var_1_full,
        arg_index: 0,
        qualification: Equality {
          root_columns: [(0, 0)],  // User.id (0 levels up)
          index_id: var_1_full,    // Use index for var_1_full
        },
        projection: { id: posts.id, title: posts.title, tags: ExprArg(0) },
        nested: [
          NestedLevel {  // Tags level (nested under Posts)
            source: var_2_full,
            arg_index: 0,
            qualification: Equality {
              root_columns: [(0, 0)],  // Post.id (0 levels up = parent)
              index_id: var_2_full,    // Use index for var_2_full
            },
            projection: { id: tags.id, name: tags.name },
            nested: [],
          }
        ],
      }
    ],
    indexes: {
      var_1_full: [1],  // Index Posts by column 1 (user_id)
      var_2_full: [1],  // Index Tags by column 1 (post_id)
    },
    projection: { id: users.id, name: users.name, posts: ExprArg(0) },
  }

  Execution (hierarchical, outside-in with upfront indexing):

  1. Load all data from nested tree:
     all_data = {
       var_1_full: [Post{id:10, user_id:1}, Post{id:11, user_id:1}, Post{id:12, user_id:2}],
       var_2_full: [Tag{id:100, post_id:10}, Tag{id:101, post_id:10}, Tag{id:102, post_id:12}],
     }

  2. Build ALL indexes upfront using action.indexes specification:
     all_indexes = {
       var_1_full: {  // Posts indexed by column 1 (user_id)
         1 -> [Post{10}, Post{11}],
         2 -> [Post{12}],
       },
       var_2_full: {  // Tags indexed by column 1 (post_id)
         10 -> [Tag{100}, Tag{101}],
         12 -> [Tag{102}],
       },
     }

     Note: These indexes are built ONCE using the pre-planned specifications.
     The qualification.index_id tells us which index to use during filtering.

  4. Process each User (OUTSIDE-IN with context, reusing indexes):

     User{id:1, name:"Alice"}:
       context_stack = [User{1}]

       // Filter Posts for this User using PRE-BUILT index
       filtered_posts = all_indexes[var_1_full][1] = [Post{10}, Post{11}]

       // Process each Post with its Tags
       processed_posts = []
       For each Post{10, user_id:1} in filtered_posts:
         context_stack = [User{1}, Post{10}]

         // Filter Tags using PRE-BUILT index
         // (In this example, Tags only reference Post, but they COULD reference User too!)
         filtered_tags = all_indexes[var_2_full][10] = [Tag{100}, Tag{101}]

         // No children under Tags, so just apply projection
         projected_tags = [
           { id: 100, name: "rust" },
           { id: 101, name: "async" },
         ]

         // Apply Posts projection with filtered tags
         processed_post = {
           id: 10,
           title: "Post1",
           tags: projected_tags,
         }
         processed_posts.push(processed_post)

       For each Post{11, user_id:1} in filtered_posts:
         context_stack = [User{1}, Post{11}]
         filtered_tags = all_indexes[var_2_full][11] = []  // Reusing index
         processed_post = { id: 11, title: "Post2", tags: [] }
         processed_posts.push(processed_post)

       // Apply User projection with processed posts
       user_result = {
         id: 1,
         name: "Alice",
         posts: processed_posts,
       }

     User{id:2, name:"Bob"}:
       context_stack = [User{2}]
       filtered_posts = all_indexes[var_1_full][2] = [Post{12}]  // Reusing index

       For each Post{12, user_id:2}:
         context_stack = [User{2}, Post{12}]
         filtered_tags = all_indexes[var_2_full][12] = [Tag{102}]  // Reusing index
         projected_tags = [{ id: 102, name: "perf" }]
         processed_post = { id: 12, title: "Post3", tags: projected_tags }

       user_result = {
         id: 2,
         name: "Bob",
         posts: [{ id: 12, title: "Post3", tags: [Tag{102}] }],
       }

  6. Final result:
     var_4 = [
       {
         id: 1,
         name: "Alice",
         posts: [
           { id: 10, title: "Post1", tags: [Tag{100}, Tag{101}] },
           { id: 11, title: "Post2", tags: [] },
         ]
       },
       {
         id: 2,
         name: "Bob",
         posts: [
           { id: 12, title: "Post3", tags: [Tag{102}] }
         ]
       },
     ]

Step 6: Project(final) -> dst
```

**Key Points:**
- **Single NestedMerge handles entire tree**: Not separate merges for Tags→Posts and Posts→Users
- **Outside-in execution with context**: Process User, then for each User process Posts, then for each Post process Tags
- **Context stack grows**: [] → [User] → [User, Post]
- **CRITICAL OPTIMIZATION - Indexes planned at planning time**: During planning, `collect_indexes()` traverses the nested tree and builds the `indexes: HashMap<VarId, Vec<usize>>` containing which columns to index for each VarId. During execution, these indexes are built ONCE upfront using the pre-planned specifications, then reused throughout iteration. The tags_index is built ONE time and then reused for ALL Posts across ALL Users. Without this optimization, the tags_index would be rebuilt for every User (2 times in this example), which would be wasteful.
- **Index references in qualifications**: Each `Equality` qualification has an `index_id: VarId` that references the pre-built index to use
- **Hierarchical structure in the operation**: Posts.nested contains Tags level
- **Deep qualifications possible**: Tags could reference User via `root_columns: [(1, 0)]` (1 level up = grandparent)

### 3.2 Example: User with Multiple Sibling Collections (Posts AND Addresses)

This example shows multiple collections at the **same level** - User has both Posts and Addresses as direct children.

**Query:** `User.include(posts, addresses)`

**Structure:**
```
User
├── Posts (sibling 1)
└── Addresses (sibling 2)
```

**Planning:**
```rust
NestedMerge {
    root: users_data,
    nested: [
        // Sibling 1: Posts
        NestedLevel {
            source: posts_data,
            arg_index: 0,  // ExprArg(0) in User projection
            qualification: Equality {
                root_columns: [(0, 0)],  // User.id
                nested_columns: [1],      // Post.user_id
            },
            projection: { id: posts.id, title: posts.title },
            nested: [],  // No children
        },
        // Sibling 2: Addresses
        NestedLevel {
            source: addresses_data,
            arg_index: 1,  // ExprArg(1) in User projection
            qualification: Equality {
                root_columns: [(0, 0)],  // User.id
                nested_columns: [1],      // Address.user_id
            },
            projection: { id: addresses.id, street: addresses.street },
            nested: [],  // No children
        },
    ],
    projection: {
        id: users.id,
        name: users.name,
        posts: ExprArg(0),     // Bound to filtered posts
        addresses: ExprArg(1),  // Bound to filtered addresses
    },
}
```

**Execution:**
```rust
// Materialization phase (batch-load all data)
users = [User{id:1, name:"Alice"}, User{id:2, name:"Bob"}]
posts = [Post{id:10, user_id:1}, Post{id:11, user_id:1}, Post{id:12, user_id:2}]
addresses = [Address{id:20, user_id:1}, Address{id:21, user_id:2}]

// Build indexes for both sibling collections
posts_index = {
    1 -> [Post{id:10}, Post{id:11}],
    2 -> [Post{id:12}],
}

addresses_index = {
    1 -> [Address{id:20}],
    2 -> [Address{id:21}],
}

// Execute nested merge
For each User:
    context_stack = [User]

    // Filter BOTH sibling collections using the same context
    filtered_posts = posts_index[User.id]
    filtered_addresses = addresses_index[User.id]

    // Apply projection with BOTH collections
    projection_args = [User, filtered_posts, filtered_addresses]
    result = projection.eval(projection_args)

Results:
[
    {
        id: 1,
        name: "Alice",
        posts: [Post{id:10}, Post{id:11}],
        addresses: [Address{id:20}],
    },
    {
        id: 2,
        name: "Bob",
        posts: [Post{id:12}],
        addresses: [Address{id:21}],
    },
]
```

**Key Points:**
- **Multiple siblings processed together**: Both Posts and Addresses are loaded, indexed, and filtered for each User
- **Same context for all siblings**: All siblings at the same level use the same ancestor context
- **Different arg_index**: Each sibling binds to a different ExprArg in the parent's projection
- **Independent qualifications**: Each sibling can have different qualification logic

### 3.3 Example: Deep Nesting with Multiple Siblings

Now consider: `User.include(posts.include(tags, comments), addresses)`

**Structure:**
```
User
├── Posts
│   ├── Tags
│   └── Comments
└── Addresses
```

**Planning:**
```rust
NestedMerge {
    root: users_data,
    nested: [
        // Sibling 1: Posts (has children)
        NestedLevel {
            source: posts_data,
            arg_index: 0,
            qualification: Equality { root_columns: [(0, 0)], nested_columns: [1] },
            projection: {
                id: posts.id,
                title: posts.title,
                tags: ExprArg(0),      // Post's child 1
                comments: ExprArg(1),   // Post's child 2
            },
            nested: [
                // Posts' child 1: Tags
                NestedLevel {
                    source: tags_data,
                    arg_index: 0,
                    qualification: Equality {
                        root_columns: [(0, 0)],  // Post.id (0 levels up)
                        nested_columns: [1],      // Tag.post_id
                    },
                    projection: { id: tags.id, name: tags.name },
                    nested: [],
                },
                // Posts' child 2: Comments
                NestedLevel {
                    source: comments_data,
                    arg_index: 1,
                    qualification: Equality {
                        root_columns: [(0, 0)],  // Post.id (0 levels up)
                        nested_columns: [1],      // Comment.post_id
                    },
                    projection: { id: comments.id, text: comments.text },
                    nested: [],
                },
            ],
        },
        // Sibling 2: Addresses (no children)
        NestedLevel {
            source: addresses_data,
            arg_index: 1,
            qualification: Equality { root_columns: [(0, 0)], nested_columns: [1] },
            projection: { id: addresses.id, street: addresses.street },
            nested: [],
        },
    ],
    projection: {
        id: users.id,
        name: users.name,
        posts: ExprArg(0),
        addresses: ExprArg(1),
    },
}
```

**Execution:**
```
For each User:
    context_stack = [User]

    // Process sibling 1: Posts (has children)
    filtered_posts = posts_index[User.id]

    For each Post in filtered_posts:
        context_stack = [User, Post]

        // Process Post's children (Tags and Comments)
        filtered_tags = tags_index[Post.id]
        filtered_comments = comments_index[Post.id]

        // Project Post with its children
        post_result = post_projection.eval([Post, filtered_tags, filtered_comments])

    processed_posts = [post_result for each Post]

    // Process sibling 2: Addresses (no children)
    filtered_addresses = addresses_index[User.id]
    processed_addresses = [address_projection.eval([addr]) for addr in filtered_addresses]

    // Project User with both sibling collections
    user_result = user_projection.eval([User, processed_posts, processed_addresses])
```

**Key Points:**
- **Nested siblings**: Posts has its own sibling children (Tags and Comments)
- **Different depths**: Tags/Comments are at depth 2, Addresses is at depth 1
- **Context grows recursively**: User → [User, Post] for Tags/Comments
- **Each level processes all its siblings**: Posts processes both Tags and Comments together

### 3.4 Example: User with Multiple Collections (Posts AND Comments)

**DEPRECATED - This example was replaced by 3.2 and 3.3 above**

```
Given query: User.include(posts, comments)

Materializations:
  var_0 = [User{id:1}, User{id:2}]
  var_1 = [Post{id:10, user_id:1}, Post{id:11, user_id:1}]
  var_2 = [Comment{id:20, user_id:1}, Comment{id:21, user_id:2}]

After leaf projections:
  var_3 = [Post{id:10, title:"..."}, Post{id:11, title:"..."}]  // Projected posts
  var_4 = [Comment{id:20, text:"..."}, Comment{id:21, text:"..."}]  // Projected comments

NestedMerge for Users (merges BOTH posts and comments):
  action = NestedMerge {
    root: var_0,
    nested: [
      NestedCollection {
        source: var_3,  // Projected posts
        arg_index: 0,   // ExprArg(0) in returning clause
        qualification: Equality { root_columns: [0], nested_columns: [1] }  // user.id == post.user_id
      },
      NestedCollection {
        source: var_4,  // Projected comments
        arg_index: 1,   // ExprArg(1) in returning clause
        qualification: Equality { root_columns: [0], nested_columns: [1] }  // user.id == comment.user_id
      },
    ],
    projection: Func([user_record, filtered_posts, filtered_comments] -> {
      id: user.id,
      name: user.name,
      posts: filtered_posts,      // Binds ExprArg(0)
      comments: filtered_comments  // Binds ExprArg(1)
    }),
  }

Execution:
  1. Build hash index on posts keyed by user_id:
     posts_index = { 1 -> [Post{10}, Post{11}] }

  2. Build hash index on comments keyed by user_id:
     comments_index = { 1 -> [Comment{20}], 2 -> [Comment{21}] }

  3. For User{id:1}:
     - Lookup posts: posts_index[1] = [Post{10}, Post{11}]
     - Lookup comments: comments_index[1] = [Comment{20}]
     - Apply projection([User{1}, [Post{10}, Post{11}], [Comment{20}]]) ->
       { id: 1, name: "Alice", posts: [...], comments: [...] }

  4. For User{id:2}:
     - Lookup posts: posts_index[2] = []
     - Lookup comments: comments_index[2] = [Comment{21}]
     - Apply projection([User{2}, [], [Comment{21}]]) ->
       { id: 2, name: "Bob", posts: [], comments: [...] }

Result:
  var_5 = [
    { id: 1, name: "Alice", posts: [Post{10}, Post{11}], comments: [Comment{20}] },
    { id: 2, name: "Bob", posts: [], comments: [Comment{21}] },
  ]
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