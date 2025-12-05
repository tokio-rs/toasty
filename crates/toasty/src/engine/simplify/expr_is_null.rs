use super::Simplify;
use toasty_core::stmt;

impl Simplify<'_> {
    pub(super) fn simplify_expr_is_null(&self, expr: &mut stmt::ExprIsNull) -> Option<stmt::Expr> {
        match &mut *expr.expr {
            stmt::Expr::Cast(expr_cast) if expr_cast.ty.is_id() => {
                *expr.expr = expr_cast.expr.take();
                None
            }
            stmt::Expr::Reference(f @ stmt::ExprReference::Field { .. }) => {
                let field = self.cx.resolve_expr_reference(f).expect_field();

                if !field.nullable() {
                    // Is null on a non nullable field evaluates to `false`.
                    return Some(stmt::Expr::Value(stmt::Value::Bool(false)));
                }

                None
            }
            stmt::Expr::Value(_) => todo!("expr={expr:#?}"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simplify::test::{test_schema, test_schema_with};
    use toasty_core::schema::app::ModelId;
    use toasty_core::stmt::{
        Expr, ExprArg, ExprCast, ExprIsNull, ExprReference, Type, Value, VisitMut as _,
    };

    #[test]
    fn cast_to_id_unwrapped() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `is_null(cast(arg(0), Id))` → `is_null(arg(0))`
        let mut expr = ExprIsNull {
            expr: Box::new(Expr::Cast(ExprCast {
                expr: Box::new(Expr::arg(0)),
                ty: Type::Id(ModelId(0)),
            })),
        };
        let result = simplify.simplify_expr_is_null(&mut expr);

        assert!(result.is_none());
        assert!(matches!(*expr.expr, Expr::Arg(ExprArg { position: 0 })));
    }

    #[test]
    fn cast_to_non_id_not_simplified() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `is_null(cast(arg(0), String))`, non-Id cast, not simplified
        let mut expr = ExprIsNull {
            expr: Box::new(Expr::Cast(ExprCast {
                expr: Box::new(Expr::arg(0)),
                ty: Type::String,
            })),
        };
        let result = simplify.simplify_expr_is_null(&mut expr);

        assert!(result.is_none());
        assert!(matches!(
            *expr.expr,
            Expr::Cast(ExprCast {
                ty: Type::String,
                ..
            })
        ));
    }

    #[test]
    fn non_cast_expr_not_simplified() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `is_null(arg(0))`, non-cast, not simplified
        let mut expr = ExprIsNull {
            expr: Box::new(Expr::arg(0)),
        };
        let result = simplify.simplify_expr_is_null(&mut expr);

        assert!(result.is_none());
        assert!(matches!(*expr.expr, Expr::Arg(ExprArg { position: 0 })));
    }

    #[test]
    fn not_is_null_cast_to_id_unwrapped() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `not(is_null(cast(arg(0), Id)))` → `not(is_null(arg(0)))`
        let mut expr = Expr::not(Expr::IsNull(ExprIsNull {
            expr: Box::new(Expr::Cast(ExprCast {
                expr: Box::new(Expr::arg(0)),
                ty: Type::Id(ModelId(0)),
            })),
        }));
        simplify.visit_expr_mut(&mut expr);

        if let Expr::Not(not) = &expr {
            if let Expr::IsNull(is_null) = not.expr.as_ref() {
                assert!(matches!(*is_null.expr, Expr::Arg(ExprArg { position: 0 })));
            } else {
                panic!("expected `IsNull` inside `Not` expression");
            }
        } else {
            panic!("expected `Not` expression");
        }
    }

    #[test]
    fn is_null_non_nullable_field() {
        use crate as toasty;
        use crate::Model as _;

        #[allow(dead_code)]
        #[derive(toasty::Model)]
        struct User {
            #[key]
            id: String,
            emailadres: String,
        }

        let schema = test_schema_with(&[User::schema()]);
        let model = schema.app.model(User::id());
        let simplify = Simplify::new(&schema);
        let simplify = simplify.scope(model);

        // `is_null(field)` → `false` (non-nullable field)
        let mut field = ExprIsNull {
            expr: Box::new(Expr::Reference(ExprReference::Field {
                nesting: 0,
                index: 1,
            })),
        };

        let result = simplify.simplify_expr_is_null(&mut field);

        assert!(matches!(result, Some(Expr::Value(Value::Bool(false)))));
    }
}
