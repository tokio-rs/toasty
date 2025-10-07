use toasty_core::stmt::{self, visit_mut, VisitMut};

use crate::engine::planner::ng::{StatementInfo, StmtId};

pub(crate) fn decompose(mut stmt: stmt::Statement) -> Vec<StatementInfo> {
    let mut walker_state = WalkerState {
        stmts: vec![StatementInfo::new()],
        scopes: vec![ScopeState { stmt_id: StmtId(0) }],
    };

    // Map the statement
    Walker {
        state: &mut walker_state,
        scope: 0,
        returning: false,
    }
    .visit_stmt_mut(&mut stmt);

    walker_state.stmts[0].stmt = Some(Box::new(stmt));

    walker_state.stmts
}

struct Walker<'a> {
    /// Partitioning state
    state: &'a mut WalkerState,
    scope: usize,
    returning: bool,
}

#[derive(Debug)]
struct WalkerState {
    /// Statements to be executed by the database, though they may still be
    /// broken down into multiple sub-statements.
    stmts: Vec<StatementInfo>,

    /// Scope state
    scopes: Vec<ScopeState>,
}

#[derive(Debug)]
struct ScopeState {
    /// Identifier of the statement in the partitioner state.
    stmt_id: StmtId,
}

impl<'a> visit_mut::VisitMut for Walker<'a> {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        match i {
            stmt::Expr::Reference(expr_reference) => {
                // At this point, the query should have been fully lowered
                let stmt::ExprReference::Column {
                    nesting,
                    table,
                    column,
                } = expr_reference
                else {
                    panic!("unexpected state: statement not lowered")
                };

                if *nesting > 0 {
                    let stmt_id = self.curr_stmt_id();
                    let target_id = self.state.scopes[self.scope - *nesting].stmt_id;

                    // The reference is recreated assuming it is evaluated from the target context.
                    let expr = stmt::ExprReference::Column {
                        nesting: 0,
                        table: *table,
                        column: *column,
                    };

                    let batch_load_index = self.stmt(target_id).new_back_ref(stmt_id, expr);
                    let arg_id =
                        self.curr_stmt()
                            .new_ref_arg(target_id, *nesting, batch_load_index);

                    // Using ExprArg as a placeholder. It will be rewritten
                    // later.
                    *i = stmt::Expr::arg(arg_id);
                }
            }
            stmt::Expr::Stmt(expr_stmt) => {
                assert!(self.returning);
                // For now, we assume nested sub-statements cannot be executed on the
                // target database. Eventually, we will need to make this smarter.

                // Create a `StatementState` to track the sub-statement
                let target_id = self.new_stmt();
                let mut scope = self.scope(target_id);
                visit_mut::visit_expr_stmt_mut(&mut scope, expr_stmt);
                self.state.scopes.pop();

                // Create a new input to receive the statement
                let arg_id = self.curr_stmt().new_sub_stmt_arg(target_id);

                // Replace the sub-statement expression with a placeholder tracking the input
                let expr = std::mem::replace(i, stmt::Expr::arg(arg_id));
                let stmt::Expr::Stmt(expr_stmt) = expr else {
                    panic!()
                };
                self.stmt(target_id).stmt = Some(expr_stmt.stmt);
            }
            _ => {
                visit_mut::visit_expr_mut(self, i);
            }
        }
    }

    fn visit_returning_mut(&mut self, i: &mut stmt::Returning) {
        assert!(!self.returning);
        self.returning = true;
        visit_mut::visit_returning_mut(self, i);
        self.returning = false;
    }
}

impl<'a> Walker<'a> {
    fn new_stmt(&mut self) -> StmtId {
        let stmt_id = StmtId(self.state.stmts.len());
        self.state.stmts.push(StatementInfo::new());
        stmt_id
    }

    fn scope<'child>(&'child mut self, stmt_id: StmtId) -> Walker<'child> {
        let scope = self.state.scopes.len();
        self.state.scopes.push(ScopeState { stmt_id });

        Walker {
            state: self.state,
            scope,
            returning: false,
        }
    }

    fn curr_stmt_id(&self) -> StmtId {
        self.state.scopes[self.scope].stmt_id
    }

    fn curr_stmt(&mut self) -> &mut StatementInfo {
        &mut self.state.stmts[self.state.scopes[self.scope].stmt_id.0]
    }

    fn stmt(&mut self, stmt_id: StmtId) -> &mut StatementInfo {
        &mut self.state.stmts[stmt_id.0]
    }
}
