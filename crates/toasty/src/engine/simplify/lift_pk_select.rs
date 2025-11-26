use toasty_core::{schema::app::FieldId, stmt};

use crate::engine::simplify::Simplify;

impl Simplify<'_> {
    /// Extracts the constant value from a simple subquery that filters on a key field.
    ///
    /// This helper method analyzes subqueries like `SELECT id FROM users WHERE id = 123`
    /// and extracts the constant value `123` if the pattern matches. The caller uses this
    /// extracted value to eliminate the subquery entirely. Primarily used during belongs-to
    /// relationship planning to extract foreign key values.
    ///
    /// Example usage by caller:
    /// ```sql
    /// -- Subquery analyzed by this method
    /// (SELECT id FROM users WHERE id = 123)
    ///
    /// -- If this method returns Some(123), caller replaces subquery with:
    /// 123
    /// ```
    ///
    /// Returns `None` if the subquery pattern doesn't match (e.g., complex filters,
    /// composite keys, non-equality operators).
    pub(crate) fn extract_key_value(
        &mut self,
        key: &[FieldId],
        stmt: &stmt::Query,
    ) -> Option<stmt::Expr> {
        let cx = self.cx.scope(stmt);

        let stmt::ExprSet::Select(select) = &stmt.body else {
            return None;
        };

        match select.filter.as_expr() {
            stmt::Expr::BinaryOp(expr_binary_op) => {
                if !expr_binary_op.op.is_eq() {
                    return None;
                }

                let [key_field] = key else {
                    return None;
                };

                match (&*expr_binary_op.lhs, &*expr_binary_op.rhs) {
                    (stmt::Expr::Reference(_), stmt::Expr::Reference(_)) => todo!("stmt={stmt:#?}"),
                    (stmt::Expr::Reference(expr_ref), other)
                    | (other, stmt::Expr::Reference(expr_ref)) => {
                        let field_ref = cx.resolve_expr_reference(expr_ref).expect_field();

                        if *key_field == field_ref.id {
                            if let stmt::Expr::Value(value) = other {
                                Some(value.clone().into())
                            } else {
                                todo!()
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            stmt::Expr::And(_) => {
                todo!("either support PKs or check each op for the key");
            }
            _ => None,
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
                Field, FieldName, FieldPrimitive, FieldTy, Index, IndexField, IndexId, Model,
                ModelId, PrimaryKey,
            },
            db::{IndexOp, IndexScope},
            Builder, Name,
        },
        stmt::{Expr, Query, Type, Value, Values},
    };

    /// Creates a schema with a single `User` model containing an `id` primary
    /// key field.
    fn test_schema() -> (toasty_core::Schema, FieldId) {
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
        app_schema.models.insert(model_id, model);

        let schema = Builder::new()
            .build(app_schema, &Capability::SQLITE)
            .expect("schema should build");

        (schema, field_id)
    }

    #[test]
    fn extracts_value_from_key_eq_filter() {
        let (schema, field_id) = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `extract_key_value(select(eq(id, 42))) → 42`
        let filter = Expr::eq(
            Expr::ref_self_field(field_id),
            Expr::Value(Value::from(42i64)),
        );
        let query = Query::new_select(ModelId(0), filter);
        let result = simplify.extract_key_value(&[field_id], &query);

        assert!(result.is_some());
        assert!(matches!(result.unwrap(), Expr::Value(Value::I64(42))));
    }

    #[test]
    fn extracts_value_with_reversed_operands() {
        let (schema, field_id) = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `extract_key_value(select(eq(99, id))) → 99`
        let filter = Expr::eq(
            Expr::Value(Value::from(99i64)),
            Expr::ref_self_field(field_id),
        );
        let query = Query::new_select(ModelId(0), filter);
        let result = simplify.extract_key_value(&[field_id], &query);

        assert!(result.is_some());
        assert!(matches!(result.unwrap(), Expr::Value(Value::I64(99))));
    }

    #[test]
    fn values_query_returns_none() {
        let (schema, field_id) = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `extract_key_value(values()) → None`
        let query = Query::values(Values::default());
        let result = simplify.extract_key_value(&[field_id], &query);

        assert!(result.is_none());
    }

    #[test]
    fn composite_key_returns_none() {
        let (schema, field_id) = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `extract_key_value([field1, field2], ...) → None` (composite keys not supported)

        let filter = Expr::eq(
            Expr::ref_self_field(field_id),
            Expr::Value(Value::from(42i64)),
        );
        let query = Query::new_select(ModelId(0), filter);
        let field_id2 = FieldId {
            model: ModelId(0),
            index: 1,
        };
        let result = simplify.extract_key_value(&[field_id, field_id2], &query);

        assert!(result.is_none());
    }
}
