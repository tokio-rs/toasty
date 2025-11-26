use super::Simplify;
use toasty_core::{schema::app::Model, stmt};

impl Simplify<'_> {
    /// Rewrites expressions where one half is a path referencing `self`. In
    /// this case, the expression can be rewritten to be an expression on the
    /// primary key.
    ///
    /// The caller must ensure it is an `eq` operation
    pub(super) fn rewrite_root_path_expr(&mut self, model: &Model, val: stmt::Expr) -> stmt::Expr {
        if let [field] = &model.primary_key.fields[..] {
            stmt::Expr::eq(stmt::Expr::ref_self_field(field), val)
        } else {
            todo!("composite primary keys")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toasty_core::{
        driver::Capability,
        schema::{
            app::{
                Field, FieldId, FieldName, FieldPrimitive, FieldTy, Index, IndexField, IndexId,
                ModelId, PrimaryKey,
            },
            db::{IndexOp, IndexScope},
            Builder, Name,
        },
        stmt::{Expr, ExprBinaryOp, Type, Value},
    };

    /// Creates a schema with a single `User` model containing an `id` primary
    /// key field.
    fn test_schema() -> (toasty_core::Schema, Model) {
        let model_id = ModelId(0);
        let field_id = FieldId {
            model: model_id,
            index: 0,
        };
        let index_id = IndexId {
            model: model_id,
            index: 0,
        };

        let model = Model {
            id: model_id,
            name: Name::new("User"),
            fields: vec![Field {
                id: field_id,
                name: FieldName {
                    app_name: "id".to_string(),
                    storage_name: None,
                },
                ty: FieldTy::Primitive(FieldPrimitive {
                    ty: Type::I64,
                    storage_ty: None,
                }),
                nullable: false,
                primary_key: true,
                auto: None,
                constraints: vec![],
            }],
            primary_key: PrimaryKey {
                fields: vec![field_id],
                index: index_id,
            },
            indices: vec![Index {
                id: index_id,
                fields: vec![IndexField {
                    field: field_id,
                    op: IndexOp::Eq,
                    scope: IndexScope::Local,
                }],
                unique: true,
                primary_key: true,
            }],
            table_name: None,
        };

        let mut app_schema = toasty_core::schema::app::Schema::default();
        app_schema.models.insert(model_id, model.clone());

        let schema = Builder::new()
            .build(app_schema, &Capability::SQLITE)
            .expect("schema should build");

        (schema, model)
    }

    #[test]
    fn single_pk_field_becomes_eq_expr() {
        let (schema, model) = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `rewrite_root_path_expr(model, 42) → eq(ref(pk), 42)`
        let val = Expr::Value(Value::from(42i64));
        let result = simplify.rewrite_root_path_expr(&model, val);

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
