use std::cell::Cell;

use index_vec::IndexVec;
use toasty_core::{
    driver::Capability,
    schema::{
        app::{self, Model},
        mapping,
    },
    stmt::{self, visit_mut, VisitMut},
};

use crate::engine::{
    planner::ng::{Arg, StatementInfoStore, StmtId},
    Engine,
};

impl super::PlannerNg<'_, '_> {
    pub(crate) fn lower_stmt(&mut self, mut stmt: stmt::Statement) {
        let root_id = self.store.root_id();

        let mut state = LoweringState {
            store: &mut self.store,
            scopes: IndexVec::new(),
            engine: self.old.engine,
        };

        let scope_id = state.scopes.push(Scope { stmt_id: root_id });

        // Map the statement
        LowerStatement {
            cx: LoweringContext {
                state: &mut state,
                expr: stmt::ExprContext::new(self.old.schema()),
                scope_id,
            },
        }
        .visit_stmt_mut(&mut stmt);

        self.store.root_mut().stmt = Some(Box::new(stmt));
    }
}

struct LowerStatement<'a, 'b> {
    /// The context in which the statement is being lowered.
    cx: LoweringContext<'a, 'b>,
}

struct LoweringReturning<'a, 'b> {
    cx: LoweringContext<'a, 'b>,
}

#[derive(Debug)]
struct LoweringState<'a> {
    /// Database engine handle
    engine: &'a Engine,

    /// Statements to be executed by the database, though they may still be
    /// broken down into multiple sub-statements.
    store: &'a mut StatementInfoStore,

    /// Scope state
    scopes: IndexVec<ScopeId, Scope>,
}

#[derive(Debug)]
struct LoweringContext<'a, 'b> {
    /// Lowering state. This is the state that is constant throughout the entire
    /// lowering process.
    state: &'a mut LoweringState<'b>,

    /// Expression context in which the statement is being lowered
    expr: stmt::ExprContext<'a>,

    /// Identifier to the current scope (stored in `scopes` on LoweringState)
    scope_id: ScopeId,
}

#[derive(Debug)]
struct Scope {
    /// Identifier of the statement in the partitioner state.
    stmt_id: StmtId,
}

index_vec::define_index_type! {
    struct ScopeId = u32;
}

impl visit_mut::VisitMut for LowerStatement<'_, '_> {
    fn visit_assignments_mut(&mut self, i: &mut stmt::Assignments) {
        let mut assignments = stmt::Assignments::default();

        for index in i.keys() {
            let field = &self.cx.model_unwrap().fields[index];

            if field.primary_key {
                todo!("updating PK not supported yet");
            }

            match &field.ty {
                app::FieldTy::Primitive(_) => {
                    let Some(field_mapping) = &self.cx.mapping_unwrap().fields[index] else {
                        todo!()
                    };

                    /*
                    let mut lowered = self.mapping().model_to_table[field_mapping.lowering].clone();
                    Substitute::new(self.model(), &*i).visit_expr_mut(&mut lowered);
                    assignments.set(field_mapping.column, lowered);
                    */
                    todo!()
                }
                _ => {
                    todo!("field = {:#?};", field);
                }
            }
        }

        *i = assignments;
    }

    fn visit_expr_set_op_mut(&mut self, i: &mut stmt::ExprSetOp) {
        todo!("stmt={i:#?}");
    }

    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        self.cx.lower_expr_common(i);

        match i {
            stmt::Expr::Stmt(expr_stmt) => {
                // For now, we assume nested sub-statements cannot be executed on the
                // target database. Eventually, we will need to make this smarter.

                let source_id = self.cx.scope_stmt_id();
                let target_id = self.cx.new_statement_info();

                self.scope(target_id, |child| {
                    visit_mut::visit_expr_stmt_mut(child, expr_stmt);
                });

                let position = self.cx.new_sub_statement(source_id, target_id, i.take());
                *i = stmt::Expr::arg(position);
            }
            _ => {
                visit_mut::visit_expr_mut(self, i);
            }
        }
    }

    fn visit_returning_mut(&mut self, i: &mut stmt::Returning) {
        LoweringReturning {
            cx: LoweringContext {
                state: self.cx.state,
                expr: self.cx.expr.clone(),
                scope_id: self.cx.scope_id,
            },
        }
        .visit_returning_mut(i);
    }

    fn visit_stmt_query_mut(&mut self, stmt: &mut stmt::Query) {
        if !self.cx.capability().sql {
            assert!(stmt.order_by.is_none(), "TODO: implement ordering for KV");
            assert!(stmt.limit.is_none(), "TODO: implement limit for KV");
        }

        visit_mut::visit_stmt_query_mut(self, stmt);
    }

    fn visit_expr_in_subquery_mut(&mut self, i: &mut stmt::ExprInSubquery) {
        if !self.cx.capability().sql {
            todo!("implement IN <subquery> expressions for KV");
        }

        visit_mut::visit_expr_in_subquery_mut(self, i);
    }
}

impl LowerStatement<'_, '_> {
    fn scope(&mut self, stmt_id: StmtId, f: impl FnOnce(&mut LowerStatement<'_, '_>)) {
        let scope_id = self.cx.state.scopes.push(Scope { stmt_id });

        let mut child = LowerStatement {
            cx: LoweringContext {
                state: self.cx.state,
                expr: self.cx.expr.clone(),
                scope_id: scope_id,
            },
        };

        f(&mut child);

        self.cx.state.scopes.pop();
    }
}

impl VisitMut for LoweringReturning<'_, '_> {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        self.cx.lower_expr_common(i);
        visit_mut::visit_expr_mut(self, i);
    }

    fn visit_stmt_query_mut(&mut self, i: &mut stmt::Query) {
        todo!()
    }

    fn visit_stmt_mut(&mut self, i: &mut stmt::Statement) {
        todo!()
    }
}

impl LoweringState<'_> {
    fn capability(&self) -> &Capability {
        self.engine.capability()
    }
}

impl LoweringContext<'_, '_> {
    /// Shared expr lowering behavior
    fn lower_expr_common(&mut self, expr: &mut stmt::Expr) {
        match expr {
            stmt::Expr::Reference(expr_reference) => {
                // At this point, the query should have been fully lowered
                let stmt::ExprReference::Column { nesting, .. } = expr_reference else {
                    panic!("unexpected state: statement not lowered")
                };

                if *nesting > 0 {
                    let source_id = self.scope_stmt_id();
                    let target_id = self.resolve_stmt_id(*nesting);

                    let position = self.new_ref(source_id, target_id, *expr_reference);

                    // Using ExprArg as a placeholder. It will be rewritten
                    // later.
                    *expr = stmt::Expr::arg(position);
                }
            }
            _ => {}
        }
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

        let target = &mut self.state.store[target_id];

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
        let source = &mut self.state.store[source_id];
        let arg = source.args.len();

        source.args.push(Arg::Ref {
            stmt_id: target_id,
            nesting,
            batch_load_index,
            input: Cell::new(None),
        });

        arg
    }

    fn new_statement_info(&mut self) -> StmtId {
        self.state.store.new_statement_info()
    }

    /// Create a new sub-statement. Returns the argument position
    fn new_sub_statement(
        &mut self,
        source_id: StmtId,
        target_id: StmtId,
        expr: stmt::Expr,
    ) -> usize {
        let source = &mut self.state.store[source_id];
        let arg = source.args.len();
        source.args.push(Arg::Sub {
            stmt_id: target_id,
            input: Cell::new(None),
        });

        let stmt::Expr::Stmt(expr_stmt) = expr else {
            panic!()
        };
        self.state.store[target_id].stmt = Some(expr_stmt.stmt);

        arg
    }

    fn capability(&self) -> &Capability {
        self.state.engine.capability()
    }

    #[track_caller]
    fn model_unwrap(&self) -> &Model {
        self.expr.target().as_model_unwrap()
    }

    #[track_caller]
    fn mapping_unwrap(&self) -> &mapping::Model {
        self.expr.schema().mapping_for(self.model_unwrap())
    }

    /// Returns the `StmtId` for the Statement at the **current** scope.
    fn scope_stmt_id(&self) -> StmtId {
        self.state.scopes[self.scope_id].stmt_id
    }

    /// Get the StmtId for the specified nesting level
    fn resolve_stmt_id(&self, nesting: usize) -> StmtId {
        self.state.scopes[self.scope_id - nesting].stmt_id
    }
}
