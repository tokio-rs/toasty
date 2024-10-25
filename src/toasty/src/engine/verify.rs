use toasty_core::{
    schema::ModelId,
    stmt::{self, Visit},
};

use super::*;

struct Verify<'stmt> {
    schema: &'stmt Schema,
}

struct VerifyExpr<'stmt> {
    schema: &'stmt Schema,
    model: ModelId,
}

pub(crate) fn apply<'stmt>(schema: &'stmt Schema, stmt: &Statement<'stmt>) {
    Verify { schema }.visit(stmt);
}

impl<'stmt> stmt::Visit<'stmt> for Verify<'stmt> {
    fn visit_stmt_insert(&mut self, i: &stmt::Insert<'stmt>) {
        self.visit_stmt_query(&i.scope);

        VerifyExpr {
            schema: self.schema,
            model: i.scope.body.as_select().source.as_model_id(),
        }
        .visit(&i.values);
    }

    fn visit_stmt_select(&mut self, i: &stmt::Select<'stmt>) {
        VerifyExpr {
            schema: self.schema,
            model: i.source.as_model_id(),
        }
        .verify_filter(&i.filter);
    }

    fn visit_stmt_update(&mut self, i: &stmt::Update<'stmt>) {
        self.visit_stmt_query(&i.selection);

        // Is not an empty update
        assert!(!i.fields.is_empty(), "stmt = {i:#?}");

        // TODO: VERIFY THIS

        // let model = self.schema.model(stmt.selection.source);

        // TODO: verify this better
        // self.verify_model_expr_record(model, &*stmt.values);

        /*

        // Verify the update expression matches the type of the field being
        // updated.
        for (field_path, expr) in stmt.fields.iter().zip(stmt.values.iter()) {
            let field = &model.fields[field_path.as_index()];
            assert!(expr.ty().casts_to(&field.expr_ty()));
        }
        */
    }

    fn visit_expr(&mut self, _i: &stmt::Expr<'stmt>) {
        panic!("should not reach this point")
    }
}

impl<'stmt> VerifyExpr<'stmt> {
    fn verify_filter(&mut self, expr: &stmt::Expr<'stmt>) {
        self.assert_bool_expr(expr);
        self.visit(expr);
    }

    fn assert_bool_expr(&self, expr: &stmt::Expr<'stmt>) {
        use stmt::Expr::*;

        match expr {
            And(_)
            | BinaryOp(_)
            | InList(_)
            | InSubquery(_)
            | Or(_)
            | Value(stmt::Value::Bool(_)) => {}
            expr => panic!("Not a bool? {expr:#?}"),
        }
    }
}

impl<'stmt> stmt::Visit<'stmt> for VerifyExpr<'stmt> {
    fn visit_expr_and(&mut self, i: &stmt::ExprAnd<'stmt>) {
        stmt::visit::visit_expr_and(self, i);

        for expr in &i.operands {
            self.assert_bool_expr(expr);
        }
    }

    fn visit_expr_or(&mut self, i: &stmt::ExprOr<'stmt>) {
        stmt::visit::visit_expr_or(self, i);

        for expr in &i.operands {
            self.assert_bool_expr(expr);
        }
    }

    fn visit_projection(&mut self, i: &stmt::Projection) {
        // The path should resolve. Verifying type is done at a higher level
        let _ = i.resolve_field(self.schema, self.schema.model(self.model));
    }

    fn visit_expr_binary_op(&mut self, i: &stmt::ExprBinaryOp<'stmt>) {
        stmt::visit::visit_expr_binary_op(self, i);

        /*
        if i.op.is_in_set() {
            assert!(!matches!(&*i.rhs, stmt::Expr::Record(_)));
        } else {
            // TODO: Update this verification
            /*
            assert!(
                lhs_ty.casts_to(&rhs_ty),
                "lhs_ty={:#?}; rhs_ty={:#?}",
                lhs_ty,
                rhs_ty
            );
            assert!(lhs_ty.applies_binary_op(i.op));
            */
        }
        */
    }

    fn visit_expr_in_subquery(&mut self, i: &stmt::ExprInSubquery<'stmt>) {
        // stmt::visit::visit_expr_in_subquery(self, i);

        // Visit **only** the subquery expression
        self.visit(&*i.expr);

        // The subquery is verified independently
        Verify {
            schema: self.schema,
        }
        .visit(&*i.query);

        // TODO: update this verification
        /*
        // The subquery should be a list of the same type as expression
        let expr_ty = self.ty(&i.expr);
        // let subquery_ty = i.subquery.ty(self.schema);
        let subquery_ty = todo!();

        assert_eq!(
            expr_ty,
            match subquery_ty {
                stmt::Type::List(item) => *item,
                _ => todo!(),
            }
        );
        */
    }
}
