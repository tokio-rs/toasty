use std::collections::{HashMap, HashSet};

use toasty_core::stmt::{self, visit, ExprReference, Visit};

use crate::engine::planner::Planner;

impl Planner<'_> {
    pub(crate) fn partition(&self, stmt: stmt::Query) -> stmt::Query {
        let stmt = stmt::Statement::Query(stmt);
        println!("stmt={stmt:#?}");

        let mut stmts = HashMap::new();
        stmts.insert(StmtId::new(&stmt), StatementState::new());

        let mut state = State {
            stmts,
            scopes: vec![ScopeState {
                stmt_id: StmtId::new(&stmt),
            }],
        };
        Walker {
            state: &mut state,
            scope: 0,
        }
        .visit_stmt(&stmt);

        if state.stmts.len() > 1 {
            todo!("state={state:#?}");
        }

        let stmt::Statement::Query(stmt) = stmt else {
            todo!()
        };
        stmt
    }
}

#[derive(Debug)]
struct State {
    /// Statements to be executed by the database, though they may still be
    /// broken down into multiple sub-statements.
    stmts: HashMap<StmtId, StatementState>,

    /// Scope state
    scopes: Vec<ScopeState>,
}

/// Per-statement state
#[derive(Debug)]
struct StatementState {
    /// List of all sub-statements
    subs: HashSet<StmtId>,

    /// Maps reference expressions in the statement to other statements.
    input: HashMap<ExprId, StmtId>,

    /// Maps expressions in the statement's returning clause to the statements that depend on the output.
    output: HashMap<ExprId, StmtId>,
}

#[derive(Debug)]
struct ScopeState {
    /// Identifier of the statement in the partitioner state.
    stmt_id: StmtId,
}

struct Walker<'a> {
    /// Partitioning state
    state: &'a mut State,
    scope: usize,
}

#[derive(Debug, PartialEq, Clone, Copy, Hash, Eq)]
struct StmtId(usize);

#[derive(Debug, PartialEq, Clone, Copy, Hash, Eq)]
struct ExprId(usize);

impl<'a> visit::Visit for Walker<'a> {
    fn visit_expr(&mut self, i: &stmt::Expr) {
        match i {
            stmt::Expr::Reference(expr_reference) => {
                // At this point, the query should have been fully lowered
                let stmt::ExprReference::Column { nesting, .. } = expr_reference else {
                    panic!("unexpected state: statement not lowered")
                };

                if *nesting > 0 {
                    println!("NESTING > 0; scope={}", self.scope);
                    let stmt_id = self.stmt_id();
                    let target_id = self.state.scopes[self.scope - *nesting].stmt_id;

                    self.stmt(stmt_id)
                        .input
                        .insert(ExprId::from_expr(i), target_id);
                }
            }
            stmt::Expr::Stmt(expr_stmt) => {
                // For now, we assume nested sub-statements cannot be executed on the
                // target database. Eventually, we will need to make this smarter.

                // Create a `StatementState` to track the sub-statement
                let stmt_id = StmtId::new(&*expr_stmt.stmt);
                let mut stmt_state = StatementState::new();

                if let Some(expr_id) = stmt_expr_id(&*expr_stmt.stmt) {
                    stmt_state.output.insert(expr_id, self.stmt_id());
                }

                self.state.stmts.insert(stmt_id, stmt_state);

                let expr_id = ExprId::from_expr(i);

                // Create a new scope for walking the statement
                let mut scope = self.sub_stmt(expr_id, stmt_id);
                visit::visit_expr_stmt(&mut scope, expr_stmt);

                self.state.scopes.pop();
            }
            _ => {
                visit::visit_expr(self, i);
            }
        }
    }
}

impl<'a> Walker<'a> {
    fn sub_stmt<'child>(&'child mut self, expr_id: ExprId, stmt_id: StmtId) -> Walker<'child> {
        for scope in &self.state.scopes {
            self.state
                .stmts
                .get_mut(&scope.stmt_id)
                .unwrap()
                .subs
                .insert(stmt_id);
        }

        self.curr_stmt().input.insert(expr_id, stmt_id);

        self.scope(stmt_id)
    }

    fn scope<'child>(&'child mut self, stmt_id: StmtId) -> Walker<'child> {
        let scope = self.state.scopes.len();
        self.state.scopes.push(ScopeState { stmt_id });

        Walker {
            state: self.state,
            scope,
        }
    }

    fn stmt_id(&self) -> StmtId {
        self.state.scopes[self.scope].stmt_id
    }

    fn curr_stmt(&mut self) -> &mut StatementState {
        self.state
            .stmts
            .get_mut(&self.state.scopes[self.scope].stmt_id)
            .unwrap()
    }

    fn stmt(&mut self, stmt_id: StmtId) -> &mut StatementState {
        self.state.stmts.get_mut(&stmt_id).unwrap()
    }
}

impl StatementState {
    fn new() -> StatementState {
        StatementState {
            subs: HashSet::new(),
            input: HashMap::new(),
            output: HashMap::new(),
        }
    }
}

impl StmtId {
    fn new(stmt: &stmt::Statement) -> StmtId {
        StmtId(stmt as *const stmt::Statement as _)
    }
}

impl ExprId {
    fn from_expr(expr: &stmt::Expr) -> ExprId {
        ExprId(expr as *const _ as _)
    }

    fn from_expr_set(expr_set: &stmt::ExprSet) -> ExprId {
        ExprId(expr_set as *const _ as _)
    }

    fn from_returning(returning: &stmt::Returning) -> ExprId {
        let stmt::Returning::Expr(expr) = returning else {
            panic!()
        };
        ExprId::from_expr(expr)
    }
}

fn stmt_expr_id(stmt: &stmt::Statement) -> Option<ExprId> {
    match stmt {
        stmt::Statement::Query(query) => Some(ExprId::from_expr_set(&query.body)),
        stmt::Statement::Delete(delete) => delete.returning.as_ref().map(ExprId::from_returning),
        stmt::Statement::Insert(insert) => insert.returning.as_ref().map(ExprId::from_returning),
        stmt::Statement::Update(update) => update.returning.as_ref().map(ExprId::from_returning),
    }
}
