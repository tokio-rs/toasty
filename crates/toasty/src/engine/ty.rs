use super::Engine;
use toasty_core::{driver::operation::TypedValue, schema::db::Index, stmt};

impl Engine {
    /// Lower a driver-bound statement's `#[document]` path reads into their
    /// driver-consumable shape: positional projections become
    /// `FuncJsonExtract` name paths. Document *values* need no boundary step —
    /// the mapping's casts converted them during lowering/simplification.
    /// Runs for every backend, immediately before the statement crosses to
    /// the driver — [`extract_params`](Self::extract_params) (SQL only)
    /// expects it to have run.
    pub(crate) fn lower_document_paths(&self, stmt: &mut stmt::Statement) {
        super::document::lower_paths(&self.schema, self.capability(), stmt);
    }

    /// Extract typed bind parameters from a statement, replacing scalar values
    /// with `Expr::Arg(n)` placeholders. The returned `Vec<TypedValue>` is
    /// indexed by the `n` in each placeholder. SQL drivers only;
    /// [`lower_document_paths`](Self::lower_document_paths) must have run on
    /// the statement first.
    pub(crate) fn extract_params(&self, stmt: &mut stmt::Statement) -> Vec<TypedValue> {
        super::bind::run(stmt, &self.schema.db, self.capability())
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
