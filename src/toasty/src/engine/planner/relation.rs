use super::*;

use std::mem;

impl<'stmt> Planner<'_, 'stmt> {
    pub(super) fn plan_mut_relation_field(
        &mut self,
        field: &Field,
        expr: &mut stmt::Expr<'stmt>,
        selection: &stmt::Query<'stmt>,
        is_insert: bool,
    ) {
        match &field.ty {
            FieldTy::HasOne(has_one) => {
                assert_ne!(self.relations.last(), Some(&has_one.pair));

                self.relation_step(field, |planner| {
                    let model = planner.schema.model(field.id.model);
                    planner.plan_mut_has_one_expr(
                        model,
                        has_one,
                        mem::take(expr),
                        todo!(),
                        is_insert,
                    );
                });
            }
            FieldTy::HasMany(has_many) => {
                assert_ne!(self.relations.last(), Some(&has_many.pair));

                self.relation_step(field, |planner| {
                    planner.plan_mut_has_many_expr(has_many, mem::take(expr), selection);
                });
            }
            FieldTy::BelongsTo(_) => {
                self.plan_mut_belongs_to_expr(field, expr, selection, is_insert);
            }
            _ => return,
        }
    }

    pub(super) fn plan_mut_belongs_to_expr(
        &mut self,
        field: &Field,
        expr: &mut stmt::Expr<'stmt>,
        scope: &stmt::Query<'stmt>,
        is_insert: bool,
    ) {
        match expr {
            stmt::Expr::Value(v) => {
                self.plan_mut_belongs_to_value(field, v, scope, is_insert);
            }
            stmt::Expr::Stmt(_) => {
                let expr_stmt = mem::take(expr).into_stmt();
                self.plan_mut_belongs_to_stmt(field, *expr_stmt.stmt, expr, scope, is_insert);
                debug_assert!(!expr.is_stmt());
            }
            _ => todo!("expr={:#?}", expr),
        }
    }

    pub fn plan_mut_belongs_to_value(
        &mut self,
        field: &Field,
        value: &mut stmt::Value<'stmt>,
        scope: &stmt::Query<'stmt>,
        is_insert: bool,
    ) {
        if value.is_null() {
            assert!(!is_insert);

            if !field.nullable {
                todo!("invalid statement. handle this case");
            }
        } else {
            self.relation_step(field, |planner| {
                let belongs_to = field.ty.expect_belongs_to();
                let pair = planner.schema.field(belongs_to.pair);

                // If the pair is *not* has_many, then any previous assignment
                // to the pair needs to be cleared out.
                if !pair.ty.is_has_many() {
                    planner.plan_mut_belongs_to_nullify(field, scope);

                    let [fk_field] = &belongs_to.foreign_key.fields[..] else {
                        todo!("composite key")
                    };

                    let scope = stmt::Query::filter(
                        field.id.model,
                        stmt::Expr::eq(fk_field.source, value.clone()),
                    );

                    if field.nullable {
                        let mut stmt = scope.update(planner.schema);
                        stmt.assignments.set(field.id, stmt::Value::Null);
                        planner.plan_update(stmt);
                    } else {
                        todo!("delete any models with the association currently being set");
                    }
                }
            });
        }
    }

    pub(super) fn plan_mut_belongs_to_stmt(
        &mut self,
        field: &Field,
        stmt: stmt::Statement<'stmt>,
        expr: &mut stmt::Expr<'stmt>,
        scope: &stmt::Query<'stmt>,
        is_insert: bool,
    ) {
        let belongs_to = field.ty.expect_belongs_to();

        match stmt {
            stmt::Statement::Insert(mut insert) => {
                if let stmt::ExprSet::Values(values) = &*insert.source.body {
                    assert_eq!(1, values.rows.len());
                }

                // Only returning that makes sense here as that is the type that
                // "belongs" in this field. We translate it to the key to set
                // the FK fields in the source model.
                assert!(matches!(insert.returning, Some(stmt::Returning::Star)));

                // Previous value of returning does nothing in this
                // context
                insert.returning = Some(match &belongs_to.foreign_key.fields[..] {
                    [fk_field] => stmt::Returning::Expr(stmt::Expr::field(fk_field.target)),
                    _ => {
                        todo!("composite keys");
                    }
                });

                let insertion_output = self.plan_insert(insert).unwrap();

                // An optimization that always holds for now. In the
                // future, this will not be the case. The
                // unoptimized path has not been implemented yet.
                let insertion_output = self.take_const_var(insertion_output);

                assert_eq!(1, insertion_output.len());
                *expr = insertion_output.into_iter().next().unwrap().into();
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

                // TODO: move this out
                let Some(e) =
                    simplify::lift_pk_select::lift_key_select(self.schema, &fields, &query)
                else {
                    todo!("belongs_to={:#?}; stmt={:#?}", belongs_to, query);
                };

                *expr = e;

                // Plan again
                self.plan_mut_belongs_to_expr(field, expr, scope, is_insert);
            }
            _ => todo!("stmt={:#?}", stmt),
        }
    }

    fn plan_mut_belongs_to_nullify(&mut self, field: &Field, scope: &stmt::Query<'stmt>) {
        self.relation_step(field, |planner| {
            let belongs_to = field.ty.expect_belongs_to();
            let pair = planner.schema.field(belongs_to.pair);

            if !pair.ty.is_has_many() && !pair.nullable {
                let mut scope = scope.clone();

                // If the belongs_to is nullable, then we want to only update
                // instances that have a belongs_to that is not null.
                if field.nullable {
                    let filter = &mut scope.body.as_select_mut().filter;
                    *filter =
                        stmt::Expr::and(filter.take(), stmt::Expr::ne(field, stmt::Value::Null));
                }

                todo!();
                /*
                planner.plan_delete(stmt::Delete {
                    selection: planner.relation_pair_scope(belongs_to.pair, scope.clone()),
                });
                */
            }
        });
    }

    /// Plan writing to a has_many field from either an insertion or update path.
    pub(super) fn plan_mut_has_many_expr(
        &mut self,
        has_many: &HasMany,
        expr: stmt::Expr<'stmt>,
        scope: &stmt::Query<'stmt>,
    ) {
        match expr {
            stmt::Expr::Stmt(expr_stmt) => {
                self.plan_mut_has_many_stmt(has_many, *expr_stmt.stmt, scope)
            }
            stmt::Expr::Concat(expr_concat) => {
                for expr in expr_concat {
                    self.plan_mut_has_many_expr(has_many, expr, scope);
                }
            }
            stmt::Expr::Value(value) => {
                self.plan_mut_has_many_value(has_many, value, scope);
            }
            _ => todo!("expr={:#?}", expr),
        }
    }

    fn plan_mut_has_many_stmt(
        &mut self,
        has_many: &HasMany,
        stmt: stmt::Statement<'stmt>,
        scope: &stmt::Query<'stmt>,
    ) {
        match stmt {
            stmt::Statement::Insert(stmt) => self.plan_mut_has_many_insert(has_many, stmt, scope),
            _ => todo!("stmt={:#?}", stmt),
        }
    }

    fn plan_mut_has_many_value(
        &mut self,
        has_many: &HasMany,
        value: stmt::Value<'stmt>,
        scope: &stmt::Query<'stmt>,
    ) {
        todo!("value={:#?}", value);
        let mut stmt = stmt::Query::filter(
            has_many.target,
            // stmt::Expr::eq(stmt::Expr::self_expr(), value),
            stmt::Expr::default(),
        )
        .update(self.schema);

        stmt.assignments
            .set(has_many.pair, stmt::ExprStmt::new(scope.clone()));
        self.plan_update(stmt);
    }

    pub(super) fn plan_mut_has_one_expr(
        &mut self,
        // Base model
        model: &Model,
        // Has one association with the base model as the source
        has_one: &HasOne,
        // Expression to use as the value for the field.
        expr: stmt::Expr<'stmt>,
        // Which instances of the base model to update
        filter: &stmt::Expr<'stmt>,
        // If the mutation is from an insert or update
        is_insert: bool,
    ) {
        match expr {
            stmt::Expr::Value(stmt::Value::Null) => {
                self.plan_mut_has_one_nullify(model, has_one, filter);
            }
            stmt::Expr::Value(value) => {
                self.plan_mut_has_one_value(model, has_one, value, filter, is_insert);
            }
            stmt::Expr::Stmt(expr_stmt) => {
                self.plan_mut_has_one_stmt(model, has_one, *expr_stmt.stmt, filter, is_insert);
            }
            expr => todo!("expr={:#?}", expr),
        }
    }

    pub(super) fn plan_mut_has_one_stmt(
        &mut self,
        model: &Model,
        has_one: &HasOne,
        stmt: stmt::Statement<'stmt>,
        filter: &stmt::Expr<'stmt>,
        is_insert: bool,
    ) {
        match stmt {
            stmt::Statement::Insert(stmt) => {
                self.plan_mut_has_one_insert(model, has_one, stmt, filter, is_insert)
            }
            _ => todo!("stmt={:#?}", stmt),
        }
    }

    pub(super) fn plan_mut_has_one_nullify(
        &mut self,
        model: &Model,
        has_one: &HasOne,
        filter: &stmt::Expr<'stmt>,
    ) {
        let pair_scope = self.relation_pair_scope(model, has_one.pair, filter);

        if self.schema.field(has_one.pair).nullable {
            // TODO: unify w/ has_many ops?
            let mut stmt = pair_scope.update(self.schema);
            stmt.assignments.set(has_one.pair, stmt::Value::Null);
            self.plan_update(stmt);
        } else {
            self.plan_delete(pair_scope.delete());
        }
    }

    pub(super) fn plan_mut_has_one_value(
        &mut self,
        model: &Model,
        has_one: &HasOne,
        value: stmt::Value<'stmt>,
        filter: &stmt::Expr<'stmt>,
        is_insert: bool,
    ) {
        // Only nullify if calling from an update context
        if !is_insert {
            // Update the row of the existing association (if there is one)
            self.plan_mut_has_one_nullify(model, has_one, filter);
        }

        todo!("value = {:#?}", value);

        /*
        let mut stmt = stmt::Query::filter(
            has_one.target,
            // stmt::Expr::eq(stmt::Expr::self_expr(), value),
            stmt::Expr::default(),
        )
        .update(self.schema);

        stmt.assignments
            .set(has_one.pair, stmt::ExprStmt::new(scope.clone()));

        self.plan_update(stmt);
        */
    }

    fn plan_mut_has_many_insert(
        &mut self,
        has_many: &HasMany,
        mut stmt: stmt::Insert<'stmt>,
        scope: &stmt::Query<'stmt>,
    ) {
        // Returning does nothing in this context.
        stmt.returning = None;

        /*
            stmt.target.and(
                self.relation_pair_scope(has_many.pair, scope.clone())
                    .body
                    .into_select()
                    .filter,
            );
        */
        todo!();

        self.plan_insert(stmt);
    }

    fn plan_mut_has_one_insert(
        &mut self,
        model: &Model,
        has_one: &HasOne,
        mut stmt: stmt::Insert<'stmt>,
        filter: &stmt::Expr<'stmt>,
        is_insert: bool,
    ) {
        // Returning does nothing in this context
        stmt.returning = None;

        // Only nullify if calling from an update context
        if !is_insert {
            // Update the row of the existing association (if there is one)
            self.plan_mut_has_one_nullify(model, has_one, filter);
        }

        /*
        stmt.target.and(
            self.relation_pair_scope(has_one.pair, scope.clone())
                .body
                .into_select()
                .filter,
        );
        */
        todo!();

        self.plan_insert(stmt);
    }

    /// Translate a source model scope to a target model scope for a has_one
    /// relation.
    fn relation_pair_scope(
        &self,
        model: &Model,
        pair: FieldId,
        filter: &stmt::Expr<'stmt>,
    ) -> stmt::Query<'stmt> {
        let scope = stmt::Query::filter(model, filter.clone());
        stmt::Query::filter(pair.model, stmt::ExprInSubquery::new(pair, scope))
    }

    fn relation_step(&mut self, field: &Field, f: impl FnOnce(&mut Planner<'_, 'stmt>)) {
        if let Some(pair) = field.pair() {
            if self.relations.last().copied() == Some(pair) {
                return;
            }
        }

        self.relations.push(field.id);

        f(self);

        self.relations.pop();
    }
}
