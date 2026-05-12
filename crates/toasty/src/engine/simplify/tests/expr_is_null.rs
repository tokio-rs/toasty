use super::test_schema_with;
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{Expr, ExprIsNull, ExprReference, Value};

#[test]
fn is_null_non_nullable_field() {
    use crate as toasty;
    use crate::schema::Register;

    #[allow(dead_code)]
    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: String,
        emailadres: String,
    }

    let schema = test_schema_with(&[User::schema()]);
    let model = schema.app.model(User::id());
    let simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);
    let simplify = simplify.scope(model.as_root_unwrap());

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
