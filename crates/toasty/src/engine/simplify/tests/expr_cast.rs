use super::test_schema_with;
use crate::engine::simplify::Simplify;
use toasty_core::stmt::{Expr, ExprCast, ExprReference, Type};

#[test]
fn redundant_cast_on_field_eliminated() {
    use crate as toasty;
    use crate::schema::Register;

    #[allow(dead_code)]
    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: String,
        name: String,
    }

    let schema = test_schema_with(&[User::schema()]);
    let model = schema.app.model(User::id());
    let simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);
    let simplify = simplify.scope(model.as_root_unwrap());

    // `cast(name_field, String) → name_field`
    // The `name` field (index 1) is already String, so the cast is a no-op.
    let field_ref = Expr::Reference(ExprReference::Field {
        nesting: 0,
        index: 1,
    });
    let mut expr = ExprCast {
        expr: Box::new(field_ref.clone()),
        ty: Type::String,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    assert_eq!(result, Some(field_ref));
}

#[test]
fn non_redundant_cast_on_field_kept() {
    use crate as toasty;
    use crate::schema::Register;

    #[allow(dead_code)]
    #[derive(toasty::Model)]
    struct User {
        #[key]
        id: String,
        name: String,
    }

    let schema = test_schema_with(&[User::schema()]);
    let model = schema.app.model(User::id());
    let simplify = Simplify::new(&schema, &toasty_core::driver::Capability::SQLITE);
    let simplify = simplify.scope(model.as_root_unwrap());

    // `cast(name_field, I64)` — String field cast to I64, not redundant.
    let mut expr = ExprCast {
        expr: Box::new(Expr::Reference(ExprReference::Field {
            nesting: 0,
            index: 1,
        })),
        ty: Type::I64,
    };
    let result = simplify.simplify_expr_cast(&mut expr);

    assert!(result.is_none());
}
