mod paginate;

use crate::engine::{simplify::Simplify, Engine};

use super::{simplify, Planner};
use toasty_core::{
    driver::Capability,
    schema::{
        app::{self, FieldId, FieldTy, Model},
        db::{Column, Table},
        mapping,
    },
    stmt::{self, VisitMut},
    Schema,
};

struct LowerStatement<'a, 'b> {
    /// The context in which the statement is being lowered.
    ///
    /// This will always be `Model`
    cx: stmt::ExprContext<'a>,

    state: &'a mut State<'b>,
}

struct State<'a> {
    /// The target database's capabilities
    engine: &'a Engine,

    /// Lowering a query can require walking relations to maintain data
    /// consistency. This field tracks the current relation edge being traversed
    /// so the planner doesn't walk it backwards.
    relations: Vec<app::FieldId>,
}

/// Substitute fields for columns
struct Substitute<'a, I> {
    target: &'a app::Model,
    input: I,
}

trait Input {
    fn resolve_field(&mut self, field_id: FieldId) -> stmt::Expr;
}

impl Planner<'_> {
    pub(crate) fn lower_stmt(&self, stmt: &mut stmt::Statement) {
        let mut state = State::new(self.engine);
        LowerStatement::new(self.engine, &mut state).visit_stmt_mut(stmt);
        simplify::simplify_stmt(&self.engine.schema, stmt);
    }

    pub(crate) fn lower_stmt_delete(&self, stmt: &mut stmt::Delete) {
        let mut state = State::new(self.engine);
        LowerStatement::new(self.engine, &mut state).visit_stmt_delete_mut(stmt);
        simplify::simplify_stmt(&self.engine.schema, stmt);
    }

    pub(crate) fn lower_stmt_insert(&self, stmt: &mut stmt::Insert) {
        let mut state = State::new(self.engine);
        LowerStatement::new(self.engine, &mut state).visit_stmt_insert_mut(stmt);
        simplify::simplify_stmt(&self.engine.schema, stmt);
    }

    pub(crate) fn lower_stmt_update(&self, stmt: &mut stmt::Update) {
        let mut state = State::new(self.engine);
        LowerStatement::new(self.engine, &mut state).visit_stmt_update_mut(stmt);
        simplify::simplify_stmt(&self.engine.schema, stmt);
    }
}

impl<'a> State<'a> {
    fn new(engine: &'a Engine) -> Self {
        State {
            engine,
            relations: vec![],
        }
    }
}

impl<'a, 'b> LowerStatement<'a, 'b> {
    fn new(engine: &'a Engine, state: &'a mut State<'b>) -> Self {
        LowerStatement {
            cx: stmt::ExprContext::new(&*engine.schema),
            state,
        }
    }

    fn capability(&self) -> &'a Capability {
        self.state.engine.capability()
    }

    fn schema(&self) -> &'a Schema {
        &self.state.engine.schema
    }
}

impl VisitMut for LowerStatement<'_, '_> {
    fn visit_assignments_mut(&mut self, i: &mut stmt::Assignments) {
        let mut assignments = stmt::Assignments::default();

        for index in i.keys() {
            let field = &self.model().fields[index];

            if field.primary_key {
                todo!("updating PK not supported yet");
            }

            match &field.ty {
                app::FieldTy::Primitive(_) => {
                    let Some(field_mapping) = &self.mapping().fields[index] else {
                        todo!()
                    };

                    let mut lowered = self.mapping().model_to_table[field_mapping.lowering].clone();
                    Substitute::new(self.model(), &*i).visit_expr_mut(&mut lowered);
                    assignments.set(field_mapping.column, lowered);
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
        stmt::visit_mut::visit_expr_mut(self, i);

        let maybe_expr = match i {
            stmt::Expr::BinaryOp(expr) => {
                self.lower_expr_binary_op(expr.op, &mut expr.lhs, &mut expr.rhs)
            }
            stmt::Expr::Reference(stmt::ExprReference::Field { nesting, index }) => {
                let model = self.cx.target_at(*nesting).as_model_unwrap();
                let mapping = self.mapping_for(model);

                *i = mapping
                    .table_to_model
                    .lower_expr_reference(*nesting, *index);
                self.visit_expr_mut(i);

                return;
            }
            stmt::Expr::InList(expr) => self.lower_expr_in_list(&mut expr.expr, &mut expr.list),
            stmt::Expr::InSubquery(expr) => {
                let maybe_res = self.lower_expr_binary_op(
                    stmt::BinaryOp::Eq,
                    &mut expr.expr,
                    expr.query
                        .body
                        .as_select_mut_unwrap()
                        .returning
                        .as_expr_mut_unwrap(),
                );

                assert!(maybe_res.is_none(), "TODO");

                return;
            }
            _ => {
                return;
            }
        };

        if let Some(expr) = maybe_expr {
            *i = expr;
        }
    }

    fn visit_insert_target_mut(&mut self, i: &mut stmt::InsertTarget) {
        *i = stmt::InsertTable {
            table: self.mapping().table,
            columns: self.mapping().columns.clone(),
        }
        .into();
    }

    fn visit_returning_mut(&mut self, i: &mut stmt::Returning) {
        if let stmt::Returning::Model { include } = i {
            // Capture the include clause as we will be using it to generate
            // inclusion statements.
            let include = std::mem::take(include);

            let mut returning = self.mapping().table_to_model.lower_returning_model();

            for path in &include {
                self.build_include_subquery(&mut returning, path);
            }

            *i = stmt::Returning::Expr(returning);
        }

        stmt::visit_mut::visit_returning_mut(self, i);
    }

    fn visit_stmt_delete_mut(&mut self, i: &mut stmt::Delete) {
        let stmt::Source::Model(source) = &i.from else {
            panic!("unexpected state")
        };

        let mut lower = self.scope(self.cx.schema().app.model(source.model));

        stmt::visit_mut::visit_stmt_delete_mut(&mut lower, i);

        assert!(i.returning.is_none(), "TODO; stmt={i:#?}");

        lower.apply_lowering_filter_constraint(&mut i.filter);
    }

    fn visit_stmt_insert_mut(&mut self, i: &mut stmt::Insert) {
        let model_id = i.target.as_model();

        let mut lower = self.scope(self.cx.schema().app.model(model_id));

        match &mut i.source.body {
            stmt::ExprSet::Values(values) => {
                for row in &mut values.rows {
                    lower.lower_insert_values(row);
                }
            }
            _ => todo!("stmt={i:#?}"),
        }

        stmt::visit_mut::visit_stmt_insert_mut(&mut lower, i);
    }

    fn visit_stmt_select_mut(&mut self, i: &mut stmt::Select) {
        stmt::visit_mut::visit_stmt_select_mut(self, i);
        self.apply_lowering_filter_constraint(&mut i.filter);
    }

    fn visit_stmt_query_mut(&mut self, i: &mut stmt::Query) {
        let model_id = match &i.body {
            stmt::ExprSet::Select(select) => {
                let stmt::Source::Model(source) = &select.source else {
                    panic!("unexpected state; {i:#?}")
                };

                source.model
            }
            stmt::ExprSet::Update(update) => {
                let stmt::UpdateTarget::Model(model) = &update.target else {
                    panic!("unexpected state")
                };

                *model
            }
            stmt::ExprSet::Values(_) => {
                // Values is a free context
                let mut lower = LowerStatement {
                    cx: self.cx.scope(stmt::ExprTarget::Free),
                    state: self.state,
                };
                stmt::visit_mut::visit_stmt_query_mut(&mut lower, i);
                return;
            }
            _ => todo!("unexpected query: {i:#?}"),
        };

        let mut lower = self.scope(self.cx.schema().app.model(model_id));
        stmt::visit_mut::visit_stmt_query_mut(&mut lower, i);

        self.rewrite_offset_after_as_filter(i);
    }

    fn visit_stmt_update_mut(&mut self, i: &mut stmt::Update) {
        let model_id = i.target.model_id_unwrap();
        let mut lower = self.scope(self.cx.schema().app.model(model_id));

        // Before lowering children, convert the "Changed" returning statement
        // to an expression referencing changed fields.

        if let Some(returning) = &mut i.returning {
            if returning.is_changed() {
                let mut fields = vec![];

                for i in i.assignments.keys() {
                    let field = &lower.model().fields[i];

                    assert!(field.ty.is_primitive(), "field={field:#?}");

                    fields.push(stmt::Expr::ref_self_field(app::FieldId {
                        model: lower.model().id,
                        index: i,
                    }));
                }

                *returning = stmt::Returning::Expr(stmt::Expr::cast(
                    stmt::ExprRecord::from_vec(fields),
                    stmt::Type::SparseRecord(i.assignments.keys().collect()),
                ));
            }
        }

        stmt::visit_mut::visit_stmt_update_mut(&mut lower, i);
    }

    fn visit_source_mut(&mut self, i: &mut stmt::Source) {
        debug_assert!(i.is_model());
        *i = stmt::Source::table(self.table().id);
    }

    fn visit_update_target_mut(&mut self, i: &mut stmt::UpdateTarget) {
        *i = stmt::UpdateTarget::table(self.table().id);
    }
}

impl<'a, 'b> LowerStatement<'a, 'b> {
    fn model(&self) -> &'a Model {
        self.cx.target_as_model().expect("expected model")
    }

    fn table(&self) -> &'a Table {
        self.cx.schema().table_for(self.model())
    }

    fn mapping(&self) -> &mapping::Model {
        self.cx.schema().mapping_for(self.model())
    }

    fn mapping_for(&self, model: &Model) -> &mapping::Model {
        self.cx.schema().mapping_for(model)
    }

    fn scope<'child>(&'child mut self, target: &'a Model) -> LowerStatement<'child, 'b> {
        LowerStatement {
            cx: self.cx.scope(target),
            state: self.state,
        }
    }

    fn apply_lowering_filter_constraint(&self, filter: &mut stmt::Filter) {
        // TODO: we really shouldn't have to simplify here, but until
        // simplification includes overlapping predicate pruning, we have to do
        // this here.
        if let Some(expr) = &mut filter.expr {
            simplify::simplify_expr(self.cx, expr);
        }

        let mut operands = vec![];

        for column in self.table().primary_key_columns() {
            let pattern = match &self.mapping().model_to_table[column.id.index] {
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

            assert_eq!(self.mapping().columns[column.id.index], column.id);

            operands.push(stmt::Expr::begins_with(
                self.cx.expr_ref_column(column),
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
                        self.cx.resolve_expr_reference(lhs).expect_column().id == column.id
                    }
                    (_, Reference(rhs)) => {
                        self.cx.resolve_expr_reference(rhs).expect_column().id == column.id
                    }
                    _ => false,
                }
            }
            InList(expr) => match &*expr.expr {
                Reference(lhs) => {
                    self.cx.resolve_expr_reference(lhs).expect_column().id == column.id
                }
                _ => todo!("expr={:#?}", expr),
            },
            _ => todo!("expr={:#?}", expr),
        }
    }

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
                Self::uncast_expr_id(lhs);
                Self::uncast_expr_id(other);
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
                Self::uncast_expr_id(expr);

                match list {
                    stmt::Expr::List(expr_list) => {
                        for expr in &mut expr_list.items {
                            Self::uncast_expr_id(expr);
                        }
                    }
                    stmt::Expr::Value(stmt::Value::List(items)) => {
                        for item in items {
                            Self::uncast_value_id(item);
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

    fn lower_insert_values(&self, expr: &mut stmt::Expr) {
        let mut lowered = self.mapping().model_to_table.clone();
        Substitute::new(self.model(), &mut *expr).visit_expr_record_mut(&mut lowered);
        *expr = lowered.into();
    }

    fn uncast_expr_id(expr: &mut stmt::Expr) {
        match expr {
            stmt::Expr::Value(value) => {
                Self::uncast_value_id(value);
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
                    Self::uncast_expr_id(expr);
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
                    Self::uncast_value_id(item);
                }
            }
            _ => todo!("{value:#?}"),
        }
    }

    fn build_include_subquery(&mut self, returning: &mut stmt::Expr, path: &stmt::Path) {
        let [field_index] = &path.projection[..] else {
            todo!("Multi-step include paths not yet supported")
        };

        let field = &self.model().fields[*field_index];

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
        Simplify::with_context(self.cx).visit_stmt_query_mut(&mut stmt);

        returning
            .entry_mut(*field_index)
            .insert(stmt::Expr::stmt(stmt));
    }
}

impl<'a, I> Substitute<'a, I> {
    fn new(target: &'a app::Model, input: I) -> Self {
        Substitute { target, input }
    }
}

impl<'a, I: Input> VisitMut for Substitute<'a, I> {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        match i {
            stmt::Expr::Reference(stmt::ExprReference::Field { nesting, index }) => {
                assert!(*nesting == 0, "TODO: support references to parent scopes");

                let field_id = FieldId {
                    model: self.target.id,
                    index: *index,
                };
                *i = self.input.resolve_field(field_id);
            }
            // Do not traverse these
            stmt::Expr::InSubquery(_) | stmt::Expr::Stmt(_) => {}
            _ => {
                // Traverse other expressions
                stmt::visit_mut::visit_expr_mut(self, i);
            }
        }
    }

    fn visit_expr_in_subquery_mut(&mut self, _i: &mut stmt::ExprInSubquery) {
        todo!()
    }
    fn visit_expr_stmt_mut(&mut self, _i: &mut stmt::ExprStmt) {
        todo!()
    }
}

impl Input for &mut stmt::Expr {
    fn resolve_field(&mut self, field_id: FieldId) -> stmt::Expr {
        self.entry(field_id.index).to_expr()
    }
}

impl Input for &stmt::Assignments {
    fn resolve_field(&mut self, field_id: FieldId) -> stmt::Expr {
        let assignment = &self[field_id.index];
        assert!(assignment.op.is_set());
        assignment.expr.clone()
    }
}
