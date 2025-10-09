use super::Planner;
use toasty_core::{schema::db::Index, stmt};

impl Planner<'_> {
    pub(crate) fn infer_ty(&self, stmt: &stmt::Statement, args: &[stmt::Type]) -> stmt::Type {
        stmt::ExprContext::new(&*self.engine.schema).infer_stmt_ty(stmt, args)
    }

    pub(crate) fn index_key_ty(&self, index: &Index) -> stmt::Type {
        match &index.columns[..] {
            [id] => self.engine.schema.db.column(id.column).ty.clone(),
            ids => stmt::Type::Record(
                ids.iter()
                    .map(|id| self.engine.schema.db.column(id.column).ty.clone())
                    .collect(),
            ),
        }
    }
}
