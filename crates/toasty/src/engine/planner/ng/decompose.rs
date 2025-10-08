use std::cell::Cell;

use toasty_core::{
    driver::Capability,
    stmt::{self, visit_mut, VisitMut},
};

use crate::engine::planner::ng::{Arg, StatementInfoStore, StmtId};

impl super::PlannerNg<'_, '_> {
    pub(crate) fn decompose(&mut self, mut stmt: stmt::Statement) {
        let root_id = self.store.root_id();

        let mut state = State {
            store: &mut self.store,
            scopes: vec![Scope { stmt_id: root_id }],
            capability: self.old.capability,
        };

        // Map the statement
        Decompose {
            state: &mut state,
            scope_id: 0,
            returning: false,
        }
        .visit_stmt_mut(&mut stmt);

        self.store.root_mut().stmt = Some(Box::new(stmt));
    }
}

struct Decompose<'a, 'b> {
    /// Partitioning state
    state: &'a mut State<'b>,
    scope_id: usize,
    returning: bool,
}

#[derive(Debug)]
struct State<'a> {
    /// Statements to be executed by the database, though they may still be
    /// broken down into multiple sub-statements.
    store: &'a mut StatementInfoStore,

    /// Scope state
    scopes: Vec<Scope>,

    /// Database capability
    capability: &'a Capability,
}

#[derive(Debug)]
struct Scope {
    /// Identifier of the statement in the partitioner state.
    stmt_id: StmtId,
}

impl visit_mut::VisitMut for Decompose<'_, '_> {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        match i {
            stmt::Expr::Reference(expr_reference) => {
                // At this point, the query should have been fully lowered
                let stmt::ExprReference::Column { nesting, .. } = expr_reference else {
                    panic!("unexpected state: statement not lowered")
                };

                if *nesting > 0 {
                    let source_id = self.scope_stmt_id();
                    let target_id = self.resolve_stmt_id(*nesting);

                    let position = self.state.new_ref(source_id, target_id, *expr_reference);

                    // Using ExprArg as a placeholder. It will be rewritten
                    // later.
                    *i = stmt::Expr::arg(position);
                }
            }
            stmt::Expr::Stmt(expr_stmt) => {
                assert!(self.returning);
                // For now, we assume nested sub-statements cannot be executed on the
                // target database. Eventually, we will need to make this smarter.

                let source_id = self.scope_stmt_id();
                let target_id = self.state.store.new_statement_info();

                self.scope(target_id, |child| {
                    visit_mut::visit_expr_stmt_mut(child, expr_stmt);
                });

                let position = self.state.new_sub_statement(source_id, target_id, i.take());
                *i = stmt::Expr::arg(position);
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

    fn visit_stmt_query_mut(&mut self, stmt: &mut stmt::Query) {
        if !self.state.capability.sql {
            assert!(stmt.order_by.is_none(), "TODO: implement ordering for KV");
            assert!(stmt.limit.is_none(), "TODO: implement limit for KV");
        }

        visit_mut::visit_stmt_query_mut(self, stmt);
    }

    fn visit_expr_in_subquery_mut(&mut self, i: &mut stmt::ExprInSubquery) {
        if !self.state.capability.sql {
            todo!("implement IN <subquery> expressions for KV");
        }

        visit_mut::visit_expr_in_subquery_mut(self, i);
    }
}

impl Decompose<'_, '_> {
    /// Returns the `StmtId` for the Statement at the **current** scope.
    fn scope_stmt_id(&self) -> StmtId {
        self.state.scopes[self.scope_id].stmt_id
    }

    /// Get the StmtId for the specified nesting level
    fn resolve_stmt_id(&self, nesting: usize) -> StmtId {
        self.state.scopes[self.scope_id - nesting].stmt_id
    }

    fn scope(&mut self, stmt_id: StmtId, f: impl FnOnce(&mut Decompose<'_, '_>)) {
        let scope_id = self.state.scopes.len();
        self.state.scopes.push(Scope { stmt_id });

        let mut child = Decompose {
            state: self.state,
            scope_id,
            // Always reset `returning` as we are entering a new statement.
            returning: false,
        };

        f(&mut child);

        self.state.scopes.pop();
    }
}

impl State<'_> {
    /// Create a new sub-statement. Returns the argument position
    fn new_sub_statement(
        &mut self,
        source_id: StmtId,
        target_id: StmtId,
        expr: stmt::Expr,
    ) -> usize {
        let source = &mut self.store[source_id];
        let arg = source.args.len();
        source.args.push(Arg::Sub {
            stmt_id: target_id,
            input: Cell::new(None),
        });

        let stmt::Expr::Stmt(expr_stmt) = expr else {
            panic!()
        };
        self.store[target_id].stmt = Some(expr_stmt.stmt);

        arg
    }

    /// Returns the ArgId for the new reference
    fn new_ref(
        &mut self,
        source_id: StmtId,
        target_id: StmtId,
        mut expr: stmt::ExprReference,
    ) -> usize {
        // First, get the nesting so we can resolve the target statmeent
        let nesting = expr.nesting();

        // We only track references that point to statements being executed by
        // separate materializations. References within the same materialization
        // are handled by the target database.
        debug_assert!(nesting != 0);

        // Set the nesting to zero as the stored ExprReference will be used from
        // the context of the *target* statement.
        let stmt::ExprReference::Column { nesting: n, .. } = &mut expr else {
            panic!()
        };
        *n = 0;

        let target = &mut self.store[target_id];

        // The `batch_load_index` is the index for this reference in the row
        // returned from the target statement's ExecStatement operation. This
        // ExecStatement operation batch loads all records needed to materialize
        // the full root statement.
        let (batch_load_index, _) = target
            .back_refs
            .entry(source_id)
            .or_default()
            .exprs
            .insert_full(expr);

        // Create an argument for inputing the expr reference's materialized
        // value into the statement.
        let source = &mut self.store[source_id];
        let arg = source.args.len();

        source.args.push(Arg::Ref {
            stmt_id: target_id,
            nesting,
            batch_load_index,
            input: Cell::new(None),
        });

        arg
    }
}
