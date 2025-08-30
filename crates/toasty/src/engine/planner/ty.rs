use super::Planner;
use toasty_core::{schema::db::Index, stmt};

impl Planner<'_> {
    /// Infer the type of an expression
    pub(crate) fn infer_expr_ty(&self, expr: &stmt::Expr, args: &[stmt::Type]) -> stmt::Type {
        crate::engine::ty::infer_expr_ty(expr, args, self.schema)
    }

    pub(crate) fn index_key_ty(&self, index: &Index) -> stmt::Type {
        match &index.columns[..] {
            [id] => self.schema.db.column(id.column).ty.clone(),
            ids => stmt::Type::Record(
                ids.iter()
                    .map(|id| self.schema.db.column(id.column).ty.clone())
                    .collect(),
            ),
        }
    }

    /// Get the record type for a model (all fields as their primitive types)
    pub(crate) fn infer_model_record_type(
        &self,
        model: &toasty_core::schema::app::Model,
    ) -> stmt::Type {
        let mut field_types = vec![];
        for field in &model.fields {
            field_types.push(field.expr_ty().clone());
        }
        stmt::Type::Record(field_types)
    }
}
