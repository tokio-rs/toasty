use super::Engine;
use toasty_core::{driver::operation::TypedValue, schema::db::Index, stmt};

impl Engine {
    /// Extract typed bind parameters from a statement, replacing scalar values
    /// with `Expr::Arg(n)` placeholders. The returned `Vec<TypedValue>` is
    /// indexed by the `n` in each placeholder.
    pub(crate) fn extract_params(&self, stmt: &mut stmt::Statement) -> Vec<TypedValue> {
        super::extract_params::extract_params(stmt, &self.schema)
    }

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
