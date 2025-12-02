use super::Simplify;
use toasty_core::stmt;

impl Simplify<'_> {
    pub(super) fn simplify_expr_is_null(&self, expr: &mut stmt::ExprIsNull) -> Option<stmt::Expr> {
        match &mut *expr.expr {
            stmt::Expr::Cast(expr_cast) if expr_cast.ty.is_id() => {
                *expr.expr = expr_cast.expr.take();
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
    use crate::engine::simplify::test::test_schema;
    use toasty_core::schema::app::ModelId;
    use toasty_core::stmt::{Expr, ExprArg, ExprCast, ExprIsNull, Type};

    #[test]
    fn cast_to_id_unwrapped() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `is_null(cast(arg(0), Id)) → is_null(arg(0))`
        let mut expr = ExprIsNull {
            expr: Box::new(Expr::Cast(ExprCast {
                expr: Box::new(Expr::arg(0)),
                ty: Type::Id(ModelId(0)),
            })),
            negate: false,
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
            negate: false,
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
            negate: false,
        };
        let result = simplify.simplify_expr_is_null(&mut expr);

        assert!(result.is_none());
        assert!(matches!(*expr.expr, Expr::Arg(ExprArg { position: 0 })));
    }

    #[test]
    fn negated_cast_to_id_unwrapped() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `is_not_null(cast(arg(0), Id)) → is_not_null(arg(0))`
        let mut expr = ExprIsNull {
            expr: Box::new(Expr::Cast(ExprCast {
                expr: Box::new(Expr::arg(0)),
                ty: Type::Id(ModelId(0)),
            })),
            negate: true,
        };
        let result = simplify.simplify_expr_is_null(&mut expr);

        assert!(result.is_none());
        assert!(matches!(*expr.expr, Expr::Arg(ExprArg { position: 0 })));
        assert!(expr.negate);
    }
}
