use super::*;

impl Planner<'_> {
    /// Infer the type of an expression
    pub(crate) fn infer_expr_ty(&self, expr: &stmt::Expr, args: &[stmt::Type]) -> stmt::Type {
        crate::engine::ty::infer_expr_ty(expr, args, self.schema)
    }

    /// Infer the type of a value
    pub(crate) fn infer_value_ty(&self, value: &stmt::Value) -> stmt::Type {
        crate::engine::ty::infer_value_ty(value)
    }

    /// The return type of a model record. This is a record type with the fields
    /// used to instantiate models.
    pub(crate) fn model_record_ty(&self, model: &Model) -> stmt::Type {
        stmt::Type::Record(
            model
                .fields
                .iter()
                .map(|field| field.expr_ty().clone())
                .collect(),
        )
    }

    pub(crate) fn index_key_ty(&self, index: &Index) -> stmt::Type {
        match &index.columns[..] {
            [id] => self.schema.column(id).ty.clone(),
            ids => stmt::Type::Record(
                ids.iter()
                    .map(|id| self.schema.column(id).ty.clone())
                    .collect(),
            ),
        }
    }
}
