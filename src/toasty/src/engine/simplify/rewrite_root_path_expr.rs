use super::*;
use app::Model;

impl Simplify<'_> {
    /// Rewrites expressions where one half is a path referencing `self`. In
    /// this case, the expression can be rewritten to be an expression on the
    /// primary key.
    ///
    /// The caller must ensure it is an `eq` operation
    pub(super) fn rewrite_root_path_expr(&mut self, model: &Model, val: stmt::Expr) -> stmt::Expr {
        if let [field] = &model.primary_key.fields[..] {
            stmt::Expr::eq(*field, val)
        } else {
            todo!("composite primary keys")
        }
    }
}
