use std::{
    cell::{Cell, OnceCell},
    collections::{HashMap, HashSet},
    ops,
};

use index_vec::IndexVec;
use indexmap::IndexSet;
use toasty_core::stmt;

use crate::engine::mir;

#[derive(Debug)]
pub(crate) struct HirStatement {
    store: Store,
}

impl HirStatement {
    pub(super) fn new(store: Store) -> HirStatement {
        HirStatement { store }
    }

    pub(super) fn into_store(self) -> Store {
        self.store
    }
}

/// Planning metadata for a statement.
///
/// Not all statements have a `StatementInfo`. Only statements that execute as
/// separate operations in the query plan have one.
#[derive(Debug)]
pub(super) struct StatementInfo {
    /// Populated later
    pub(super) stmt: Option<Box<stmt::Statement>>,

    /// Statements that this statement depends on. The result is not needed, but
    /// dependencies need to execute first for consistency.
    pub(super) deps: HashSet<StmtId>,

    /// Statement arguments
    pub(super) args: Vec<Arg>,

    /// Back-refs are expressions within sub-statements that reference the
    /// current statemetn.
    pub(super) back_refs: HashMap<StmtId, BackRef>,

    /// Node ID of the operation that executes this statement's database query.
    pub(super) exec_statement: Cell<Option<mir::NodeId>>,

    /// Columns selected by exec_statement
    pub(super) exec_statement_selection: OnceCell<IndexSet<stmt::ExprReference>>,

    /// This statement's node ID representing the final computation.
    pub(super) output: Cell<Option<mir::NodeId>>,
}

/// StatementInfo store
#[derive(Debug)]
pub(super) struct Store {
    store: IndexVec<StmtId, StatementInfo>,
}

index_vec::define_index_type! {
    pub(crate) struct StmtId = u32;
}

impl StatementInfo {
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
        store: &'a Store,
    ) -> impl Iterator<Item = mir::NodeId> + 'a {
        self.deps
            .iter()
            .map(|stmt_id| store[stmt_id].output.get().unwrap())
    }
}

#[derive(Debug, Default)]
pub(super) struct BackRef {
    /// Column expressions from this statement that are referenced by a child statement.
    ///
    /// When a child statement references columns from this statement (via `Arg::Ref`),
    /// those columns must be included in this statement's batch-load query. This set
    /// tracks which columns need to be loaded so they can be used during nested merge.
    pub(super) exprs: IndexSet<stmt::ExprReference>,

    /// Node ID of the projection operation that extracts these back-ref columns.
    ///
    /// After executing this statement, a projection node is created to extract just
    /// the columns needed by child statements. This projection's output is used as
    /// input to the child statement's batch-load operation.
    pub(super) node_id: Cell<Option<mir::NodeId>>,
}

#[derive(Debug)]
pub(super) enum Arg {
    /// A sub-statement argument.
    Sub {
        /// The statement ID that provides the data for this argument.
        stmt_id: StmtId,

        /// True when the sub-statement is used in the returning clause, false when used in filters.
        ///
        /// Determines how the sub-statement is handled during planning:
        /// - `true`: Data is merged with parent rows via `NestedMerge`
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

impl Store {
    pub(super) fn new() -> Store {
        Store {
            store: IndexVec::new(),
        }
    }

    pub(super) fn insert(&mut self, info: StatementInfo) -> StmtId {
        self.store.push(info)
    }

    pub(super) fn new_statement_info(&mut self, deps: HashSet<StmtId>) -> StmtId {
        self.insert(StatementInfo::new(deps))
    }

    pub(super) fn root_id(&self) -> StmtId {
        StmtId::from(0)
    }

    pub(super) fn root(&self) -> &StatementInfo {
        let root_id = self.root_id();
        &self.store[root_id]
    }
}

impl ops::Index<StmtId> for Store {
    type Output = StatementInfo;

    fn index(&self, index: StmtId) -> &Self::Output {
        self.store.index(index)
    }
}

impl ops::IndexMut<StmtId> for Store {
    fn index_mut(&mut self, index: StmtId) -> &mut Self::Output {
        self.store.index_mut(index)
    }
}

impl ops::Index<&StmtId> for Store {
    type Output = StatementInfo;

    fn index(&self, index: &StmtId) -> &Self::Output {
        self.store.index(*index)
    }
}
