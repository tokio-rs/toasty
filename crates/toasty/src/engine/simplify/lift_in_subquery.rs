use super::Simplify;
use toasty_core::{
    schema::app::{BelongsTo, FieldId, FieldTy, HasOne},
    stmt::{self, Visit},
};

struct LiftBelongsTo<'a> {
    simplify: &'a Simplify<'a>,
    belongs_to: &'a BelongsTo,
    // TODO: switch to bit field set
    fk_field_matches: Vec<bool>,
    fail: bool,
    operands: Vec<stmt::Expr>,
}

impl Simplify<'_> {
    pub(crate) fn lift_in_subquery(
        &mut self,
        expr: &stmt::Expr,
        query: &stmt::Query,
    ) -> Option<stmt::Expr> {
        // The expression is a path expression referencing a relation.
        let field = match expr {
            stmt::Expr::Project(_) => {
                todo!()
            }
            stmt::Expr::Reference(expr_reference @ stmt::ExprReference::Field { .. }) => self
                .cx
                .resolve_expr_reference(expr_reference)
                .expect_field(),
            _ => {
                return None;
            }
        };

        // If the field is not a belongs_to relation, abort
        let mut maybe_expr = match &field.ty {
            FieldTy::BelongsTo(belongs_to) => self.lift_belongs_to_in_subquery(belongs_to, query),
            FieldTy::HasOne(has_one) => self.lift_has_one_in_subquery(has_one, query),
            _ => {
                return None;
            }
        };

        if let Some(maybe_expr) = &mut maybe_expr {
            stmt::visit_mut::visit_expr_mut(self, maybe_expr);
        }

        maybe_expr
    }

    /// Optimizes queries by lifting BelongsTo relation constraints out of subqueries when possible.
    ///
    /// This is an app-level optimization that operates on the application schema before
    /// statements are lowered to database-specific representations.
    ///
    /// This method analyzes a subquery that filters a related model and determines if the query
    /// can be rewritten to avoid the subquery by directly comparing foreign key fields.
    ///
    /// For example, transforms:
    /// ```sql
    /// -- Original: subquery filtering related records
    /// user_id IN (SELECT id FROM users WHERE name = 'Alice')
    ///
    /// -- Optimized: direct foreign key comparison
    /// user_id = 'Alice_user_id'
    /// ```
    ///
    /// The optimization works by:
    /// 1. Verifying the subquery targets the same model as the BelongsTo relation
    /// 2. Analyzing the subquery's WHERE clause to find constraints on foreign key fields
    /// 3. If all constraints can be lifted, rewriting them as direct field comparisons
    /// 4. If constraints reference non-foreign-key fields, falling back to an IN subquery
    ///
    /// Returns `None` if the subquery cannot be optimized (wrong target model).
    /// Returns `Some(expr)` containing either:
    /// - Direct field comparison expressions (when optimization succeeds)
    /// - An IN subquery expression (when partial optimization is possible)
    ///
    /// Currently only supports single-field foreign keys; composite keys are not yet implemented.
    fn lift_belongs_to_in_subquery(
        &self,
        belongs_to: &BelongsTo,
        query: &stmt::Query,
    ) -> Option<stmt::Expr> {
        if belongs_to.target != query.body.as_select_unwrap().source.model_id_unwrap() {
            return None;
        }

        let select = query.body.as_select_unwrap();

        assert_eq!(
            belongs_to.foreign_key.fields.len(),
            1,
            "TODO: composite keys"
        );

        let mut lift = LiftBelongsTo {
            simplify: &self.scope(&select.source),
            belongs_to,
            fk_field_matches: vec![false; belongs_to.foreign_key.fields.len()],
            operands: vec![],
            fail: false,
        };

        lift.visit_filter(&select.filter);

        if lift.fail {
            let [fk_fields] = &belongs_to.foreign_key.fields[..] else {
                todo!("composite keys")
            };
            let mut subquery = query.clone();

            subquery.body.as_select_mut_unwrap().returning =
                stmt::Returning::Expr(stmt::Expr::ref_self_field(fk_fields.target));

            Some(stmt::Expr::in_subquery(
                stmt::Expr::ref_self_field(fk_fields.source),
                subquery,
            ))
        } else {
            Some(if lift.operands.len() == 1 {
                lift.operands.into_iter().next().unwrap()
            } else {
                stmt::ExprAnd {
                    operands: lift.operands,
                }
                .into()
            })
        }
    }

    /// Rewrite the `HasOne` in subquery expression to reference the foreign key.
    fn lift_has_one_in_subquery(
        &self,
        has_one: &HasOne,
        query: &stmt::Query,
    ) -> Option<stmt::Expr> {
        if has_one.target != query.body.as_select_unwrap().source.model_id_unwrap() {
            return None;
        }

        let pair = has_one.pair(&self.schema().app);

        let expr = match &pair.foreign_key.fields[..] {
            [fk_field] => stmt::Expr::ref_self_field(fk_field.target),
            _ => todo!("composite"),
        };

        let mut subquery = query.clone();

        match &mut subquery.body {
            stmt::ExprSet::Select(subquery) => {
                subquery.returning = stmt::Returning::Expr(match &pair.foreign_key.fields[..] {
                    [fk_field] => stmt::Expr::ref_self_field(fk_field.source),
                    _ => todo!("composite key"),
                });
            }
            _ => todo!(),
        };

        Some(
            stmt::ExprInSubquery {
                expr: Box::new(expr),
                query: Box::new(subquery),
            }
            .into(),
        )
    }
}

impl Visit for LiftBelongsTo<'_> {
    fn visit_expr_binary_op(&mut self, i: &stmt::ExprBinaryOp) {
        match (&*i.lhs, &*i.rhs) {
            (stmt::Expr::Reference(expr_reference), other)
            | (other, stmt::Expr::Reference(expr_reference)) => {
                assert!(i.op.is_eq() || i.op.is_ne());

                if i.op.is_eq() || i.op.is_ne() {
                    let field = self
                        .simplify
                        .cx
                        .resolve_expr_reference(expr_reference)
                        .expect_field();

                    self.lift_fk_constraint(field.id, i.op, other);
                } else {
                    self.fail = true;
                }
            }
            _ => {}
        }
    }
}

impl LiftBelongsTo<'_> {
    fn lift_fk_constraint(&mut self, field: FieldId, op: stmt::BinaryOp, expr: &stmt::Expr) {
        for (i, fk_field) in self.belongs_to.foreign_key.fields.iter().enumerate() {
            if fk_field.target == field {
                if self.fk_field_matches[i] {
                    todo!("not handled");
                }

                self.operands.push(stmt::Expr::binary_op(
                    stmt::Expr::ref_self_field(fk_field.source),
                    op,
                    expr.clone(),
                ));
                self.fk_field_matches[i] = true;

                return;
            }
        }

        self.fail = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as toasty;
    use crate::model::Register;
    use toasty_core::{
        driver::Capability,
        schema::{app, app::ModelId, Builder},
        stmt::{Expr, ExprBinaryOp, Query, Value},
    };

    #[allow(dead_code)]
    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: i64,

        #[has_many]
        posts: toasty::HasMany<Post>,
    }

    #[allow(dead_code)]
    #[derive(toasty::Model)]
    struct Post {
        #[key]
        id: i64,

        #[index]
        user_id: i64,

        #[belongs_to(key = user_id, references = id)]
        user: toasty::BelongsTo<User>,
    }

    /// Schema with `User` and `Post` models in a `HasMany`/`BelongsTo`
    /// relationship.
    struct UserPostSchema {
        schema: toasty_core::Schema,
        user_model: ModelId,
        user_id: FieldId,
        post_model: ModelId,
        post_user: FieldId,
    }

    impl UserPostSchema {
        fn new() -> Self {
            let app_schema = app::Schema::from_macro(&[User::schema(), Post::schema()])
                .expect("schema should build from macro");

            let schema = Builder::new()
                .build(app_schema, &Capability::SQLITE)
                .expect("schema should build");

            let user_model = User::id();
            let post_model = Post::id();

            // Find field IDs by name from the generated schema
            let user_id = schema
                .app
                .model(user_model)
                .fields
                .iter()
                .find(|f| f.name.app_name == "id")
                .unwrap()
                .id;

            let post_user = schema
                .app
                .model(post_model)
                .fields
                .iter()
                .find(|f| f.name.app_name == "user")
                .unwrap()
                .id;

            Self {
                schema,
                user_model,
                user_id,
                post_model,
                post_user,
            }
        }
    }

    #[test]
    fn belongs_to_lifts_fk_constraint_to_direct_eq() {
        let s = UserPostSchema::new();
        let simplify = Simplify::new(&s.schema);

        let post_source: stmt::Source = s.post_model.into();
        let mut scoped_simplify = simplify.scope(&post_source);

        // `lift_in_subquery(user, select(User, eq(id, 42))) → eq(user_id, 42)`
        let expr = Expr::ref_self_field(s.post_user);
        let filter = Expr::eq(
            Expr::ref_self_field(s.user_id),
            Expr::Value(Value::from(42i64)),
        );
        let query = Query::new_select(s.user_model, filter);

        let result = scoped_simplify.lift_in_subquery(&expr, &query);

        assert!(result.is_some());
        let lifted = result.unwrap();
        let Expr::BinaryOp(ExprBinaryOp { op, lhs, rhs }) = lifted else {
            panic!("expected result to be an `Expr::BinaryOp`");
        };
        assert!(op.is_eq());
        assert!(matches!(
            *lhs,
            Expr::Reference(stmt::ExprReference::Field { index: 1, .. })
        ));
        assert!(matches!(*rhs, Expr::Value(Value::I64(42))));
    }
}
