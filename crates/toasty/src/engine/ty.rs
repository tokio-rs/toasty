use super::{Engine, SelectItem};
use toasty_core::{schema::db::Index, stmt};

impl Engine {
    pub(crate) fn infer_ty(&self, stmt: &stmt::Statement, args: &[stmt::Type]) -> stmt::Type {
        stmt::ExprContext::new(&*self.schema).infer_stmt_ty(stmt, args)
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
    /// inferred from the corresponding select item in `items`.
    pub(crate) fn infer_record_list_ty<'a, T>(&self, cx: &stmt::Statement, items: T) -> stmt::Type
    where
        T: IntoIterator<Item = &'a SelectItem>,
    {
        let cx = self.expr_cx_for(cx);
        let field_tys = items
            .into_iter()
            .map(|item| match item {
                SelectItem::ExprReference(expr_reference) => {
                    cx.infer_expr_reference_ty(expr_reference)
                }
                SelectItem::CountStar => stmt::Type::U64,
            })
            .collect();
        stmt::Type::list(stmt::Type::Record(field_tys))
    }
}
