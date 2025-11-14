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

/// Additional information needed for planning a statement for materialization.
/// Note, there is not a 1-1 mapping between `StatementInfo` and statements. A
/// `StatementInfo` is used for statements that need to be materialized
/// separately.
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

    /// This statement's ExecStatement materialization node ID.
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

    /// Returns an iterator over the materialization node IDs that this statement
    /// depends on.
    ///
    /// Dependencies must execute before this statement for consistency, even if
    /// their results are not directly consumed. For example, an UPDATE operation
    /// may depend on a prior INSERT completing first to maintain referential
    /// integrity.
    ///
    /// Each dependency is represented by its output node ID - the final
    /// computation node that produces the dependency's result.
    pub(super) fn dependent_materializations<'a>(
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
    /// The expression
    pub(super) exprs: IndexSet<stmt::ExprReference>,

    /// Projection materialization node ID
    pub(super) node_id: Cell<Option<mir::NodeId>>,
}

#[derive(Debug)]
pub(super) enum Arg {
    /// A sub-statement
    Sub {
        /// The statement ID providing the input
        stmt_id: StmtId,

        /// True when the sub is used in the returning clause
        returning: bool,

        /// The index in the materialization node's inputs list. This is set
        /// when planning materialization.
        input: Cell<Option<usize>>,
    },

    /// A reference to a parent statement.
    Ref {
        /// The statement providing the data for the reference
        stmt_id: StmtId,

        /// The nesting level
        nesting: usize,

        /// The index of the column within the set of columns included during
        /// the batch-load query.
        batch_load_index: usize,

        /// The index in the materialization node's inputs list. This is set
        /// when planning materialization.
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
