mod insert;
mod paginate;
mod relation;
mod returning;

use std::{cell::Cell, collections::HashSet, mem};

use index_vec::IndexVec;
use toasty_core::{
    driver::Capability,
    schema::{
        app::{self, FieldTy, Model},
        db::{Column, ColumnId},
        mapping,
    },
    stmt::{self, visit_mut, IntoExprTarget, VisitMut},
    Result, Schema,
};

use crate::engine::{
    planner::ng::{info::StatementInfo, Arg, StatementInfoStore, StmtId},
    simplify::{self, Simplify},
    Engine,
};

impl super::PlannerNg<'_, '_> {
    pub(crate) fn lower_stmt(&mut self, stmt: stmt::Statement) -> Result<()> {
        let mut state = LoweringState {
            store: &mut self.store,
            scopes: IndexVec::new(),
            engine: self.old.engine,
            relations: vec![],
            errors: vec![],
            dependencies: HashSet::new(),
        };

        state.lower_stmt(stmt::ExprContext::new(self.old.schema()), stmt);

        if let Some(err) = state.errors.into_iter().next() {
            return Err(err);
        }

        Ok(())
    }
}

impl LoweringState<'_> {
    fn lower_stmt(&mut self, expr_cx: stmt::ExprContext, mut stmt: stmt::Statement) -> StmtId {
        // TODO: avoid simplifying many times
        self.engine.simplify_stmt(&mut stmt);

        let stmt_id = self.store.new_statement_info();
        let scope_id = self.scopes.push(Scope { stmt_id });
        let mut dependencies = self.dependencies.clone();

        // Map the statement
        LowerStatement {
            state: self,
            expr_cx,
            scope_id,
            cx: LoweringContext::Statement,
            dependencies: &mut dependencies,
        }
        .visit_stmt_mut(&mut stmt);

        // TODO: is there a way to avoid simplifying again?
        self.engine.simplify_stmt(&mut stmt);

        let stmt_info = &mut self.store[stmt_id];
        stmt_info.stmt = Some(Box::new(stmt));

        debug_assert!(stmt_info.deps.is_empty());
        stmt_info.deps = dependencies;

        stmt_id
    }
}

struct LowerStatement<'a, 'b> {
    /// Lowering state. This is the state that is constant throughout the entire
    /// lowering process.
    state: &'a mut LoweringState<'b>,

    /// Expression context in which the statement is being lowered
    expr_cx: stmt::ExprContext<'a>,

    /// Identifier to the current scope (stored in `scopes` on LoweringState)
    scope_id: ScopeId,

    /// Current lowering context
    cx: LoweringContext<'a>,

    /// Track dependencies here.
    dependencies: &'a mut HashSet<StmtId>,
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

    /// Planning a query can require walking relations to maintain data
    /// consistency. This field tracks the current relation edge being traversed
    /// so the planner doesn't walk it backwards.
    relations: Vec<app::FieldId>,

    /// All new statements should include these as part of its dependencies
    dependencies: HashSet<StmtId>,

    /// Tracks errors that occured while lowering the statement
    errors: Vec<crate::Error>,
}

#[derive(Debug, Clone, Copy)]
enum LoweringContext<'a> {
    /// Lowering update assignments
    Assignment(&'a stmt::Assignments),

    /// Lowering an insertion statement
    Insert(&'a [ColumnId]),

    /// Lowering a value row being inserted
    InsertRow(&'a stmt::Expr),

    /// Lowering the returning clause of a statement.
    Returning,

    /// All other lowering cases
    Statement,
}

#[derive(Debug)]
struct Scope {
    /// Identifier of the statement in the partitioner state.
    stmt_id: StmtId,
}

index_vec::define_index_type! {
    struct ScopeId = u32;
}

impl LowerStatement<'_, '_> {
    fn new_dependency(&mut self, stmt: impl Into<stmt::Statement>) -> StmtId {
        // Need to reset the scope stack as the statement cannot reference the
        // current scope.
        let scopes = mem::take(&mut self.state.scopes);

        let stmt_id = self
            .state
            .lower_stmt(stmt::ExprContext::new(self.schema()), stmt.into());

        self.state.scopes = scopes;
        self.dependencies.insert(stmt_id);

        stmt_id
    }

    fn collect_dependencies(
        &mut self,
        f: impl FnOnce(&mut LowerStatement<'_, '_>),
    ) -> HashSet<StmtId> {
        let old = std::mem::take(self.dependencies);

        f(self);

        std::mem::replace(self.dependencies, old)
    }

    fn with_dependencies(
        &mut self,
        dependencies: HashSet<StmtId>,
        f: impl FnOnce(&mut LowerStatement<'_, '_>),
    ) {
        debug_assert!(self.state.dependencies.is_empty());
        let old = std::mem::replace(&mut self.state.dependencies, dependencies);
        f(self);
        self.state.dependencies = old;
    }
}

impl visit_mut::VisitMut for LowerStatement<'_, '_> {
    fn visit_assignments_mut(&mut self, i: &mut stmt::Assignments) {
        let mut assignments = stmt::Assignments::default();

        for index in i.keys() {
            let field = &self.model_unwrap().fields[index];

            if field.primary_key {
                todo!("updating PK not supported yet");
            }

            match &field.ty {
                app::FieldTy::Primitive(_) => {
                    let mapping = self.mapping_unwrap();

                    let Some(field_mapping) = &mapping.fields[index] else {
                        todo!()
                    };

                    let column = field_mapping.column;
                    let mut lowered = mapping.model_to_table[field_mapping.lowering].clone();
                    self.lower_assignment(i).visit_expr_mut(&mut lowered);
                    assignments.set(column, lowered);
                }
                _ => panic!("relation fields should already have been handled; field={field:#?}"),
            }
        }

        *i = assignments;
    }

    fn visit_expr_set_op_mut(&mut self, i: &mut stmt::ExprSetOp) {
        todo!("stmt={i:#?}");
    }

    fn visit_expr_in_subquery_mut(&mut self, i: &mut stmt::ExprInSubquery) {
        if !self.capability().sql {
            todo!("implement IN <subquery> expressions for KV");
        }

        visit_mut::visit_expr_in_subquery_mut(self, i);
    }

    fn visit_expr_mut(&mut self, expr: &mut stmt::Expr) {
        match expr {
            stmt::Expr::BinaryOp(e) => {
                self.visit_expr_binary_op_mut(e);

                if let Some(lowered) = self.lower_expr_binary_op(e.op, &mut e.lhs, &mut e.rhs) {
                    *expr = lowered;
                }
            }
            stmt::Expr::InList(e) => {
                self.visit_expr_in_list_mut(e);

                if let Some(lowered) = self.lower_expr_in_list(&mut e.expr, &mut e.list) {
                    *expr = lowered;
                }
            }
            stmt::Expr::InSubquery(e) => {
                self.visit_expr_in_subquery_mut(e);

                let maybe_res = self.lower_expr_binary_op(
                    stmt::BinaryOp::Eq,
                    &mut e.expr,
                    e.query.returning_mut_unwrap().as_expr_mut_unwrap(),
                );

                assert!(maybe_res.is_none(), "TODO");

                let returning = e.query.returning_mut_unwrap().as_expr_mut_unwrap();

                if !returning.is_record() {
                    *returning = stmt::Expr::record([returning.take()]);
                }
            }
            stmt::Expr::Reference(expr_reference) => {
                match expr_reference {
                    stmt::ExprReference::Field { nesting, index } => {
                        *expr = self.lower_expr_field(*nesting, *index);
                        self.visit_expr_mut(expr);
                    }
                    stmt::ExprReference::Model { .. } => todo!(),
                    stmt::ExprReference::Column(expr_column) => {
                        if expr_column.nesting > 0 {
                            let source_id = self.scope_stmt_id();
                            let target_id = self.resolve_stmt_id(expr_column.nesting);

                            let position = self.new_ref(source_id, target_id, *expr_reference);

                            // Using ExprArg as a placeholder. It will be rewritten
                            // later.
                            *expr = stmt::Expr::arg(position);
                        }
                    }
                }
            }
            stmt::Expr::Stmt(expr_stmt) => {
                assert!(self.cx.is_returning(), "cx={:#?}", self.cx);

                // For now, we assume nested sub-statements cannot be executed on the
                // target database. Eventually, we will need to make this smarter.
                let source_id = self.scope_stmt_id();
                let target_id = self.scope_statement(|child| {
                    visit_mut::visit_expr_stmt_mut(child, expr_stmt);
                });

                let position = self.new_sub_statement(source_id, target_id, expr.take());
                *expr = stmt::Expr::arg(position);
            }
            _ => {
                // Recurse down the statement tree
                stmt::visit_mut::visit_expr_mut(self, expr);
            }
        }
    }

    fn visit_insert_target_mut(&mut self, i: &mut stmt::InsertTarget) {
        match i {
            stmt::InsertTarget::Scope(_) => todo!("stmt={i:#?}"),
            stmt::InsertTarget::Model(model_id) => {
                let mapping = self.schema().mapping_for(model_id);
                *i = stmt::InsertTable {
                    table: mapping.table,
                    columns: mapping.columns.clone(),
                }
                .into();
            }
            _ => todo!(),
        }
    }

    fn visit_update_target_mut(&mut self, i: &mut stmt::UpdateTarget) {
        match i {
            stmt::UpdateTarget::Query(_) => todo!("update_target={i:#?}"),
            stmt::UpdateTarget::Model(model_id) => {
                let table_id = self.schema().table_id_for(model_id);
                *i = stmt::UpdateTarget::table(table_id);
            }
            stmt::UpdateTarget::Table(_) => {}
        }
    }

    fn visit_expr_stmt_mut(&mut self, i: &mut stmt::ExprStmt) {
        todo!("expr={i:#?}");
    }

    fn visit_returning_mut(&mut self, i: &mut stmt::Returning) {
        if let stmt::Returning::Model { include } = i {
            // Capture the include clause as we will be using it to generate
            // inclusion statements.
            let include = std::mem::take(include);

            let mut returning = self.mapping_unwrap().table_to_model.lower_returning_model();

            for path in &include {
                self.build_include_subquery(&mut returning, path);
            }

            *i = stmt::Returning::Expr(returning);
        }

        stmt::visit_mut::visit_returning_mut(&mut self.lower_returning(), i);
    }

    fn visit_stmt_delete_mut(&mut self, stmt: &mut stmt::Delete) {
        assert!(stmt.returning.is_none(), "TODO; stmt={stmt:#?}");

        // Create a new expr scope for the statement, and lower all parts
        // *except* the source field (since it is borrowed).
        let mut lower = self.scope_expr(&stmt.from);

        // Before lowering, handle cascading deletes
        lower.plan_stmt_delete_relations(stmt);

        lower.visit_filter_mut(&mut stmt.filter);

        if let Some(returning) = &mut stmt.returning {
            lower.visit_returning_mut(returning);
        }

        lower.apply_lowering_filter_constraint(&mut stmt.filter);

        self.visit_source_mut(&mut stmt.from);
    }

    fn visit_stmt_insert_mut(&mut self, stmt: &mut stmt::Insert) {
        // First, if an insertion scope is specified, lower the scope to be just "model"
        self.apply_insert_scope(&mut stmt.target, &mut stmt.source);

        // Create a new expr scope for the statement, and lower all parts
        // *except* the target field (since it is borrowed).
        let mut lower = self.lower_insert(&stmt.target);

        // Preprocess the insertion source (values usually);
        lower.preprocess_insert_values(&mut stmt.source);

        // Lower the insertion source
        lower.visit_stmt_query_mut(&mut stmt.source);

        if let Some(returning) = &mut stmt.returning {
            lower.visit_returning_mut(returning);
            lower.constantize_insert_returning(returning, &stmt.source);
        }

        self.visit_insert_target_mut(&mut stmt.target);
    }

    fn visit_stmt_query_mut(&mut self, stmt: &mut stmt::Query) {
        if !self.capability().sql {
            assert!(stmt.order_by.is_none(), "TODO: implement ordering for KV");
            assert!(stmt.limit.is_none(), "TODO: implement limit for KV");
        }

        let mut lower = self.scope_expr(&stmt.body);

        if let Some(with) = &mut stmt.with {
            lower.visit_with_mut(with);
        }

        if let Some(order_by) = &mut stmt.order_by {
            lower.visit_order_by_mut(order_by);
        }

        if let Some(limit) = &mut stmt.limit {
            lower.visit_limit_mut(limit);
        }

        self.visit_expr_set_mut(&mut stmt.body);

        self.rewrite_offset_after_as_filter(stmt);
    }

    fn visit_stmt_select_mut(&mut self, stmt: &mut stmt::Select) {
        let mut lower = self.scope_expr(&stmt.source);

        lower.visit_filter_mut(&mut stmt.filter);
        lower.visit_returning_mut(&mut stmt.returning);
        lower.apply_lowering_filter_constraint(&mut stmt.filter);

        self.visit_source_mut(&mut stmt.source);
    }

    fn visit_stmt_update_mut(&mut self, stmt: &mut stmt::Update) {
        let mut lower = self.scope_expr(&stmt.target);

        // Plan relations
        lower.plan_stmt_update_relations(&mut stmt.assignments, &stmt.filter);

        // Before lowering children, convert the "Changed" returning statement
        // to an expression referencing changed fields.
        if let Some(returning) = &mut stmt.returning {
            if returning.is_changed() {
                if let Some(model) = lower.model() {
                    let mut fields = vec![];

                    for i in stmt.assignments.keys() {
                        let field = &model.fields[i];

                        assert!(field.ty.is_primitive(), "field={field:#?}");

                        fields.push(stmt::Expr::ref_self_field(app::FieldId {
                            model: model.id,
                            index: i,
                        }));
                    }

                    *returning = stmt::Returning::Expr(stmt::Expr::cast(
                        stmt::ExprRecord::from_vec(fields),
                        stmt::Type::SparseRecord(stmt.assignments.keys().collect()),
                    ));
                }
            }
        }

        lower.visit_assignments_mut(&mut stmt.assignments);
        lower.visit_filter_mut(&mut stmt.filter);

        if let Some(expr) = &mut stmt.condition.expr {
            lower.visit_expr_mut(expr);
        }

        if let Some(returning) = &mut stmt.returning {
            lower.visit_returning_mut(returning);
        }

        self.visit_update_target_mut(&mut stmt.target);
    }

    fn visit_source_mut(&mut self, stmt: &mut stmt::Source) {
        if let stmt::Source::Model(source_model) = stmt {
            debug_assert!(source_model.via.is_none(), "TODO");

            let table_id = self.schema().table_id_for(source_model.model);
            *stmt = stmt::Source::table(table_id);
        }
    }

    fn visit_values_mut(&mut self, stmt: &mut stmt::Values) {
        if self.cx.is_insert() {
            if let Some(mapping) = self.mapping() {
                for row in &mut stmt.rows {
                    let mut lowered = mapping.model_to_table.clone();
                    self.lower_insert_row(row)
                        .visit_expr_record_mut(&mut lowered);

                    *row = lowered.into();
                }

                return;
            }
        }

        visit_mut::visit_values_mut(self, stmt);
    }
}

impl<'a, 'b> LowerStatement<'a, 'b> {
    fn lower_expr_binary_op(
        &mut self,
        op: stmt::BinaryOp,
        lhs: &mut stmt::Expr,
        rhs: &mut stmt::Expr,
    ) -> Option<stmt::Expr> {
        match (&mut *lhs, &mut *rhs) {
            (stmt::Expr::Value(value), other) | (other, stmt::Expr::Value(value))
                if value.is_null() =>
            {
                let other = other.take();

                Some(match op {
                    stmt::BinaryOp::Eq => stmt::Expr::is_null(other),
                    stmt::BinaryOp::Ne => stmt::Expr::is_not_null(other),
                    _ => todo!(),
                })
            }
            (stmt::Expr::DecodeEnum(base, _, variant), other)
            | (other, stmt::Expr::DecodeEnum(base, _, variant)) => {
                assert!(op.is_eq());

                Some(stmt::Expr::eq(
                    (**base).clone(),
                    stmt::Expr::concat_str((
                        variant.to_string(),
                        "#",
                        stmt::Expr::cast(other.clone(), stmt::Type::String),
                    )),
                ))
            }
            (stmt::Expr::Cast(expr_cast), other) if expr_cast.ty.is_id() => {
                uncast_expr_id(lhs);
                uncast_expr_id(other);
                None
            }
            (stmt::Expr::Cast(_), stmt::Expr::Cast(_)) => todo!(),
            _ => None,
        }
    }

    fn lower_expr_in_list(
        &mut self,
        expr: &mut stmt::Expr,
        list: &mut stmt::Expr,
    ) -> Option<stmt::Expr> {
        match (&mut *expr, list) {
            (expr, stmt::Expr::Map(expr_map)) => {
                assert!(expr_map.base.is_arg(), "TODO");
                let maybe_res =
                    self.lower_expr_binary_op(stmt::BinaryOp::Eq, expr, &mut expr_map.map);

                assert!(maybe_res.is_none(), "TODO");
                None
            }
            (stmt::Expr::Cast(expr_cast), list) if expr_cast.ty.is_id() => {
                uncast_expr_id(expr);

                match list {
                    stmt::Expr::List(expr_list) => {
                        for expr in &mut expr_list.items {
                            uncast_expr_id(expr);
                        }
                    }
                    stmt::Expr::Value(stmt::Value::List(items)) => {
                        for item in items {
                            uncast_value_id(item);
                        }
                    }
                    stmt::Expr::Arg(_) => {
                        let arg = list.take();

                        // TODO: don't always cast to a string...
                        let cast = stmt::Expr::cast(stmt::Expr::arg(0), stmt::Type::String);
                        *list = stmt::Expr::map(arg, cast);
                    }
                    _ => todo!("expr={expr:#?}; list={list:#?}"),
                }

                None
            }
            (stmt::Expr::Record(lhs), stmt::Expr::List(list)) => {
                // TODO: implement for real
                for lhs in lhs {
                    assert!(lhs.is_column());
                }

                for item in &mut list.items {
                    assert!(item.is_value());
                }

                None
            }
            (stmt::Expr::Record(lhs), stmt::Expr::Value(stmt::Value::List(_))) => {
                // TODO: implement for real
                for lhs in lhs {
                    assert!(lhs.is_column());
                }

                None
            }
            (expr, list) => todo!("expr={expr:#?}; list={list:#?}"),
        }
    }

    fn apply_lowering_filter_constraint(&self, filter: &mut stmt::Filter) {
        let Some(model) = self.expr_cx.target().as_model() else {
            return;
        };

        let table = self.schema().table_for(model);
        let mapping = self.mapping_unwrap();

        // TODO: we really shouldn't have to simplify here, but until
        // simplification includes overlapping predicate pruning, we have to do
        // this here.
        if let Some(expr) = &mut filter.expr {
            simplify::simplify_expr(self.expr_cx, expr);
        }

        let mut operands = vec![];

        for column in table.primary_key_columns() {
            let pattern = match &mapping.model_to_table[column.id.index] {
                stmt::Expr::ConcatStr(expr) => {
                    // hax
                    let stmt::Expr::Value(stmt::Value::String(a)) = &expr.exprs[0] else {
                        todo!()
                    };
                    let stmt::Expr::Value(stmt::Value::String(b)) = &expr.exprs[1] else {
                        todo!()
                    };

                    format!("{a}{b}")
                }
                stmt::Expr::Value(_) => todo!(),
                _ => continue,
            };

            if let Some(filter) = &filter.expr {
                if self.is_eq_constrained(filter, column) {
                    continue;
                }
            }

            assert_eq!(self.mapping_unwrap().columns[column.id.index], column.id);

            operands.push(stmt::Expr::begins_with(
                self.expr_cx.expr_ref_column(column),
                pattern,
            ));
        }

        if operands.is_empty() {
            return;
        }

        filter.add_filter(stmt::Expr::and_from_vec(operands));
    }

    fn is_eq_constrained(&self, expr: &stmt::Expr, column: &Column) -> bool {
        use stmt::Expr::*;

        match expr {
            And(expr) => expr.iter().any(|expr| self.is_eq_constrained(expr, column)),
            Or(expr) => expr.iter().all(|expr| self.is_eq_constrained(expr, column)),
            BinaryOp(expr) => {
                if !expr.op.is_eq() {
                    return false;
                }

                match (&*expr.lhs, &*expr.rhs) {
                    (Reference(lhs), _) => {
                        self.expr_cx.resolve_expr_reference(lhs).expect_column().id == column.id
                    }
                    (_, Reference(rhs)) => {
                        self.expr_cx.resolve_expr_reference(rhs).expect_column().id == column.id
                    }
                    _ => false,
                }
            }
            InList(expr) => match &*expr.expr {
                Reference(lhs) => {
                    self.expr_cx.resolve_expr_reference(lhs).expect_column().id == column.id
                }
                _ => todo!("expr={:#?}", expr),
            },
            _ => todo!("expr={:#?}", expr),
        }
    }

    fn lower_expr_field(&self, nesting: usize, index: usize) -> stmt::Expr {
        match self.cx {
            LoweringContext::Statement | LoweringContext::Returning => {
                let mapping = self.mapping_at_unwrap(nesting);
                mapping.table_to_model.lower_expr_reference(nesting, index)
            }
            LoweringContext::Assignment(assignments) => {
                assert_eq!(nesting, 0, "TODO");
                let assignment = &assignments[index];
                assert!(assignment.op.is_set(), "TODO");
                assignment.expr.clone()
            }
            LoweringContext::InsertRow(row) => row.entry(index).to_expr(),
            _ => todo!("cx={:#?}", self.cx),
        }
    }

    fn build_include_subquery(&mut self, returning: &mut stmt::Expr, path: &stmt::Path) {
        let [field_index] = &path.projection[..] else {
            todo!("Multi-step include paths not yet supported")
        };

        let field = &self.model_unwrap().fields[*field_index];

        let mut stmt = match &field.ty {
            FieldTy::HasMany(rel) => stmt::Query::new_select(
                rel.target,
                stmt::Expr::eq(
                    stmt::Expr::ref_parent_model(),
                    stmt::Expr::ref_self_field(rel.pair),
                ),
            ),
            // To handle single relations, we need a new query modifier that
            // returns a single record and not a list. This matters for the type
            // system.
            FieldTy::BelongsTo(rel) => {
                let source_fk;
                let target_pk;

                if let [fk_field] = &rel.foreign_key.fields[..] {
                    source_fk = stmt::Expr::ref_parent_field(fk_field.source);
                    target_pk = stmt::Expr::ref_self_field(fk_field.target);
                } else {
                    let mut source_fk_fields = vec![];
                    let mut target_pk_fields = vec![];

                    for fk_field in &rel.foreign_key.fields {
                        source_fk_fields.push(stmt::Expr::ref_parent_field(fk_field.source));
                        target_pk_fields.push(stmt::Expr::ref_parent_field(fk_field.source));
                    }

                    source_fk = stmt::Expr::record_from_vec(source_fk_fields);
                    target_pk = stmt::Expr::record_from_vec(target_pk_fields);
                }

                let mut query =
                    stmt::Query::new_select(rel.target, stmt::Expr::eq(source_fk, target_pk));
                query.single = true;
                query
            }
            FieldTy::HasOne(rel) => {
                let mut query = stmt::Query::new_select(
                    rel.target,
                    stmt::Expr::eq(
                        stmt::Expr::ref_parent_model(),
                        stmt::Expr::ref_self_field(rel.pair),
                    ),
                );
                query.single = true;
                query
            }
            _ => todo!(),
        };

        // Simplify the new stmt to handle relations.
        Simplify::with_context(self.expr_cx).visit_stmt_query_mut(&mut stmt);

        returning
            .entry_mut(*field_index)
            .insert(stmt::Expr::stmt(stmt));
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

        let stmt::Expr::Stmt(mut expr_stmt) = expr else {
            panic!()
        };

        // TODO: Is ther ea way to avoid simplifying here?
        self.state.engine.simplify_stmt(&mut *expr_stmt.stmt);

        self.state.store[target_id].stmt = Some(expr_stmt.stmt);

        arg
    }

    fn schema(&self) -> &'b Schema {
        &self.state.engine.schema
    }

    fn capability(&self) -> &Capability {
        self.state.engine.capability()
    }

    fn field(&self, id: impl Into<app::FieldId>) -> &'b app::Field {
        self.schema().app.field(id.into())
    }

    fn model(&self) -> Option<&Model> {
        self.expr_cx.target().as_model()
    }

    #[track_caller]
    fn model_unwrap(&self) -> &Model {
        self.expr_cx.target().as_model_unwrap()
    }

    fn mapping(&self) -> Option<&'b mapping::Model> {
        self.model()
            .map(|model| self.state.engine.schema.mapping_for(model))
    }

    #[track_caller]
    fn mapping_unwrap(&self) -> &'b mapping::Model {
        self.state.engine.schema.mapping_for(self.model_unwrap())
    }

    #[track_caller]
    fn mapping_at_unwrap(&self, nesting: usize) -> &'b mapping::Model {
        let model = self.expr_cx.target_at(nesting).as_model_unwrap();
        self.state.engine.schema.mapping_for(model)
    }

    fn curr_stmt_info(&mut self) -> &mut StatementInfo {
        let stmt_id = self.scope_stmt_id();
        &mut self.state.store[stmt_id]
    }

    /// Returns the `StmtId` for the Statement at the **current** scope.
    fn scope_stmt_id(&self) -> StmtId {
        self.state.scopes[self.scope_id].stmt_id
    }

    /// Get the StmtId for the specified nesting level
    fn resolve_stmt_id(&self, nesting: usize) -> StmtId {
        self.state.scopes[self.scope_id - nesting].stmt_id
    }

    /// Plan a sub-statement that is able to reference the parent statement
    fn scope_statement(&mut self, f: impl FnOnce(&mut LowerStatement<'_, '_>)) -> StmtId {
        let stmt_id = self.new_statement_info();
        let scope_id = self.state.scopes.push(Scope { stmt_id });
        let mut dependencies = self.state.dependencies.clone();

        let mut lower = LowerStatement {
            state: self.state,
            expr_cx: self.expr_cx,
            scope_id,
            cx: LoweringContext::Statement,
            dependencies: &mut dependencies,
        };

        f(&mut lower);

        self.state.store[stmt_id].deps = dependencies;
        self.state.scopes.pop();
        stmt_id
    }

    fn scope_expr<'child>(
        &'child mut self,
        target: impl IntoExprTarget<'child>,
    ) -> LowerStatement<'child, 'b> {
        LowerStatement {
            state: self.state,
            expr_cx: self.expr_cx.scope(target),
            scope_id: self.scope_id,
            cx: self.cx,
            dependencies: self.dependencies,
        }
    }

    fn lower_assignment<'child>(
        &'child mut self,
        assignments: &'child stmt::Assignments,
    ) -> LowerStatement<'child, 'b> {
        LowerStatement {
            state: self.state,
            expr_cx: self.expr_cx,
            scope_id: self.scope_id,
            cx: LoweringContext::Assignment(assignments),
            dependencies: self.dependencies,
        }
    }

    fn lower_insert<'child>(
        &'child mut self,
        target: &'child stmt::InsertTarget,
    ) -> LowerStatement<'child, 'b> {
        let columns = match target {
            stmt::InsertTarget::Scope(_) => {
                panic!("InsertTarget::Scope should already have been lowered by this point")
            }
            stmt::InsertTarget::Model(model_id) => &self.schema().mapping_for(model_id).columns,
            stmt::InsertTarget::Table(insert_table) => &insert_table.columns,
        };

        LowerStatement {
            state: self.state,
            expr_cx: self.expr_cx.scope(target),
            scope_id: self.scope_id,
            cx: LoweringContext::Insert(columns),
            dependencies: self.dependencies,
        }
    }

    fn lower_insert_row<'child>(
        &'child mut self,
        row: &'child stmt::Expr,
    ) -> LowerStatement<'child, 'b> {
        LowerStatement {
            state: self.state,
            expr_cx: self.expr_cx,
            scope_id: self.scope_id,
            cx: LoweringContext::InsertRow(row),
            dependencies: self.dependencies,
        }
    }

    fn lower_returning(&mut self) -> LowerStatement<'_, 'b> {
        LowerStatement {
            state: self.state,
            expr_cx: self.expr_cx,
            scope_id: self.scope_id,
            cx: LoweringContext::Returning,
            dependencies: self.dependencies,
        }
    }
}

impl LoweringContext<'_> {
    fn is_insert(&self) -> bool {
        matches!(self, LoweringContext::Insert { .. })
    }

    fn is_returning(&self) -> bool {
        matches!(self, LoweringContext::Returning)
    }
}

fn uncast_expr_id(expr: &mut stmt::Expr) {
    match expr {
        stmt::Expr::Value(value) => {
            uncast_value_id(value);
        }
        stmt::Expr::Cast(expr_cast) if expr_cast.ty.is_id() => {
            *expr = expr_cast.expr.take();
        }
        stmt::Expr::Project(_) => {
            // TODO: don't always cast to a string...
            let base = expr.take();
            *expr = stmt::Expr::cast(base, stmt::Type::String);
        }
        stmt::Expr::List(expr_list) => {
            for expr in &mut expr_list.items {
                uncast_expr_id(expr);
            }
        }
        _ => todo!("{expr:#?}"),
    }
}

fn uncast_value_id(value: &mut stmt::Value) {
    match value {
        stmt::Value::Id(_) => {
            let uncast = value.take().into_id().into_primitive();
            *value = uncast;
        }
        stmt::Value::List(items) => {
            for item in items {
                uncast_value_id(item);
            }
        }
        _ => todo!("{value:#?}"),
    }
}
