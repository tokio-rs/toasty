use super::Engine;
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
}
