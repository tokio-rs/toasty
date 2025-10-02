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
/// A single NestedMerge can handle multiple nested collections at the same level.
/// For example, if User has both `posts` and `comments`, and both are eagerly loaded,
/// one NestedMerge will filter both collections and bind them to their respective
/// ExprArgs before applying the projection.
pub(crate) struct NestedMerge {
    /// Root materialization variable (parent records)
    root: VarId,

    /// Nested collections to merge into the root
    /// Each entry corresponds to a child statement that needs to be merged
    nested: Vec<NestedCollection>,

    /// Output variable (projected result with nested structure)
    output: VarId,

    /// Projection to apply after filtering all nested collections
    /// Args: [root_record, filtered_collection_0, filtered_collection_1, ...]
    /// The filtered collections are bound to ExprArgs in the returning clause
    /// This comes from the returning clause
    projection: eval::Func,
}

pub(crate) struct NestedCollection {
    /// Variable containing the nested records to filter
    source: VarId,

    /// Which ExprArg in the projection corresponds to this collection
    /// After filtering, the results will be passed as this argument to the projection
    arg_index: usize,

    /// How to filter nested records for each root record
    /// The qualification determines the indexing strategy:
    /// - Equality: Build hash index on nested_columns
    /// - Predicate: No index, evaluate predicate for each pair
    qualification: MergeQualification,
}

pub(crate) enum MergeQualification {
    /// Equality on specific columns (uses hash index)
    /// root_record[root_columns] == nested_record[nested_columns]
    ///
    /// Execution: Build hash index on nested records keyed by nested_columns,
    /// then lookup using root_columns for each root record.
    Equality {
        root_columns: Vec<usize>,
        nested_columns: Vec<usize>,
    },

    /// General predicate evaluation (uses nested loop)
    /// Args: [root_record, nested_record] -> bool
    ///
    /// Execution: For each root record, evaluate predicate against all
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
/// This graph structure is general enough to support future operation types beyond
/// NestedMerge (e.g., Union, PolymorphicMerge, Aggregate), though currently only
/// ExecStatement, NestedMerge, and Project are implemented.
struct MaterializationGraph {
    /// All operations (database queries and post-processing) in execution order
    nodes: Vec<MaterializationNode>,

    /// Topologically sorted execution order (inside-out)
    execution_order: Vec<NodeId>,
}

type NodeId = usize;

struct MaterializationNode {
    /// Unique ID for this node
    id: NodeId,

    /// The operation to execute
    operation: Operation,

    /// Nodes that must complete before this one
    /// (because they produce variables we need as input)
    dependencies: Vec<NodeId>,
}

enum Operation {
    /// Execute a database query
    ExecStatement {
        /// The database query to execute
        stmt: stmt::Statement,

        /// Input variable containing parent's projected join columns
        /// None for root query, Some(var_id) for child queries
        input_var: Option<VarId>,

        /// Output projections from the query
        /// A single query can produce multiple outputs, each with its own variable and projection
        /// Example: One output for the full materialized records, another for just join columns
        outputs: Vec<Output>,
    },

    /// Nested merge operation - combines parent and child materializations
    NestedMerge(plan::NestedMerge),

    /// Projection operation - transforms records (including final output projection)
    Project(plan::Project),

    // Future operation types:
    // Union(plan::Union),
    // PolymorphicMerge(plan::PolymorphicMerge),
    // Aggregate(plan::Aggregate),
}

struct Output {
    /// Variable where this projected output is stored
    var: VarId,

    /// Expression to project from the raw query results
    expr: stmt::Expr,
}

// Example: User query with has_many Todos
//
// Two nodes created for the Users statement:
//
// Node 0: ExecStatement
//   stmt: SELECT * FROM users WHERE users.active = true
//   input_var: None  (root query)
//   outputs: [
//     Output { var: var_0_full, expr: { id: users.id, name: users.name, email: users.email } },
//     Output { var: var_0_ids, expr: users.id },
//   ]
//   Purpose: Load user records and extract join columns for child queries
//
// Node 1: NestedMerge
//   root: var_0_full
//   nested: [NestedCollection { source: var_todos, ... }]
//   output: var_1
//   Purpose: Merge todos into users
//
// StatementState for Users:
//   exec_node: 0   (points to the ExecStatement node)
//   output_node: 1 (points to the NestedMerge node that produces final output)
//
// This separation allows:
// - Child queries to reference exec_node to get join column vars (var_0_ids)
// - Parent merges to reference output_node to get final merged results (var_1)

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

        // Build materialization graph structure (queries not yet transformed)
        let mut graph = MaterializationGraph::new();
        self.build_materialization_graph(&mut graph, StmtId(0));

        // TRANSFORMATION STEP: Transform queries to use VALUES(arg(0)) pattern
        // and compute multiple outputs for each materialization
        self.compute_materializations(&mut graph);

        // Add final projection node to graph (output to destination variable)
        let root_output_node = self.stmts[0].output_node.unwrap();
        let final_var = match &graph.nodes[root_output_node].operation {
            Operation::NestedMerge(merge) => merge.output,
            Operation::ExecStatement { outputs, .. } => outputs[0].var,
            _ => unreachable!(),
        };

        let final_node_id = graph.nodes.len();
        graph.nodes.push(MaterializationNode {
            id: final_node_id,
            operation: Operation::Project(plan::Project {
                input: final_var,
                output: plan::Output {
                    ty: /* ... */,
                    targets: vec![plan::OutputTarget { var: dst, project: None }],
                },
            }),
            dependencies: vec![root_output_node],
        });

        // Compute execution order for all nodes
        graph.compute_execution_order();

        // Execute all operations in topological order
        for node_id in &graph.execution_order {
            let node = &graph.nodes[*node_id];
            match &node.operation {
                Operation::ExecStatement { stmt, input_var, outputs } => {
                    let mut output_targets = Vec::new();
                    for output in outputs {
                        output_targets.push(plan::OutputTarget {
                            var: output.var,
                            project: self.build_projection(&output.expr, /* types */),
                        });
                    }

                    self.push_action(plan::ExecStatement {
                        input: input_var.map(|var| plan::Input { var }),
                        output: Some(plan::Output {
                            ty: /* materialized record type */,
                            targets: output_targets,
                        }),
                        stmt: stmt.clone(),
                        conditional_update_with_no_returning: false,
                    });
                }
                Operation::NestedMerge(merge) => {
                    self.push_action(merge.clone());
                }
                Operation::Project(project) => {
                    self.push_action(project.clone());
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
                input_var: None,  // Will be set later based on parent
                outputs: vec![],  // Will be populated by compute_materializations()
            },
            dependencies: vec![],  // Will be set later based on parent exec node
        });
        self.stmts[stmt_id.0].exec_node = Some(exec_node_id);

        // Create NestedMerge node for this statement (if it has children or needs projection)
        self.plan_merge_for_stmt(graph, stmt_id);
    }

    fn compute_materializations(&mut self, graph: &mut MaterializationGraph) {
        // Transform queries and compute outputs for all ExecStatement nodes
        for stmt_id in 0..self.stmts.len() {
            let stmt_state = &self.stmts[stmt_id];
            let exec_node_id = stmt_state.exec_node.unwrap();
            let node = &mut graph.nodes[exec_node_id];

            let Operation::ExecStatement { stmt, input_var, outputs } = &mut node.operation else {
                unreachable!()
            };

            // TRANSFORMATION STEP: Rewrite the query to use VALUES(arg(0)) pattern
            // This is where we apply the EXISTS subquery transformation
            if let Some(parent_stmt_id) = self.find_parent_stmt(StmtId(stmt_id)) {
                let parent_exec_node_id = self.stmts[parent_stmt_id.0].exec_node.unwrap();

                // Transform child query to use VALUES(arg(0)) in EXISTS clause
                self.transform_to_values_pattern(stmt, &self.stmts[stmt_id]);

                // Set input_var to parent's join column output
                let parent_exec_node = &graph.nodes[parent_exec_node_id];
                let Operation::ExecStatement { outputs: parent_outputs, .. } = &parent_exec_node.operation else {
                    unreachable!()
                };
                *input_var = Some(parent_outputs[1].var);  // Second output is join columns

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

    fn plan_merge_for_stmt(
        &mut self,
        graph: &mut MaterializationGraph,
        stmt_id: StmtId,
    ) {
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

        let merge_node_id = graph.nodes.len();
        let output_var = self.var_table.register_var(/* projected type */);

        let exec_node_id = stmt_state.exec_node.unwrap();

        if children.is_empty() {
            // Leaf node - no merges needed, just need to project materialization
            let projection = self.build_projection(&stmt_state.projection, /* types */);

            graph.nodes.push(MaterializationNode {
                id: merge_node_id,
                operation: Operation::NestedMerge(plan::NestedMerge {
                    root: /* Will be set after compute_materializations(), use exec outputs[0] */,
                    nested: vec![],  // No nested collections for leaf
                    output: output_var,
                    projection,
                }),
                dependencies: vec![exec_node_id],  // Depends on its own ExecStatement
            });
        } else {
            // Non-leaf node: create ONE operation that handles ALL children
            let mut nested_collections = Vec::new();
            let mut dependencies = vec![exec_node_id];  // Always depends on its own ExecStatement

            // Gather all nested collections from StatementState
            for (arg_idx, child_stmt_id) in children {
                // Get the child's output node (its NestedMerge)
                let child_output_node_id = self.stmts[child_stmt_id.0].output_node.unwrap();

                // Extract merge qualification from the nested statement's WHERE clause
                let qualification = self.extract_merge_qualification(stmt_id, child_stmt_id);

                nested_collections.push(plan::NestedCollection {
                    source: /* Will be filled from child's output after nodes are built */,
                    arg_index: arg_idx,  // ExprArg position from StatementState.args
                    qualification,
                });

                // Add dependency on child's output node
                dependencies.push(child_output_node_id);
            }

            // Build projection function from StatementState.projection
            let projection = self.build_merge_projection(&stmt_state.projection, /* types */);

            graph.nodes.push(MaterializationNode {
                id: merge_node_id,
                operation: Operation::NestedMerge(plan::NestedMerge {
                    root: /* Will be set after compute_materializations(), use exec outputs[0] */,
                    nested: nested_collections,
                    output: output_var,
                    projection,
                }),
                dependencies,
            });
        }

        // Record this as the output node for this statement
        self.stmts[stmt_id.0].output_node = Some(merge_node_id);
    }

    fn extract_merge_qualification(
        &self,
        parent_stmt: StmtId,
        child_stmt: StmtId,
    ) -> MergeQualification {
        // Look at the child statement's WHERE clause
        // It should contain the correlation condition (e.g., posts.user_id = users.id)
        // This was added during partitioning when creating the EXISTS subquery
        //
        // IMPORTANT: The correlation condition appears in TWO places:
        // 1. In the materialization query's EXISTS clause (for batch loading)
        // 2. We extract it here to use during the merge (for per-record filtering)
        //
        // Example: For posts materialized with EXISTS(...WHERE posts.user_id = users.id...)
        // We extract: root_columns=[0] (users.id), nested_columns=[1] (posts.user_id)

        let child_query = &self.stmts[child_stmt.0].stmt;

        // Try to extract equality condition on columns from the correlation
        if let Some(equality) = self.try_extract_equality(child_query) {
            // Use equality qualification -> will use hash index
            MergeQualification::Equality {
                root_columns: equality.parent_columns,
                nested_columns: equality.child_columns,
            }
        } else {
            // Fall back to general predicate -> will use nested loop
            // Build an Expr that evaluates the correlation condition
            let predicate = self.build_correlation_predicate(parent_stmt, child_stmt);
            MergeQualification::Predicate(predicate)
        }
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

### 2.1 Example: Planning User -> Posts -> Tags

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

  - Stmt0: SELECT * FROM users WHERE ...
    RETURNING { id: users.id, name: users.name, posts: ExprArg(0) }
    children: [(arg_index: 0, Stmt1)]

  - Stmt1: SELECT * FROM posts WHERE EXISTS (SELECT 1 FROM [Stmt0] WHERE posts.user_id = users.id)
    RETURNING { id: posts.id, title: posts.title, tags: ExprArg(0) }
    children: [(arg_index: 0, Stmt2)]

  - Stmt2: SELECT * FROM tags WHERE EXISTS (SELECT 1 FROM [Stmt1] WHERE tags.post_id = posts.id)
    RETURNING { id: tags.id, name: tags.name }
    children: []

Step 2: Build graph nodes (inside-out)
The graph structure combines database queries and post-processing operations:

  Graph nodes (created in inside-out order):

  Node 0: ExecStatement (Tags query)
    stmt: SELECT * FROM tags WHERE EXISTS (...)
    input_var: Some(var_1_ids)  // From posts exec
    outputs: [
      Output { var: var_2_full, expr: { id: tags.id, name: tags.name } }
    ]
    dependencies: []  // Will be set to [Node 2] after parent is built

  Node 1: NestedMerge (Tags projection - leaf)
    root: var_2_full
    nested: []
    output: var_3
    projection: Func([tags_record] -> { id: tags.id, name: tags.name })
    dependencies: [0]  // Depends on Node 0 (Tags exec)

  Node 2: ExecStatement (Posts query)
    stmt: SELECT * FROM posts WHERE EXISTS (...)
    input_var: Some(var_0_ids)  // From users exec
    outputs: [
      Output { var: var_4_full, expr: { id: posts.id, title: posts.title } },
      Output { var: var_1_ids, expr: posts.id }  // For tags query arg(0)
    ]
    dependencies: []  // Will be set to [Node 4] after parent is built

  Node 3: NestedMerge (Merge tags into posts)
    root: var_4_full
    nested: [
      NestedCollection {
        source: var_3,  // From Node 1 (projected tags)
        arg_index: 0,
        qualification: Equality { root_columns: [0], nested_columns: [1] }
      }
    ]
    output: var_5
    projection: Func([post_record, filtered_tags] -> { id, title, tags })
    dependencies: [1, 2]  // Depends on Node 1 (tags merge) and Node 2 (posts exec)

  Node 4: ExecStatement (Users query)
    stmt: SELECT * FROM users WHERE users.active = true
    input_var: None  // Root query
    outputs: [
      Output { var: var_6_full, expr: { id: users.id, name: users.name } },
      Output { var: var_0_ids, expr: users.id }  // For posts query arg(0)
    ]
    dependencies: []  // No dependencies

  Node 5: NestedMerge (Merge posts into users)
    root: var_6_full
    nested: [
      NestedCollection {
        source: var_5,  // From Node 3 (posts with tags)
        arg_index: 0,
        qualification: Equality { root_columns: [0], nested_columns: [1] }
      }
    ]
    output: var_7
    projection: Func([user_record, filtered_posts] -> { id, name, posts })
    dependencies: [3, 4]  // Depends on Node 3 (posts merge) and Node 4 (users exec)

  Node 6: Project (Final output)
    input: var_7
    output: dst
    dependencies: [5]  // Depends on Node 5 (users merge)

Step 3: Fix dependencies after all nodes are built
After building nodes inside-out, update ExecStatement dependencies:
  Node 0.dependencies = [2]  // Tags exec depends on posts exec (for var_1_ids)
  Node 2.dependencies = [4]  // Posts exec depends on users exec (for var_0_ids)

Step 4: Compute execution order
  Topological sort: [4, 2, 0, 1, 3, 5, 6]

  Execution sequence:
    4: ExecStatement(Users) -> [var_6_full, var_0_ids]
    2: ExecStatement(Posts, arg: var_0_ids) -> [var_4_full, var_1_ids]
    0: ExecStatement(Tags, arg: var_1_ids) -> [var_2_full]
    1: NestedMerge(Tags projection) -> var_3
    3: NestedMerge(Merge tags into posts) -> var_5
    5: NestedMerge(Merge posts into users) -> var_7
    6: Project(var_7) -> dst

StatementState tracking:
  Stmt0 (Users): exec_node=4, output_node=5
  Stmt1 (Posts): exec_node=2, output_node=3
  Stmt2 (Tags): exec_node=0, output_node=1

Key efficiency gains:
- **Single unified graph** contains all operations (queries + merges + projections)
- **Dependencies are explicit** in the graph structure
- **Single query produces multiple outputs** (full records + join columns)
- **Child queries receive projected parent results** as runtime args
- **StatementState stores node IDs** for O(1) lookups (no hash maps needed)
```

### 3. Execution Phase

In `engine/exec/nested_merge.rs` (new file):

```rust
impl Exec<'_> {
    pub(super) async fn action_nested_merge(
        &mut self,
        action: &plan::NestedMerge
    ) -> Result<()> {
        // Load root materialization
        let root_records = self.vars.load(action.root).collect().await?;

        // Load all nested collections and build indices
        let mut collections = Vec::with_capacity(action.nested.len());

        for nested_collection in &action.nested {
            let records = self.vars.load(nested_collection.source).collect().await?;

            // Build index based on qualification type
            let index = match &nested_collection.qualification {
                MergeQualification::Equality { nested_columns, .. } => {
                    Some(self.build_hash_index(&records, nested_columns)?)
                }
                MergeQualification::Predicate(_) => None,
            };

            collections.push(LoadedCollection {
                records,
                index,
                arg_index: nested_collection.arg_index,
                qualification: &nested_collection.qualification,
            });
        }

        // Process each root record
        let mut results = Vec::with_capacity(root_records.len());

        for root_record in root_records {
            // Filter all nested collections for this root record
            // Build arguments array: [root_record, filtered_0, filtered_1, ...]
            let mut projection_args = vec![root_record.clone()];

            // Collections may not be in arg_index order, so build a sparse array
            let max_arg = collections.iter().map(|c| c.arg_index).max().unwrap_or(0);
            let mut filtered_collections = vec![stmt::Value::Null; max_arg + 1];

            for collection in &collections {
                let filtered = self.filter_nested_for_root(
                    &root_record,
                    &collection.records,
                    collection.qualification,
                    collection.index.as_ref(),
                )?;

                filtered_collections[collection.arg_index] = stmt::Value::List(filtered);
            }

            // Append filtered collections to projection args
            projection_args.extend(filtered_collections);

            // Apply projection: [root_record, filtered_0, filtered_1, ...] -> final_record
            let projected = action.projection.eval(&projection_args)?;

            results.push(projected);
        }

        // Store output
        self.vars.store(action.output, ValueStream::from_vec(results));
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
            index
                .entry(key)
                .or_insert_with(Vec::new)
                .push(record.clone());
        }

        Ok(index)
    }

    fn filter_nested_for_root(
        &self,
        root_record: &stmt::Value,
        nested_records: &[stmt::Value],
        qualification: &MergeQualification,
        index: Option<&HashMap<CompositeKey, Vec<stmt::Value>>>,
    ) -> Result<Vec<stmt::Value>> {
        match qualification {
            MergeQualification::Equality { root_columns, .. } => {
                // Use hash index (should always be present for Equality)
                let hash_index = index.expect("Hash index should exist for Equality qualification");
                let root_rec = root_record.expect_record();
                let key = self.extract_key(root_rec, root_columns)?;

                Ok(hash_index
                    .get(&key)
                    .map(|v| v.clone())
                    .unwrap_or_else(Vec::new))
            }
            MergeQualification::Predicate(predicate) => {
                // Evaluate predicate for each nested record
                let mut matches = Vec::new();

                for nested_record in nested_records {
                    // Args: [root_record, nested_record] -> bool
                    if predicate.eval_bool(&[root_record.clone(), nested_record.clone()])? {
                        matches.push(nested_record.clone());
                    }
                }

                Ok(matches)
            }
        }
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

// Helper struct for loaded nested collections
struct LoadedCollection<'a> {
    records: Vec<stmt::Value>,
    index: Option<HashMap<CompositeKey, Vec<stmt::Value>>>,
    arg_index: usize,
    qualification: &'a MergeQualification,
}

// Composite key type for multi-column equality
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
  4. NestedMerge(Project Tags) -> var_3
  5. NestedMerge(Merge Tags into Posts) -> var_4
  6. NestedMerge(Merge Posts into Users) -> var_5

Execution trace:

Step 1-3: Execute materialization queries
  var_0 = [
    { id: 1, name: "Alice" },  // Raw user materialization
    { id: 2, name: "Bob" },
  ]

  var_1 = [
    { id: 10, user_id: 1, title: "Post1" },  // Raw post materialization
    { id: 11, user_id: 1, title: "Post2" },
    { id: 12, user_id: 2, title: "Post3" },
  ]

  var_2 = [
    { id: 100, post_id: 10, name: "rust" },  // Raw tag materialization
    { id: 101, post_id: 10, name: "async" },
    { id: 102, post_id: 12, name: "performance" },
  ]

Step 4: NestedMerge(Project Tags) - Leaf node projection
  action = NestedMerge {
    root: var_2,
    nested: var_2,  // Unused
    qualification: Predicate(always_true),  // No filtering for leaf
    projection: Func([tag_record, _] -> { id: tag.id, name: tag.name }),
  }

  Execution:
    For each tag in var_2:
      - No filtering (predicate always true)
      - Apply projection to each tag

    Result:
      var_3 = [
        { id: 100, name: "rust" },
        { id: 101, name: "async" },
        { id: 102, name: "performance" },
      ]

Step 5: NestedMerge(Merge Tags into Posts)
  action = NestedMerge {
    root: var_1,  // Posts materialization
    nested: var_3,  // Projected tags
    qualification: Equality {
      root_columns: [0],     // posts.id
      nested_columns: [1],   // tags.post_id (from WHERE clause correlation)
    },
    projection: Func([post_record, filtered_tags] -> {
      id: post.id,
      title: post.title,
      tags: filtered_tags  // Binds to ExprArg(0) in returning clause
    }),
  }

  Execution:
    1. Build hash index on Tags (var_3) keyed by tags.post_id:
       (Equality qualification → automatically uses hash index)
       index = {
         10 -> [Tag{id:100, name:"rust"}, Tag{id:101, name:"async"}],
         12 -> [Tag{id:102, name:"performance"}],
       }

    2. For each post in var_1:
       Post{id:10, user_id:1, title:"Post1"}:
         - Extract root_key = post.id = 10
         - Lookup in index: filtered_tags = [Tag{100}, Tag{101}]
         - Apply projection([post_record, filtered_tags]) ->
           { id: 10, title: "Post1", tags: [Tag{100}, Tag{101}] }

       Post{id:11, user_id:1, title:"Post2"}:
         - Extract root_key = 11
         - Lookup in index: filtered_tags = []
         - Apply projection([post_record, []]) ->
           { id: 11, title: "Post2", tags: [] }

       Post{id:12, user_id:2, title:"Post3"}:
         - Extract root_key = 12
         - Lookup in index: filtered_tags = [Tag{102}]
         - Apply projection([post_record, filtered_tags]) ->
           { id: 12, title: "Post3", tags: [Tag{102}] }

    3. Store result:
       var_4 = [
         { id: 10, title: "Post1", tags: [Tag{100}, Tag{101}] },
         { id: 11, title: "Post2", tags: [] },
         { id: 12, title: "Post3", tags: [Tag{102}] },
       ]

    4. Drop index (free memory)

Step 6: NestedMerge(Merge Posts into Users)
  action = NestedMerge {
    root: var_0,  // Users materialization
    nested: var_4,  // Posts-with-Tags
    qualification: Equality {
      root_columns: [0],     // users.id
      nested_columns: [1],   // posts.user_id (from WHERE clause correlation)
    },
    projection: Func([user_record, filtered_posts] -> {
      id: user.id,
      name: user.name,
      posts: filtered_posts  // Binds to ExprArg(0) in returning clause
    }),
  }

  Execution:
    1. Build hash index on Posts-with-Tags (var_4) keyed by posts.user_id:
       (Equality qualification → automatically uses hash index)
       index = {
         1 -> [Post{id:10, tags:[...]}, Post{id:11, tags:[]}],
         2 -> [Post{id:12, tags:[...]}],
       }

    2. For each user in var_0:
       User{id:1, name:"Alice"}:
         - Extract root_key = user.id = 1
         - Lookup in index: filtered_posts = [Post{10}, Post{11}]
         - Apply projection([user_record, filtered_posts]) ->
           { id: 1, name: "Alice", posts: [Post{10, ...}, Post{11, ...}] }

       User{id:2, name:"Bob"}:
         - Extract root_key = 2
         - Lookup in index: filtered_posts = [Post{12}]
         - Apply projection([user_record, filtered_posts]) ->
           { id: 2, name: "Bob", posts: [Post{12, ...}] }

    3. Store result:
       var_5 = [
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

    4. Drop index (free memory)

Final: Project var_5 to destination
```

**Key Points:**
- Each merge processes one root record at a time
- Filtered nested records are passed as arguments to the projection function
- The projection function evaluates the returning clause with ExprArgs bound to filtered records
- Indices are built once and used for all root records (amortized cost)
- Inside-out execution ensures child records are fully projected before being merged into parents
- A single NestedMerge can handle multiple collections at the same level

### 3.2 Example: User with Multiple Collections (Posts AND Comments)

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