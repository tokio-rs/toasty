use toasty_core::{
    schema::app::{self, Field, FieldId, FieldTy},
    stmt,
};

use crate::engine::{planner::ng::lower::LowerStatement, Simplify};

#[derive(Debug)]
enum Mutation {
    /// An existing value is being associated with the relation source.
    Associate {
        // Target relation expression
        expr: stmt::Expr,

        // When true, any previous values associated with the source relation
        // need to be cleared.
        exclusive: bool,
    },

    /// Disassociate an existing relation
    Disassociate {
        /// Target relation expression
        expr: stmt::Expr,
    },

    /// Disassociate any target relations
    DisassociateAll {
        // True when disasociating before deleting a record.
        delete: bool,
    },
}

trait RelationSource {
    /// Return a query representing the source selection
    fn selection(&self) -> stmt::Query;

    /// Update a field expression
    fn set(&mut self, field: FieldId, expr: stmt::Expr);
}

struct InsertRelationSource<'a> {
    model: &'a app::Model,
    row: &'a mut stmt::Expr,
}

struct UpdateRelationSource<'a> {
    model: &'a app::Model,
    filter: &'a stmt::Filter,
    assignments: &'a mut stmt::Assignments,
}

impl LowerStatement<'_, '_> {
    pub(super) fn plan_stmt_delete_relations(&mut self, mut stmt: &stmt::Delete) {
        // Cascading deletes are only handled at the application level
        let Some(model) = self.expr_cx.target().as_model() else {
            return;
        };

        // Handle any cascading deletes
        for field in model.fields.iter() {
            self.plan_mut_relation_field(
                field,
                Mutation::DisassociateAll { delete: true },
                &mut stmt,
            );
        }
    }

    pub(super) fn plan_stmt_insert_relations(&mut self, row: &mut stmt::Expr) {
        let model = self.expr_cx.target().as_model_unwrap();

        for (i, field) in model.fields.iter().enumerate() {
            if field.is_relation() {
                let expr = row.entry_mut(i).take();

                if expr.is_value_null() {
                    if !field.nullable && field.ty.is_has_one() {
                        panic!(
                            "Insert missing non-nullable field; model={}; field={}; ty={:#?}; expr={:#?}",
                            model.name.upper_camel_case(),
                            field.name,
                            field.ty,
                            expr
                        );
                    }

                    continue;
                }

                self.plan_mut_relation_field(
                    field,
                    Mutation::Associate {
                        expr,
                        exclusive: field.ty.is_belongs_to(),
                    },
                    &mut InsertRelationSource { model, row },
                );
            }
        }
    }

    pub(super) fn plan_stmt_update_relations(
        &mut self,
        assignments: &mut stmt::Assignments,
        filter: &stmt::Filter,
    ) {
        let model = self.expr_cx.target().as_model_unwrap();

        for (i, field) in model.fields.iter().enumerate() {
            if !field.is_relation() {
                continue;
            }

            let Some(assignment) = assignments.take(&i) else {
                continue;
            };

            let mutation = match assignment.op {
                stmt::AssignmentOp::Set => match assignment.expr {
                    e if e.is_value_null() => Mutation::DisassociateAll { delete: false },
                    expr => Mutation::Associate {
                        expr,
                        exclusive: true,
                    },
                },
                stmt::AssignmentOp::Insert => {
                    assert!(
                        field.ty.is_has_many(),
                        "TODO: can other types support insert?"
                    );
                    debug_assert!(!assignment.expr.is_value_null(), "should not be null");
                    Mutation::Associate {
                        expr: assignment.expr,
                        exclusive: false,
                    }
                }
                stmt::AssignmentOp::Remove => {
                    assert!(
                        field.ty.is_has_many(),
                        "TODO: can other types support remove?"
                    );
                    debug_assert!(!assignment.expr.is_value_null(), "should not be null");
                    Mutation::Disassociate {
                        expr: assignment.expr,
                    }
                }
            };

            self.plan_mut_relation_field(
                field,
                mutation,
                &mut UpdateRelationSource {
                    model,
                    filter,
                    assignments: &mut *assignments,
                },
            );
        }
    }

    fn plan_mut_relation_field(
        &mut self,
        field: &app::Field,
        op: Mutation,
        source: &mut dyn RelationSource,
    ) {
        match &field.ty {
            FieldTy::HasOne(..) => {
                self.relation_step(field, |lower| lower.plan_mut_has_one(field, op, source));
            }
            FieldTy::HasMany(..) => {
                self.relation_step(field, |lower| lower.plan_mut_has_many(field, op, source));
            }
            FieldTy::BelongsTo(_) => {
                self.plan_mut_belongs_to(field, op, source);
            }
            _ => (),
        }
    }

    fn plan_mut_has_many(&mut self, field: &Field, op: Mutation, source: &mut dyn RelationSource) {
        let has_many = field.ty.expect_has_many();
        let pair = self.field(has_many.pair);

        self.plan_mut_has_n(field, pair, op, source);
    }

    fn plan_mut_has_one(&mut self, field: &Field, op: Mutation, source: &mut dyn RelationSource) {
        let has_one = field.ty.expect_has_one();
        let pair = self.field(has_one.pair);

        self.plan_mut_has_n(field, pair, op, source);
    }

    fn plan_mut_has_n(
        &mut self,
        field: &Field,
        pair: &Field,
        op: Mutation,
        source: &mut dyn RelationSource,
    ) {
        match op {
            Mutation::DisassociateAll { .. } => {
                self.plan_mut_has_n_disassociate_all(pair, source);
            }
            Mutation::Associate { expr, exclusive } => {
                if exclusive {
                    self.plan_mut_has_n_disassociate_all(pair, source);
                }

                self.plan_mut_has_n_associate_expr(field, pair, expr, source)
            }
            Mutation::Disassociate { expr } => {
                debug_assert!(field.ty.is_has_many());
                self.plan_mut_has_many_disassociate_expr(field, pair, expr, source);
            }
        }
    }

    fn plan_mut_has_n_associate_expr(
        &mut self,
        field: &Field,
        pair: &Field,
        expr: stmt::Expr,
        source: &mut dyn RelationSource,
    ) {
        match expr {
            stmt::Expr::List(expr_list) => {
                for expr in expr_list.items {
                    self.plan_mut_has_n_associate_expr(field, pair, expr, source);
                }
            }
            stmt::Expr::Stmt(expr_stmt) => {
                self.plan_mut_has_n_associate_stmt(field, pair, *expr_stmt.stmt, source);
            }
            stmt::Expr::Value(stmt::Value::List(value_list)) => {
                for value in value_list {
                    self.plan_mut_has_n_associate_value(pair, value, source);
                }
            }
            stmt::Expr::Value(value) => {
                self.plan_mut_has_n_associate_value(pair, value, source);
            }
            _ => todo!("field={field:#?}, expr={expr:#?}"),
        }
    }

    fn plan_mut_has_n_associate_value(
        &mut self,
        pair: &Field,
        value: stmt::Value,
        source: &mut dyn RelationSource,
    ) {
        assert!(!value.is_list());

        let mut stmt = stmt::Query::new_select(
            pair.id.model,
            stmt::Expr::eq(stmt::Expr::key(pair.id.model), value),
        )
        .update();

        stmt.assignments
            .set(pair.id, stmt::Expr::stmt(source.selection()));

        self.new_dependency(stmt);
    }

    fn plan_mut_has_many_disassociate_expr(
        &mut self,
        field: &Field,
        pair: &Field,
        expr: stmt::Expr,
        source: &dyn RelationSource,
    ) {
        match expr {
            stmt::Expr::Value(value) => {
                self.plan_mut_has_many_disassociate_value(pair, value, source)
            }
            _ => todo!("field={field:#?}; expr={expr:#?}"),
        }
    }

    fn plan_mut_has_many_disassociate_value(
        &mut self,
        pair: &Field,
        value: stmt::Value,
        source: &dyn RelationSource,
    ) {
        let selection = stmt::Query::new_select(
            pair.id.model,
            stmt::Expr::eq(stmt::Expr::key(pair.id.model), value),
        );

        if pair.nullable {
            let mut stmt = selection.update();

            // This protects against races.
            stmt.condition = stmt::Condition::new(self.relation_pair_filter(pair.id, source));
            stmt.assignments.set(pair, stmt::Value::Null);
            self.new_dependency(stmt);
        } else {
            self.new_dependency(selection.delete());
        }
    }

    fn plan_mut_has_n_disassociate_all(&mut self, pair: &Field, source: &dyn RelationSource) {
        let query = self.relation_pair_scope(pair.id, source);

        if pair.nullable {
            let mut update = query.update();
            update.assignments.set(pair.id, stmt::Value::Null);

            self.new_dependency(update);
        } else {
            self.new_dependency(query.delete());
        }
    }

    fn plan_mut_has_n_associate_stmt(
        &mut self,
        field: &Field,
        pair: &Field,
        stmt: stmt::Statement,
        source: &dyn RelationSource,
    ) {
        match stmt {
            stmt::Statement::Insert(stmt) => {
                self.plan_mut_has_n_associate_insert(field, pair, stmt, source)
            }
            _ => todo!("stmt={:#?}", stmt),
        }
    }

    fn plan_mut_has_n_associate_insert(
        &mut self,
        _field: &Field,
        pair: &Field,
        mut stmt: stmt::Insert,
        source: &dyn RelationSource,
    ) {
        // Returning does nothing in this context.
        stmt.returning = None;

        debug_assert_eq!(stmt.target.as_model_unwrap(), pair.id.model);
        debug_assert!(stmt.target.is_model());

        stmt.target = self.relation_pair_scope(pair.id, source).into();
        self.new_dependency(stmt);
    }

    fn plan_mut_belongs_to(
        &mut self,
        field: &Field,
        op: Mutation,
        source: &mut dyn RelationSource,
    ) {
        match op {
            Mutation::Associate { expr, .. } => {
                self.plan_mut_belongs_to_associate(field, expr, source)
            }
            Mutation::DisassociateAll { delete } => {
                self.plan_mut_belongs_to_disassociate(field, delete, source);
            }
            Mutation::Disassociate { .. } => {
                todo!("is this needed?")
            }
        }
    }

    fn plan_mut_belongs_to_disassociate(
        &mut self,
        field: &Field,
        delete: bool,
        source: &mut dyn RelationSource,
    ) {
        if !field.nullable {
            // TODO: probably don't panic if `delete`
            todo!("invalid statement. handle this case");
        }

        let belongs_to = field.ty.expect_belongs_to();

        if let Some(pair_id) = belongs_to.pair {
            let pair = self.field(pair_id);

            if pair.ty.is_has_one() && !pair.nullable {
                self.relation_step(field, |planner| {
                    let delete = planner.relation_pair_scope(pair.id, source).delete();
                    planner.new_dependency(delete);
                });
            }
        }

        if !delete {
            for fk_field in &belongs_to.foreign_key.fields {
                source.set(fk_field.source, stmt::Expr::null());
            }
        }
    }

    fn plan_mut_belongs_to_associate(
        &mut self,
        field: &Field,
        expr: stmt::Expr,
        source: &mut dyn RelationSource,
    ) {
        if let Some(pair_id) = field.pair() {
            if self.field(pair_id).ty.is_has_one() {
                // Disassociate an existing HasOne. This handles the case where
                // if the HasOne association is required (i.e. *not* `Option`),
                // then the record gets deleted.
                self.plan_mut_belongs_to_disassociate(field, false, source);

                // This handles disassociating any *other* instances of the current
                // model that already are associated with the target model being
                // passed it. This is necessary because of the 1-1 relation
                // mapping.
                if field.nullable {
                    self.relation_step(field, |planner| {
                        assert!(expr.is_value(), "TODO; expr={expr:#?}");

                        let scope = stmt::Query::new_select(
                            field.id.model,
                            stmt::Expr::eq(stmt::Expr::ref_self_field(field), expr.clone()),
                        );

                        if field.nullable {
                            let mut stmt = scope.update();
                            stmt.assignments.set(field.id, stmt::Value::Null);
                            planner.new_dependency(stmt);
                        } else {
                            todo!();
                        }
                    });
                } else {
                    todo!()
                }
            }
        }

        match expr {
            expr if expr.is_value() => {
                assert!(!expr.is_value_null());

                self.set_relation_field(field, expr, source);
            }
            stmt::Expr::Stmt(expr_stmt) => {
                self.plan_mut_belongs_to_associate_stmt(field, *expr_stmt.stmt, source);
            }
            _ => todo!("expr={expr:#?}"),
        }
    }

    fn plan_mut_belongs_to_associate_stmt(
        &mut self,
        field: &Field,
        stmt: stmt::Statement,
        source: &mut dyn RelationSource,
    ) {
        let belongs_to = field.ty.expect_belongs_to();

        match stmt {
            stmt::Statement::Insert(mut insert) => {
                if let stmt::ExprSet::Values(values) = &insert.source.body {
                    assert_eq!(1, values.rows.len());
                }

                // Only returning that makes sense here as that is the type that
                // "belongs" in this field. We translate it to the key to set
                // the FK fields in the source model.
                assert!(matches!(
                    insert.returning,
                    Some(stmt::Returning::Model { .. })
                ));

                // Previous value of returning does nothing in this
                // context
                insert.returning = Some(
                    stmt::Expr::record(
                        belongs_to
                            .foreign_key
                            .fields
                            .iter()
                            .map(|fk_field| stmt::Expr::ref_self_field(fk_field.target)),
                    )
                    .into(),
                );

                let stmt_id = self.new_dependency(insert);

                let stmt_info = &self.state.store[stmt_id];

                let returning = stmt_info.stmt.as_ref().unwrap().returning().expect("bug");
                let stmt::Returning::Value(value) = returning.clone() else {
                    todo!()
                };
                let stmt::Value::List(rows) = value else {
                    todo!()
                };
                assert_eq!(1, rows.len());

                self.set_relation_field(field, rows.into_iter().next().unwrap().into(), source);
            }
            stmt::Statement::Query(query) => {
                // First, we have to try to extract the FK from the select
                // without having to perform the query
                //
                // TODO: make this less terrible lol
                let fields: Vec<_> = belongs_to
                    .foreign_key
                    .fields
                    .iter()
                    .map(|fk_field| fk_field.target)
                    .collect();

                let Some(expr) = Simplify::new(self.schema()).extract_key_value(&fields, &query)
                else {
                    todo!("belongs_to={:#?}; stmt={:#?}", belongs_to, query);
                };

                self.plan_mut_belongs_to_associate(field, expr, source);
            }
            _ => todo!("stmt={:#?}", stmt),
        }
    }

    /// Translate a source model scope to a target model scope for a has_one
    /// relation.
    fn relation_pair_scope(&self, pair: FieldId, source: &dyn RelationSource) -> stmt::Query {
        stmt::Query::new_select(pair.model, self.relation_pair_filter(pair, source))
    }

    fn relation_pair_filter(&self, pair: FieldId, source: &dyn RelationSource) -> stmt::Expr {
        stmt::Expr::in_subquery(stmt::Expr::ref_self_field(pair), source.selection())
    }

    fn relation_step(&mut self, field: &Field, f: impl FnOnce(&mut LowerStatement)) {
        if let Some(pair) = field.pair() {
            if self.state.relations.last().copied() == Some(pair) {
                return;
            }
        }

        self.state.relations.push(field.id);
        f(self);
        self.state.relations.pop();
    }

    fn set_relation_field(&self, field: &Field, expr: stmt::Expr, source: &mut dyn RelationSource) {
        let app::FieldTy::BelongsTo(belongs_to) = &field.ty else {
            todo!("field={field:#?}")
        };

        let fk_fields = &belongs_to.foreign_key.fields;

        if let Some(len) = expr.record_len() {
            assert_eq!(len, fk_fields.len(), "expr={expr:#?}");
            let fk_values = expr.into_record_items().unwrap();

            for (fk_field, fk_value) in fk_fields.iter().zip(fk_values) {
                source.set(fk_field.source, fk_value);
            }
        } else {
            let [fk_field] = &fk_fields[..] else { todo!() };
            source.set(fk_field.source, expr);
        }
    }
}

impl RelationSource for &stmt::Delete {
    fn selection(&self) -> stmt::Query {
        stmt::Delete::selection(self)
    }

    fn set(&mut self, _field: FieldId, _expr: stmt::Expr) {
        unimplemented!("delete statements do not need to update field values");
    }
}

impl RelationSource for UpdateRelationSource<'_> {
    fn selection(&self) -> stmt::Query {
        stmt::Query::new_select(self.model, self.filter.clone())
    }

    fn set(&mut self, field: FieldId, expr: stmt::Expr) {
        self.assignments.set(field, expr);
    }
}

impl RelationSource for InsertRelationSource<'_> {
    fn selection(&self) -> stmt::Query {
        // The owner's primary key
        let mut args = vec![];

        for pk_field in self.model.primary_key_fields() {
            // let expr = eval::Expr::from(expr.entry(pk_field.id.index).to_value());
            args.push(self.row.entry(pk_field.id.index).to_value());
        }

        self.model.find_by_id(&args)
    }

    fn set(&mut self, field: FieldId, expr: stmt::Expr) {
        assert_eq!(self.model.id, field.model);
        self.row.as_record_mut()[field.index] = expr;
    }
}
