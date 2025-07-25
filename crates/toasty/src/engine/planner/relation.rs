use super::*;
use app::{Field, FieldId, FieldTy, HasMany, HasOne};

use crate::Result;

use std::mem;

impl Planner<'_> {
    pub(super) fn plan_mut_relation_field(
        &mut self,
        field: &app::Field,
        op: stmt::AssignmentOp,
        expr: &mut stmt::Expr,
        selection: &stmt::Query,
        is_insert: bool,
    ) -> Result<()> {
        match &field.ty {
            FieldTy::HasOne(has_one) => {
                assert_ne!(self.relations.last(), Some(&has_one.pair));

                self.relation_step(field, |planner| {
                    planner.plan_mut_has_one_expr(has_one, mem::take(expr), selection, is_insert)
                })?;
            }
            FieldTy::HasMany(has_many) => {
                assert_ne!(self.relations.last(), Some(&has_many.pair));

                self.relation_step(field, |planner| {
                    planner.plan_mut_has_many_expr(has_many, op, mem::take(expr), selection)
                })?;
            }
            FieldTy::BelongsTo(_) => {
                self.plan_mut_belongs_to_expr(field, expr, selection, is_insert)?;
            }
            _ => (),
        }

        Ok(())
    }

    pub(super) fn plan_mut_belongs_to_expr(
        &mut self,
        field: &Field,
        expr: &mut stmt::Expr,
        scope: &stmt::Query,
        is_insert: bool,
    ) -> Result<()> {
        match expr {
            stmt::Expr::Value(v) => {
                self.plan_mut_belongs_to_value(field, v, scope, is_insert)?;
            }
            stmt::Expr::Stmt(_) => {
                let expr_stmt = mem::take(expr).into_stmt();
                self.plan_mut_belongs_to_stmt(field, *expr_stmt.stmt, expr, scope, is_insert)?;
                debug_assert!(!expr.is_stmt());
            }
            _ => todo!("expr={:#?}", expr),
        }

        Ok(())
    }

    pub fn plan_mut_belongs_to_value(
        &mut self,
        field: &Field,
        value: &mut stmt::Value,
        scope: &stmt::Query,
        is_insert: bool,
    ) -> Result<()> {
        if value.is_null() {
            assert!(!is_insert);

            if !field.nullable {
                todo!("invalid statement. handle this case");
            }
        } else {
            self.relation_step(field, |planner| {
                let belongs_to = field.ty.expect_belongs_to();

                // If the BelongsTo field is the pair of a HasOne, then any
                // previous assignment to the pair needs to be cleared out.
                let nullify = belongs_to
                    .pair
                    .map(|pair| planner.schema.app.field(pair).ty.is_has_one())
                    .unwrap_or(false);

                // If the pair is *not* has_many, then any previous assignment
                // to the pair needs to be cleared out.
                if nullify {
                    planner.plan_mut_belongs_to_nullify(field, scope)?;

                    let [fk_field] = &belongs_to.foreign_key.fields[..] else {
                        todo!("composite key")
                    };

                    let scope = stmt::Query::filter(
                        field.id.model,
                        stmt::Expr::eq(fk_field.source, value.clone()),
                    );

                    if field.nullable {
                        let mut stmt = scope.update();
                        stmt.assignments.set(field.id, stmt::Value::Null);
                        planner.plan_stmt(&Context::default(), stmt.into())?;
                    } else {
                        todo!("delete any models with the association currently being set");
                    }
                }

                Ok(())
            })?;
        }

        Ok(())
    }

    pub(super) fn plan_mut_belongs_to_stmt(
        &mut self,
        field: &Field,
        stmt: stmt::Statement,
        expr: &mut stmt::Expr,
        scope: &stmt::Query,
        is_insert: bool,
    ) -> Result<()> {
        let belongs_to = field.ty.expect_belongs_to();

        match stmt {
            stmt::Statement::Insert(mut insert) => {
                if let stmt::ExprSet::Values(values) = &insert.source.body {
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

                let insertion_output = self.plan_stmt(&Context::default(), insert.into())?.unwrap();

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
                self.plan_mut_belongs_to_expr(field, expr, scope, is_insert)?;
            }
            _ => todo!("stmt={:#?}", stmt),
        }

        Ok(())
    }

    fn plan_mut_belongs_to_nullify(&mut self, field: &Field, scope: &stmt::Query) -> Result<()> {
        self.relation_step(field, |planner| {
            let belongs_to = field.ty.expect_belongs_to();

            if let Some(pair) = belongs_to.pair.map(|pair| planner.schema.app.field(pair)) {
                if pair.ty.is_has_one() && !pair.nullable {
                    let mut scope = scope.clone();

                    // If the belongs_to is nullable, then we want to only update
                    // instances that have a belongs_to that is not null.
                    if field.nullable {
                        let filter = &mut scope.body.as_select_mut().filter;
                        *filter = stmt::Expr::and(
                            filter.take(),
                            stmt::Expr::ne(field, stmt::Value::Null),
                        );
                    }

                    let delete = planner.relation_pair_scope(pair.id, scope).delete();
                    planner.plan_stmt(&Context::default(), delete.into())?;
                }
            }

            Ok(())
        })
    }

    /// Plan writing to a has_many field from either an insertion or update path.
    pub(super) fn plan_mut_has_many_expr(
        &mut self,
        has_many: &HasMany,
        op: stmt::AssignmentOp,
        expr: stmt::Expr,
        scope: &stmt::Query,
    ) -> Result<()> {
        match expr {
            stmt::Expr::Stmt(expr_stmt) => {
                assert!(!op.is_remove());
                self.plan_mut_has_many_stmt(has_many, *expr_stmt.stmt, scope)?
            }
            stmt::Expr::List(expr_list) => {
                for expr in expr_list.items {
                    self.plan_mut_has_many_expr(has_many, op, expr, scope)?;
                }
            }
            stmt::Expr::Value(stmt::Value::List(value_list)) => {
                for value in value_list {
                    self.plan_mut_has_many_value(has_many, op, value, scope)?;
                }
            }
            stmt::Expr::Value(value) => {
                self.plan_mut_has_many_value(has_many, op, value, scope)?;
            }
            _ => todo!("expr={:#?}", expr),
        }

        Ok(())
    }

    fn plan_mut_has_many_stmt(
        &mut self,
        has_many: &HasMany,
        stmt: stmt::Statement,
        scope: &stmt::Query,
    ) -> Result<()> {
        match stmt {
            stmt::Statement::Insert(stmt) => self.plan_mut_has_many_insert(has_many, stmt, scope),
            _ => todo!("stmt={:#?}", stmt),
        }
    }

    fn plan_mut_has_many_value(
        &mut self,
        has_many: &HasMany,
        op: stmt::AssignmentOp,
        value: stmt::Value,
        scope: &stmt::Query,
    ) -> Result<()> {
        assert!(!value.is_list());

        if op.is_remove() {
            self.plan_mut_has_many_value_remove(has_many, value, scope)?;
        } else {
            let mut stmt = stmt::Query::filter(
                has_many.target,
                stmt::Expr::eq(stmt::Expr::key(has_many.target), value),
            )
            .update();

            stmt.assignments
                .set(has_many.pair, stmt::Expr::stmt(scope.clone()));
            let out = self.plan_stmt(&Context::default(), stmt.into())?;
            assert!(out.is_none());
        }

        Ok(())
    }

    fn plan_mut_has_many_value_remove(
        &mut self,
        has_many: &HasMany,
        value: stmt::Value,
        scope: &stmt::Query,
    ) -> Result<()> {
        let pair = self.schema.app.field(has_many.pair);

        let selection = stmt::Query::filter(
            has_many.target,
            stmt::Expr::eq(stmt::Expr::key(has_many.target), value),
        );

        if pair.nullable {
            let mut stmt = selection.update();

            // This protects against races.
            stmt.condition = Some(stmt::Expr::in_subquery(has_many.pair, scope.clone()));
            stmt.assignments.set(has_many.pair, stmt::Value::Null);
            let out = self.plan_stmt(&Context::default(), stmt.into())?;
            assert!(out.is_none());
        } else {
            let out = self.plan_stmt(&Context::default(), selection.delete().into())?;
            assert!(out.is_none());
        }

        Ok(())
    }

    pub(super) fn plan_mut_has_one_expr(
        &mut self,
        // Has one association with the base model as the source
        has_one: &HasOne,
        // Expression to use as the value for the field.
        expr: stmt::Expr,
        // Scope of the mutation
        scope: &stmt::Query,
        // If the mutation is from an insert or update
        is_insert: bool,
    ) -> Result<()> {
        match expr {
            stmt::Expr::Value(stmt::Value::Null) => {
                self.plan_mut_has_one_nullify(has_one, scope)?;
            }
            stmt::Expr::Value(value) => {
                self.plan_mut_has_one_value(has_one, value, scope, is_insert)?;
            }
            stmt::Expr::Stmt(expr_stmt) => {
                self.plan_mut_has_one_stmt(has_one, *expr_stmt.stmt, scope, is_insert)?;
            }
            expr => todo!("expr={:#?}", expr),
        }

        Ok(())
    }

    pub(super) fn plan_mut_has_one_stmt(
        &mut self,
        has_one: &HasOne,
        stmt: stmt::Statement,
        scope: &stmt::Query,
        is_insert: bool,
    ) -> Result<()> {
        match stmt {
            stmt::Statement::Insert(stmt) => {
                self.plan_mut_has_one_insert(has_one, stmt, scope, is_insert)?
            }
            _ => todo!("stmt={:#?}", stmt),
        }

        Ok(())
    }

    pub(super) fn plan_mut_has_one_nullify(
        &mut self,
        has_one: &HasOne,
        scope: &stmt::Query,
    ) -> Result<()> {
        let pair_scope = self.relation_pair_scope(has_one.pair, scope.clone());

        if self.schema.app.field(has_one.pair).nullable {
            // TODO: unify w/ has_many ops?
            let mut stmt = pair_scope.update();
            stmt.assignments.set(has_one.pair, stmt::Value::Null);
            let out = self.plan_stmt(&Context::default(), stmt.into())?;
            assert!(out.is_none());
        } else {
            let out = self.plan_stmt(&Context::default(), pair_scope.delete().into())?;
            assert!(out.is_none());
        }

        Ok(())
    }

    pub(super) fn plan_mut_has_one_value(
        &mut self,
        has_one: &HasOne,
        value: stmt::Value,
        scope: &stmt::Query,
        is_insert: bool,
    ) -> Result<()> {
        // Only nullify if calling from an update context
        if !is_insert {
            // Update the row of the existing association (if there is one)
            self.plan_mut_has_one_nullify(has_one, scope)?;
        }

        let mut stmt = stmt::Query::filter(
            has_one.target,
            stmt::Expr::eq(stmt::Expr::key(has_one.target), value),
        )
        .update();

        stmt.assignments
            .set(has_one.pair, stmt::Expr::stmt(scope.clone()));

        let out = self.plan_stmt(&Context::default(), stmt.into())?;
        assert!(out.is_none());
        Ok(())
    }

    fn plan_mut_has_many_insert(
        &mut self,
        has_many: &HasMany,
        mut stmt: stmt::Insert,
        scope: &stmt::Query,
    ) -> Result<()> {
        // Returning does nothing in this context.
        stmt.returning = None;

        assert_eq!(stmt.target.as_model(), has_many.target);

        stmt.target = self
            .relation_pair_scope(has_many.pair, scope.clone())
            .into();

        let out = self.plan_stmt(&Context::default(), stmt.into())?;
        assert!(out.is_none());
        Ok(())
    }

    fn plan_mut_has_one_insert(
        &mut self,
        has_one: &HasOne,
        mut stmt: stmt::Insert,
        scope: &stmt::Query,
        is_insert: bool,
    ) -> Result<()> {
        // Returning does nothing in this context
        stmt.returning = None;

        // Only nullify if calling from an update context
        if !is_insert {
            // Update the row of the existing association (if there is one)
            self.plan_mut_has_one_nullify(has_one, scope)?;
        }

        stmt.target.constrain(
            self.relation_pair_scope(has_one.pair, scope.clone())
                .body
                .into_select()
                .filter,
        );

        let out = self.plan_stmt(&Context::default(), stmt.into())?;
        assert!(out.is_none());
        Ok(())
    }

    /// Translate a source model scope to a target model scope for a has_one
    /// relation.
    fn relation_pair_scope(&self, pair: FieldId, scope: stmt::Query) -> stmt::Query {
        stmt::Query::filter(pair.model, stmt::Expr::in_subquery(pair, scope))
    }

    fn relation_step(
        &mut self,
        field: &Field,
        f: impl FnOnce(&mut Planner) -> Result<()>,
    ) -> Result<()> {
        if let Some(pair) = field.pair() {
            if self.relations.last().copied() == Some(pair) {
                return Ok(());
            }
        }

        self.relations.push(field.id);

        let ret = f(self);

        self.relations.pop();

        ret
    }
}
