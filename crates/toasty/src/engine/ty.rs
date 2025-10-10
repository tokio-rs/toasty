use super::Engine;
use toasty_core::{schema::db::Index, stmt};

impl Engine {
    pub(crate) fn infer_ty(&self, stmt: &stmt::Statement, args: &[stmt::Type]) -> stmt::Type {
        stmt::ExprContext::new(&*self.schema).infer_stmt_ty(stmt, args)
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

    pub(crate) fn index_key_record_ty(&self, index: &Index) -> stmt::Type {
        let field_tys = index
            .columns
            .iter()
            .map(|id| self.schema.db.column(id.column).ty.clone())
            .collect();
        stmt::Type::Record(field_tys)
    }

    /// Returns `Type::List(Type::Record(field_tys))` where each `field_ty` is
    /// inferred from the corresponding expression in `record`.
    pub(crate) fn infer_record_list_ty<'a, T>(&self, cx: &stmt::Statement, record: T) -> stmt::Type
    where
        T: IntoIterator<Item = &'a stmt::ExprReference>,
    {
        let cx = self.expr_cx_for(cx);
        let field_tys = record
            .into_iter()
            .map(|expr_reference| cx.infer_expr_reference_ty(expr_reference))
            .collect();
        stmt::Type::list(stmt::Type::Record(field_tys))
    }
}
