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

struct LowerAssignment<'a, 'b> {
    cx: LoweringContext<'a, 'b>,

    /// Assignments being lowered
    assignments: &'a stmt::Assignments,
}

struct LowerReturning<'a, 'b> {
    cx: LoweringContext<'a, 'b>,
}

struct LowerStatement<'a, 'b> {
    /// The context in which the statement is being lowered.
    cx: LoweringContext<'a, 'b>,
}

trait LowerCommon<'a, 'b: 'a>: VisitMut {
    fn cx(&mut self) -> &mut LoweringContext<'a, 'b>;

    fn lower_expr_common(&mut self, expr: &mut stmt::Expr) {
        loop {
            // First recurse up the expression
            stmt::visit_mut::visit_expr_mut(self, expr);

            match self.cx().lower_expr_common(expr) {
                Lowering::Final(lowered) => {
                    *expr = lowered;
                    break;
                }
                Lowering::Partial(lowered) => {
                    *expr = lowered;
                }
                Lowering::None => break,
            }
        }
    }
}

impl<'a, 'b> LowerCommon<'a, 'b> for LowerStatement<'a, 'b> {
    fn cx(&mut self) -> &mut LoweringContext<'a, 'b> {
        &mut self.cx
    }
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

#[derive(Debug)]
enum Lowering {
    Final(stmt::Expr),
    Partial(stmt::Expr),
    None,
}

index_vec::define_index_type! {
    struct ScopeId = u32;
}

impl visit_mut::VisitMut for LowerAssignment<'_, '_> {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        // self.cx.lower_expr_common(i);
        todo!()
    }
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
                    let mapping = self.cx.mapping_unwrap();

                    let Some(field_mapping) = &mapping.fields[index] else {
                        todo!()
                    };

                    let column = field_mapping.column;
                    let mut lowered = mapping.model_to_table[field_mapping.lowering].clone();
                    self.cx.lower_assignment(i).visit_expr_mut(&mut lowered);
                    assignments.set(column, lowered);
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

    fn visit_expr_mut(&mut self, expr: &mut stmt::Expr) {
        loop {
            // First recurse up the expression
            stmt::visit_mut::visit_expr_mut(self, expr);

            match self.cx.lower_expr_common(expr) {
                Lowering::Final(lowered) => {
                    *expr = lowered;
                    break;
                }
                Lowering::Partial(lowered) => {
                    *expr = lowered;
                }
                Lowering::None => break,
            }
        }

        self.cx.register_expr_column_as_ref(expr);

        match expr {
            stmt::Expr::Reference(stmt::ExprReference::Field { nesting, index }) => {
                todo!()
            }
            _ => todo!(),
        }
    }

    fn visit_expr_stmt_mut(&mut self, i: &mut stmt::ExprStmt) {
        todo!("expr={i:#?}");
    }

    fn visit_returning_mut(&mut self, i: &mut stmt::Returning) {
        self.cx.lower_returning().visit_returning_mut(i);
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

impl VisitMut for LowerReturning<'_, '_> {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        self.cx.lower_expr_common(i);

        match i {
            stmt::Expr::Stmt(expr_stmt) => {
                // For now, we assume nested sub-statements cannot be executed on the
                // target database. Eventually, we will need to make this smarter.

                let source_id = self.cx.scope_stmt_id();
                let target_id = self.cx.new_statement_info();

                self.cx.scope_statement(target_id, |lower| {
                    visit_mut::visit_expr_stmt_mut(lower, expr_stmt);
                });

                let position = self.cx.new_sub_statement(source_id, target_id, i.take());
                *i = stmt::Expr::arg(position);
            }
            _ => {
                visit_mut::visit_expr_mut(self, i);
            }
        }
    }

    fn visit_stmt_query_mut(&mut self, i: &mut stmt::Query) {
        todo!()
    }

    fn visit_stmt_mut(&mut self, i: &mut stmt::Statement) {
        todo!()
    }
}

/*
fn lower_expr_common<'a>(lower: &mut impl LowerCommon<'a>, expr: &mut stmt::Expr) {
    loop {
        // First recurse up the expression
        stmt::visit_mut::visit_expr_mut(lower, expr);

        match lower.cx().lower_expr_common(expr) {
            Lowering::Final(lowered) => {
                *expr = lowered;
                break;
            }
            Lowering::Partial(lowered) => {
                *expr = lowered;
            }
            Lowering::None => break,
        }
    }
}
    */

impl<'b> LoweringContext<'_, 'b> {
    fn lower_expr_common(&mut self, expr: &mut stmt::Expr) -> Lowering {
        match expr {
            stmt::Expr::Reference(stmt::ExprReference::Field { nesting, index }) => {
                let mapping = self.mapping_at_unwrap(*nesting);
                Lowering::Partial(
                    mapping
                        .table_to_model
                        .lower_expr_reference(*nesting, *index),
                )
            }
            _ => Lowering::None,
        }
    }

    /// If the ExprColumn points to an ancesstor statement, register it with the
    /// current StatementInfo.
    fn register_expr_column_as_ref(&mut self, expr: &mut stmt::Expr) {
        if let stmt::Expr::Reference(expr_reference) = expr {
            debug_assert!(expr_reference.is_column());

            if expr_reference.nesting() > 0 {
                let source_id = self.scope_stmt_id();
                let target_id = self.resolve_stmt_id(expr_reference.nesting());

                let position = self.new_ref(source_id, target_id, *expr_reference);

                // Using ExprArg as a placeholder. It will be rewritten
                // later.
                *expr = stmt::Expr::arg(position);
            }
        }
    }

    /// Returns the ArgId for the new reference
    fn new_ref(
        &mut self,
        source_id: StmtId,
        target_id: StmtId,
        mut expr_reference: stmt::ExprReference,
    ) -> usize {
        let stmt::ExprReference::Column(expr_column) = &mut expr_reference else {
            todo!()
        };

        // First, get the nesting so we can resolve the target statmeent
        let nesting = expr_column.nesting;

        // We only track references that point to statements being executed by
        // separate materializations. References within the same materialization
        // are handled by the target database.
        debug_assert!(nesting != 0);

        // Set the nesting to zero as the stored ExprReference will be used from
        // the context of the *target* statement.
        expr_column.nesting = 0;

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
            .insert_full(expr_reference);

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

    #[track_caller]
    fn mapping_at_unwrap(&self, nesting: usize) -> &mapping::Model {
        let model = self.expr.target_at(nesting).as_model_unwrap();
        self.expr.schema().mapping_for(model)
    }

    /// Returns the `StmtId` for the Statement at the **current** scope.
    fn scope_stmt_id(&self) -> StmtId {
        self.state.scopes[self.scope_id].stmt_id
    }

    /// Get the StmtId for the specified nesting level
    fn resolve_stmt_id(&self, nesting: usize) -> StmtId {
        self.state.scopes[self.scope_id - nesting].stmt_id
    }

    fn scope_statement(&mut self, stmt_id: StmtId, f: impl FnOnce(&mut LowerStatement<'_, '_>)) {
        self.scope_id = self.state.scopes.push(Scope { stmt_id });
        f(&mut self.lower_statement());
        self.state.scopes.pop();
    }

    fn lower_assignment<'a>(
        &'a mut self,
        assignments: &'a stmt::Assignments,
    ) -> LowerAssignment<'a, 'b> {
        LowerAssignment {
            cx: self.borrow(),
            assignments,
        }
    }

    fn lower_returning(&mut self) -> LowerReturning<'_, 'b> {
        LowerReturning { cx: self.borrow() }
    }

    fn lower_statement(&mut self) -> LowerStatement<'_, 'b> {
        LowerStatement { cx: self.borrow() }
    }

    fn borrow(&mut self) -> LoweringContext<'_, 'b> {
        LoweringContext {
            state: self.state,
            expr: self.expr.clone(),
            scope_id: self.scope_id,
        }
    }
}
