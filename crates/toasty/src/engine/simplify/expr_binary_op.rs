use super::Simplify;
use toasty_core::{
    schema::app::FieldTy,
    stmt::{self, Expr},
};

impl Simplify<'_> {
    pub(super) fn simplify_expr_eq_operand(&mut self, operand: &mut stmt::Expr) {
        match operand {
            stmt::Expr::Reference(expr_reference) if expr_reference.is_model() => {
                let model = self
                    .cx
                    .resolve_expr_reference(expr_reference)
                    .expect_model();

                let [pk_field] = &model.primary_key.fields[..] else {
                    todo!("handle composite keys");
                };

                *operand = stmt::Expr::ref_field(expr_reference.nesting(), pk_field);
            }
            stmt::Expr::Reference(expr_reference) if expr_reference.is_field() => {
                let field = self
                    .cx
                    .resolve_expr_reference(expr_reference)
                    .expect_field();

                match &field.ty {
                    FieldTy::Primitive(_) => {}
                    FieldTy::HasMany(_) | FieldTy::HasOne(_) => todo!(),
                    FieldTy::BelongsTo(rel) => {
                        let [fk_field] = &rel.foreign_key.fields[..] else {
                            todo!("handle composite keys");
                        };

                        let stmt::ExprReference::Field { index, .. } = expr_reference else {
                            panic!()
                        };
                        *index = fk_field.source.index;
                    }
                }
            }
            _ => {}
        }
    }

    /// Recursively walk a binary expression in parallel
    pub(super) fn simplify_expr_binary_op(
        &mut self,
        op: stmt::BinaryOp,
        lhs: &mut stmt::Expr,
        rhs: &mut stmt::Expr,
    ) -> Option<stmt::Expr> {
        if op.is_eq() || op.is_ne() {
            self.simplify_expr_eq_operand(lhs);
            self.simplify_expr_eq_operand(rhs);
        }

        match (&mut *lhs, &mut *rhs) {
            (Expr::Cast(cast), Expr::Value(val)) if cast.ty.is_id() => {
                *lhs = cast.expr.take();
                self.uncast_value_id(val);
                None
            }
            (Expr::Value(val), Expr::Cast(cast)) if cast.ty.is_id() => {
                *rhs = cast.expr.take();
                self.uncast_value_id(val);
                None
            }
            (stmt::Expr::Key(_), other) | (other, stmt::Expr::Key(_)) => {
                assert!(op.is_eq());

                // At this point, we must be in a model context, otherwise key
                // expressions don't make sense.
                let Some(model) = self.cx.target_as_model() else {
                    todo!();
                };

                Some(self.rewrite_root_path_expr(model, other.take()))
            }
            _ => {
                // For now, just make sure there are no relations in the expression
                stmt::visit::for_each_expr(lhs, |expr| {
                    if let stmt::Expr::Project(_) = expr {
                        todo!()
                    }
                });

                stmt::visit::for_each_expr(rhs, |expr| {
                    if let stmt::Expr::Project(_) = expr {
                        todo!()
                    }
                });

                None
            }
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
                Field, FieldId, FieldName, FieldPrimitive, Index, IndexField, IndexId, Model,
                ModelId, PrimaryKey,
            },
            db::{IndexOp, IndexScope},
            Builder, Name,
        },
        stmt::{BinaryOp, ExprCast, Id, Type, Value},
    };

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
                    ty: Type::String,
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
    fn cast_id_on_lhs_unwrapped() {
        let (schema, _) = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `eq(cast(arg(0), Id), Id("abc")) → eq(arg(0), "abc")`
        let mut lhs = Expr::Cast(ExprCast {
            expr: Box::new(Expr::arg(0)),
            ty: Type::Id(ModelId(0)),
        });
        let mut rhs = Expr::Value(Value::Id(Id::from_string(ModelId(0), "abc".to_string())));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(result.is_none());
        assert!(matches!(lhs, Expr::Arg(_)));
        assert!(matches!(rhs, Expr::Value(Value::String(s)) if s == "abc"));
    }

    #[test]
    fn cast_id_on_rhs_unwrapped() {
        let (schema, _model) = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `eq(Id("xyz"), cast(arg(0), Id)) → eq("xyz", arg(0))`
        let mut lhs = Expr::Value(Value::Id(Id::from_string(ModelId(0), "xyz".to_string())));
        let mut rhs = Expr::Cast(ExprCast {
            expr: Box::new(Expr::arg(0)),
            ty: Type::Id(ModelId(0)),
        });

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(result.is_none());
        assert!(matches!(lhs, Expr::Value(Value::String(s)) if s == "xyz"));
        assert!(matches!(rhs, Expr::Arg(_)));
    }

    #[test]
    fn non_id_cast_not_unwrapped() {
        let (schema, _model) = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `eq(cast(arg(0), String), "test")`, non-Id cast is not unwrapped
        let mut lhs = Expr::Cast(ExprCast {
            expr: Box::new(Expr::arg(0)),
            ty: Type::String,
        });
        let mut rhs = Expr::Value(Value::from("test"));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(result.is_none());
        assert!(matches!(lhs, Expr::Cast(_)));
    }

    #[test]
    fn plain_values_unchanged() {
        let (schema, _model) = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `eq(1, 2)`, plain values are unchanged
        let mut lhs = Expr::Value(Value::from(1i64));
        let mut rhs = Expr::Value(Value::from(2i64));

        let result = simplify.simplify_expr_binary_op(BinaryOp::Eq, &mut lhs, &mut rhs);

        assert!(result.is_none());
        assert!(matches!(lhs, Expr::Value(Value::I64(1))));
        assert!(matches!(rhs, Expr::Value(Value::I64(2))));
    }
}
