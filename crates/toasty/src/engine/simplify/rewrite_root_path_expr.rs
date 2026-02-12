use super::Simplify;
use toasty_core::{schema::app::Model, stmt};

impl Simplify<'_> {
    /// Rewrites expressions where one half is a path referencing `self`. In
    /// this case, the expression can be rewritten to be an expression on the
    /// primary key.
    ///
    /// The caller must ensure it is an `eq` operation
    pub(super) fn rewrite_root_path_expr(&mut self, model: &Model, val: stmt::Expr) -> stmt::Expr {
        let primary_key = model
            .primary_key()
            .expect("root path expr rewrite requires root model with primary key");

        if let [field] = &primary_key.fields[..] {
            stmt::Expr::eq(stmt::Expr::ref_self_field(field), val)
        } else {
            let comparisons: Vec<_> = primary_key
                .fields
                .iter()
                .enumerate()
                .map(|(i, field)| {
                    stmt::Expr::eq(
                        stmt::Expr::ref_self_field(field),
                        stmt::Expr::project(val.clone(), [i]),
                    )
                })
                .collect();
            stmt::Expr::and_from_vec(comparisons)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as toasty;
    use crate::model::Register;
    use toasty_core::{
        driver::Capability,
        schema::{app, Builder},
        stmt::{Expr, ExprAnd, ExprBinaryOp, ExprProject, Value},
    };

    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: i64,
    }

    #[derive(toasty::Model)]
    struct Composite {
        #[key]
        one: String,

        #[key]
        two: String,
    }

    /// Creates a schema with a single `User` model containing an `id` primary
    /// key field.
    fn test_schema() -> toasty_core::Schema {
        let app_schema =
            app::Schema::from_macro(&[User::schema()]).expect("schema should build from macro");

        Builder::new()
            .build(app_schema, &Capability::SQLITE)
            .expect("schema should build")
    }

    fn composite_schema() -> toasty_core::Schema {
        let app_schema = app::Schema::from_macro(&[Composite::schema()])
            .expect("schema should build from macro");

        Builder::new()
            .build(app_schema, &Capability::SQLITE)
            .expect("schema should build")
    }

    #[test]
    fn single_pk_field_becomes_eq_expr() {
        let schema = test_schema();
        let model = schema.app.model(User::id());
        let mut simplify = Simplify::new(&schema);

        // `rewrite_root_path_expr(model, 42) → eq(ref(pk), 42)`
        let val = Expr::Value(Value::from(42i64));
        let result = simplify.rewrite_root_path_expr(model, val);

        let Expr::BinaryOp(ExprBinaryOp { op, lhs, rhs }) = result else {
            panic!("expected result to be an `Expr::BinaryOp`");
        };
        assert!(op.is_eq());
        assert!(matches!(
            *lhs,
            Expr::Reference(stmt::ExprReference::Field { index: 0, .. })
        ));
        assert!(matches!(*rhs, Expr::Value(Value::I64(42))));
    }

    #[test]
    fn composite_pk_becomes_and_of_eq_exprs() {
        let schema = composite_schema();
        let model = schema.app.model(Composite::id());
        let mut simplify = Simplify::new(&schema);

        // For composite keys, `rewrite_root_path_expr(model, record("a", "b"))` should
        // produce `eq(ref(field0), project(val, 0)) AND eq(ref(field1), project(val, 1))`
        let val = Expr::record([
            Expr::Value(Value::from("a")),
            Expr::Value(Value::from("b")),
        ]);
        let result = simplify.rewrite_root_path_expr(model, val);

        let Expr::And(ExprAnd { operands }) = result else {
            panic!("expected And expression, got {result:#?}");
        };
        assert_eq!(operands.len(), 2);

        // First operand: eq(ref(field0), project(val, 0))
        let Expr::BinaryOp(ExprBinaryOp { op, lhs, rhs }) = &operands[0] else {
            panic!("expected BinaryOp");
        };
        assert!(op.is_eq());
        assert!(matches!(
            lhs.as_ref(),
            Expr::Reference(stmt::ExprReference::Field { index: 0, .. })
        ));
        assert!(matches!(rhs.as_ref(), Expr::Project(ExprProject { .. })));

        // Second operand: eq(ref(field1), project(val, 1))
        let Expr::BinaryOp(ExprBinaryOp { op, lhs, rhs }) = &operands[1] else {
            panic!("expected BinaryOp");
        };
        assert!(op.is_eq());
        assert!(matches!(
            lhs.as_ref(),
            Expr::Reference(stmt::ExprReference::Field { index: 1, .. })
        ));
        assert!(matches!(rhs.as_ref(), Expr::Project(ExprProject { .. })));
    }
}
