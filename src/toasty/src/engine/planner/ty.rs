use super::*;

impl Planner<'_> {
    /// Infer the type of an expression
    pub(crate) fn infer_expr_ty(&self, expr: &stmt::Expr, args: &[stmt::Type]) -> stmt::Type {
        crate::engine::ty::infer_expr_ty(expr, args, self.schema)
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
