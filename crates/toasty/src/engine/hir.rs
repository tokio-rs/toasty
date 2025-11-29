use std::{
    cell::{Cell, OnceCell},
    collections::{HashMap, HashSet},
    ops,
};

use index_vec::IndexVec;
use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::mir;

/// High-level Intermediate Representation of a query.
///
/// [`HirStatement`] is a collection of related statements with tracked
/// dependencies, produced by the lowering phase. It captures the structure of
/// a query after model-to-table transformation but before operation graph
/// construction.
///
/// The HIR tracks which statements depend on results from others, enabling the
/// planner to determine execution order and identify opportunities for batching.
/// The dependency graph may contain cycles when preloading associations (e.g.,
/// users → todos → users), which the planning phase must break when building
/// the final DAG.
#[derive(Debug)]
pub(super) struct HirStatement {
    /// Storage for all statement metadata, indexed by [`StmtId`].
    store: IndexVec<StmtId, StatementInfo>,
}

/// Metadata for a single statement within the HIR.
///
/// Each [`StatementInfo`] represents one database operation that will execute
/// as part of the query. Not every sub-expression in the original query becomes
/// a [`StatementInfo`]; only those that execute as separate database operations
/// (e.g., the root query and any `include()` subqueries).
///
/// It tracks the statement's dependencies on other statements, arguments passed
/// from parent or child statements, and references to the MIR nodes created
/// during planning.
#[derive(Debug)]
pub(super) struct StatementInfo {
    /// The lowered statement to execute.
    ///
    /// Initially `None`, populated during lowering after all transformations
    /// are complete. Contains the table-level statement ready for planning.
    pub(super) stmt: Option<Box<stmt::Statement>>,

    /// Statement IDs that must execute before this statement.
    ///
    /// Dependencies ensure execution order for consistency, even when this
    /// statement does not consume the dependency's result. For example, an
    /// `UPDATE` may depend on a prior `INSERT` to maintain referential
    /// integrity.
    pub(super) deps: HashSet<StmtId>,

    /// Arguments that flow into this statement from other statements.
    ///
    /// Each [`Arg`] describes data passed from a sub-statement ([`Arg::Sub`]) or
    /// referenced from a parent statement ([`Arg::Ref`]). During planning, these
    /// arguments become edges in the operation graph.
    pub(super) args: Vec<Arg>,

    /// Column references from child statements that point back to this one.
    ///
    /// When a child statement references columns from this statement (via
    /// [`Arg::Ref`]), those columns must be included in this statement's
    /// batch-load query. The key is the child's [`StmtId`], and the value
    /// tracks which columns are referenced.
    pub(super) back_refs: HashMap<StmtId, BackRef>,

    /// MIR node that executes this statement's database query.
    ///
    /// Set during planning when the statement is converted to an operation.
    /// Used to wire up dependencies between operations.
    pub(super) exec_statement: Cell<Option<mir::NodeId>>,

    /// Columns selected by the `exec_statement` operation.
    ///
    /// Populated during planning to track which columns are fetched from the
    /// database. Used to resolve column references in child statements.
    pub(super) exec_statement_selection: OnceCell<IndexSet<stmt::ExprReference>>,

    /// MIR node representing this statement's final output.
    ///
    /// May differ from `exec_statement` when post-processing is required
    /// (e.g., filtering, projection, or nested merge). Dependent statements
    /// reference this node to establish execution order.
    pub(super) output: Cell<Option<mir::NodeId>>,
}

index_vec::define_index_type! {
    pub(crate) struct StmtId = u32;
}

impl StatementInfo {
    /// Creates a new [`StatementInfo`] with the given dependencies.
    ///
    /// All other fields are initialized to empty or `None` and populated
    /// later during lowering and planning.
    pub(super) fn new(deps: HashSet<StmtId>) -> StatementInfo {
        StatementInfo {
            stmt: None,
            deps,
            args: vec![],
            back_refs: HashMap::new(),
            exec_statement: Cell::new(None),
            exec_statement_selection: OnceCell::new(),
            output: Cell::new(None),
        }
    }

    /// Returns an iterator over the node IDs that this statement depends on.
    ///
    /// Dependencies must execute before this statement, even if their results
    /// are not directly consumed. For example, an UPDATE may depend on a prior
    /// INSERT to maintain referential integrity.
    ///
    /// Each dependency is represented by its output node ID.
    pub(super) fn dependent_operations<'a>(
        &'a self,
        hir: &'a HirStatement,
    ) -> impl Iterator<Item = mir::NodeId> + 'a {
        self.deps
            .iter()
            .map(|stmt_id| hir[stmt_id].output.get().unwrap())
    }
}

/// Tracks columns referenced by a child statement.
///
/// When a child statement (e.g., an `include()` subquery) references columns
/// from its parent, the parent must include those columns in its result set.
/// [`BackRef`] records which columns are needed and the MIR node that projects
/// them for the child's batch-load operation.
#[derive(Debug, Default)]
pub(super) struct BackRef {
    /// Column expressions from this statement that are referenced by a child statement.
    ///
    /// When a child statement references columns from this statement (via
    /// [`Arg::Ref`]), those columns must be included in this statement's
    /// batch-load query. This set tracks which columns need to be loaded so they
    /// can be used during nested merge.
    pub(super) exprs: IndexSet<stmt::ExprReference>,

    /// Node ID of the projection operation that extracts these back-ref columns.
    ///
    /// After executing this statement, a projection node is created to extract just
    /// the columns needed by child statements. This projection's output is used as
    /// input to the child statement's batch-load operation.
    pub(super) node_id: Cell<Option<mir::NodeId>>,
}

/// An argument that flows between statements in the HIR.
///
/// Arguments represent data dependencies between statements. They describe how
/// results from one statement are used by another, enabling the planner to wire
/// up the operation graph correctly.
#[derive(Debug)]
pub(super) enum Arg {
    /// Data from a sub-statement that feeds into this statement.
    Sub {
        /// The statement ID that provides the data for this argument.
        stmt_id: StmtId,

        /// True when the sub-statement is used in the returning clause, false when used in filters.
        ///
        /// Determines how the sub-statement is handled during planning:
        /// - `true`: Data is merged with parent rows via [`NestedMerge`](crate::engine::mir::NestedMerge)
        /// - `false`: Data is used as input to filter expressions
        returning: bool,

        /// Index in the operation's inputs list. Set during planning.
        input: Cell<Option<usize>>,
    },

    /// A reference to a parent statement's columns.
    Ref {
        /// The parent statement that provides the data for this reference.
        stmt_id: StmtId,

        /// Number of nesting levels between this statement and the referenced parent.
        ///
        /// A value of 1 means the immediate parent, 2 means the grandparent, etc.
        nesting: usize,

        /// Index of this column in the parent's batch-load query results.
        ///
        /// The parent statement includes columns in its batch-load that are referenced
        /// by child statements. This is the index of this specific column in that set.
        batch_load_index: usize,

        /// Index in the operation's inputs list. Set during planning.
        input: Cell<Option<usize>>,
    },
}

impl HirStatement {
    /// Creates an empty [`HirStatement`] with no statements.
    pub(super) fn new() -> HirStatement {
        HirStatement {
            store: IndexVec::new(),
        }
    }

    /// Inserts a [`StatementInfo`] and returns its assigned [`StmtId`].
    pub(super) fn insert(&mut self, info: StatementInfo) -> StmtId {
        self.store.push(info)
    }

    /// Creates and inserts a new [`StatementInfo`] with the given dependencies.
    pub(super) fn new_statement_info(&mut self, deps: HashSet<StmtId>) -> StmtId {
        self.insert(StatementInfo::new(deps))
    }

    /// Returns the [`StmtId`] of the root statement.
    ///
    /// The root statement is always the first one inserted (index 0) and
    /// represents the top-level query that produces the final result.
    pub(super) fn root_id(&self) -> StmtId {
        StmtId::from(0)
    }

    /// Returns a reference to the root [`StatementInfo`].
    pub(super) fn root(&self) -> &StatementInfo {
        let root_id = self.root_id();
        &self.store[root_id]
    }
}

impl ops::Index<StmtId> for HirStatement {
    type Output = StatementInfo;

    fn index(&self, index: StmtId) -> &Self::Output {
        self.store.index(index)
    }
}

impl ops::IndexMut<StmtId> for HirStatement {
    fn index_mut(&mut self, index: StmtId) -> &mut Self::Output {
        self.store.index_mut(index)
    }
}

impl ops::Index<&StmtId> for HirStatement {
    type Output = StatementInfo;

    fn index(&self, index: &StmtId) -> &Self::Output {
        self.store.index(*index)
    }
}
