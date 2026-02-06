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
            todo!("composite primary keys")
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
        stmt::{Expr, ExprBinaryOp, Value},
    };

    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: i64,
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
}
