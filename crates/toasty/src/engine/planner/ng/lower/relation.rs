use toasty_core::{
    schema::app::{self, BelongsTo, Field, FieldId, FieldTy, HasMany, HasOne, Model},
    stmt,
};

use crate::engine::planner::ng::lower::LowerStatement;

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

    pub(super) fn plan_stmt_update_relations(
        &mut self,
        assignments: &mut stmt::Assignments,
        filter: &stmt::Filter,
    ) {
        let model = self.expr_cx.target().as_model_unwrap();

        for (i, field) in model.fields.iter().enumerate() {
            let Some(assignment) = assignments.take(&i) else {
                continue;
            };

            let mutation = match assignment.op {
                stmt::AssignmentOp::Set => match &assignment.expr {
                    e if e.is_value_null() => Mutation::DisassociateAll { delete: false },
                    _ => todo!("assignment={assignment:#?}"),
                },
                stmt::AssignmentOp::Insert => todo!(),
                stmt::AssignmentOp::Remove => todo!(),
            };

            self.plan_mut_relation_field(field, mutation, &mut (model, filter, &mut *assignments));
        }
    }

    fn plan_mut_relation_field(
        &mut self,
        field: &app::Field,
        op: Mutation,
        source: &mut dyn RelationSource,
    ) {
        match &field.ty {
            FieldTy::HasOne(has_one) => {
                debug_assert_ne!(self.state.relations.last(), Some(&has_one.pair));

                self.relation_step(field, |lower| {
                    // lower.plan_mut_has_one_expr(has_one, mem::take(expr), selection, is_insert)
                    todo!()
                });
            }
            FieldTy::HasMany(has_many) => {
                debug_assert_ne!(self.state.relations.last(), Some(&has_many.pair));

                self.relation_step(field, |lower| lower.plan_mut_has_many(field, op, source));
            }
            FieldTy::BelongsTo(belongs_to) => {
                self.plan_mut_belongs_to(field, op, source);
            }
            _ => (),
        }
    }

    fn plan_mut_has_many(&mut self, field: &Field, op: Mutation, source: &dyn RelationSource) {
        let has_many = field.ty.expect_has_many();
        /*
                let pair = self.schema().app.field(rel.pair);

                // TODO: can this be unified with update?
                let query = stmt::Query::new_select(
                    rel.target,
                    stmt::Expr::in_subquery(
                        stmt::Expr::ref_self_field(rel.pair),
                        selection.clone(),
                    ),
                );

                if pair.nullable {
                    let mut update = query.update();
                    update.assignments.set(pair.id, stmt::Value::Null);

                    self.plan_stmt(update.into())?;
                } else {
                    self.plan_stmt(query.delete().into())?;
                }
        */

        let pair = self.schema().app.field(has_many.pair);

        match op {
            Mutation::DisassociateAll { .. } => {
                let query = self.relation_pair_scope(has_many.pair, source);

                if pair.nullable {
                    let mut update = query.update();
                    update.assignments.set(pair.id, stmt::Value::Null);

                    self.new_dependency(update.into());
                } else {
                    self.new_dependency(query.delete().into());
                }
            }
            _ => todo!(),
        }
    }

    fn plan_mut_has_many_delete(&mut self) {
        todo!()
    }

    fn plan_has_one_nullify(&mut self, field: &Field, source: &dyn RelationSource) {
        let has_one = field.ty.expect_has_one();
        let pair_scope = self.relation_pair_scope(has_one.pair, source);

        if self.schema().app.field(has_one.pair).nullable {
            // TODO: unify w/ has_many ops?
            let mut stmt = pair_scope.update();
            stmt.assignments.set(has_one.pair, stmt::Value::Null);
            /*
            let out = self.plan_stmt(stmt.into())?;
            assert!(out.is_none());
            */
            todo!("stmt={stmt:#?}");
        } else {
            let stmt = pair_scope.delete();
            /*
            let out = self.plan_stmt(pair_scope.delete().into())?;
            assert!(out.is_none());
            */
            todo!("stmt={stmt:#?}");
        }
    }

    fn plan_mut_belongs_to(
        &mut self,
        field: &Field,
        op: Mutation,
        source: &mut dyn RelationSource,
    ) {
        match op {
            Mutation::Associate { expr, exclusive } => {
                // belongs-to associations are always exclusive
                debug_assert!(exclusive);

                match expr {
                    stmt::Expr::Value(v) => {
                        assert!(!v.is_null());
                    }
                    stmt::Expr::Stmt(_) => {
                        todo!("stmt");
                    }
                    _ => todo!("expr={expr:#?}"),
                }
            }
            Mutation::DisassociateAll { delete } => {
                if !delete {
                    self.plan_mut_belongs_to_nullify(field, source);
                }
            }
        }
    }

    fn plan_mut_belongs_to_nullify(&mut self, field: &Field, source: &mut dyn RelationSource) {
        if !field.nullable {
            todo!("invalid statement. handle this case");
        }

        let belongs_to = field.ty.expect_belongs_to();

        for fk_field in &belongs_to.foreign_key.fields {
            source.set(fk_field.source, stmt::Expr::null());
        }
    }

    fn plan_mut_belongs_to_associate_value(&mut self, field: &Field, source: &dyn RelationSource) {
        let belongs_to = field.ty.expect_belongs_to();

        self.relation_step(field, |lower| {
            let Some(pair) = belongs_to.pair.map(|id| lower.schema().app.field(id)) else {
                return;
            };

            if pair.ty.is_has_one() {
                lower.plan_has_one_nullify(field, source);
            }
        });
    }

    /// Translate a source model scope to a target model scope for a has_one
    /// relation.
    fn relation_pair_scope(&self, pair: FieldId, source: &dyn RelationSource) -> stmt::Query {
        stmt::Query::new_select(
            pair.model,
            stmt::Expr::in_subquery(stmt::Expr::ref_self_field(pair), source.selection()),
        )
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
}

impl RelationSource for &stmt::Delete {
    fn selection(&self) -> stmt::Query {
        stmt::Delete::selection(self)
    }

    fn set(&mut self, _field: FieldId, _expr: stmt::Expr) {
        unimplemented!("delete statements do not need to update field values");
    }
}

impl RelationSource for (&Model, &stmt::Filter, &mut stmt::Assignments) {
    fn selection(&self) -> stmt::Query {
        stmt::Query::new_select(self.0, self.1.clone())
    }

    fn set(&mut self, field: FieldId, expr: stmt::Expr) {
        self.2.set(field, expr);
    }
}
