use toasty_core::stmt::{self, Project};

use super::Simplify;

impl Simplify<'_> {
    pub(super) fn simplify_expr_project(
        &mut self,
        expr: &mut stmt::ExprProject,
    ) -> Option<stmt::Expr> {
        // Constant evaluation: if the base is an Expr::Value, we can evaluate
        // the projection at compile time.
        //
        // Examples:
        //   - `project(Value::I64(42), [])` → `Value::I64(42)`
        //   - `project(Value::Record([1, 2, 3]), [0])` → `Value::I64(1)`
        //
        // Note: This only handles Expr::Value, not Expr::Record or Expr::List.
        // Those variants represent expressions that will be evaluated later,
        // not constant values that can be folded now.
        if let stmt::Expr::Value(value) = &*expr.base {
            // Use the value's project method to follow the projection path
            if let Some(result) = value.project(&expr.projection) {
                return Some(result);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simplify::test::test_schema;
    use toasty_core::stmt::{Expr, Projection, Value};

    #[test]
    fn project_non_constant_not_simplified() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `project(arg(0), [0])` is not simplified (non-constant base)
        let mut expr = stmt::ExprProject {
            base: Box::new(Expr::arg(0)),
            projection: Projection::from(0),
        };

        let result = simplify.simplify_expr_project(&mut expr);

        assert!(result.is_none());
    }

    #[test]
    fn project_identity_path() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `project(42, [])` → `42` (identity projection)
        let mut expr = stmt::ExprProject {
            base: Box::new(Expr::from(42i64)),
            projection: Projection::identity(),
        };

        let result = simplify.simplify_expr_project(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::I64(42)))));
    }
}
