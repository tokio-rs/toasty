use super::{eval, Planner};
use indexmap::IndexSet;
use toasty_core::stmt::{self, visit_mut, ExprContext};

impl Planner<'_> {
    /// Partition a returning statement between what can be handled by the
    /// target database and what Toasty handles in-memory.
    pub(crate) fn partition_returning(
        &self,
        cx: &ExprContext<'_>,
        stmt: &mut stmt::Returning,
    ) -> eval::Func {
        let mut db = IndexSet::new();
        let mut tys = vec![];

        let stmt::Returning::Expr(expr) = stmt else {
            todo!()
        };
        visit_mut::for_each_expr_mut(expr, |expr| {
            match expr {
                stmt::Expr::Reference(e) => {
                    // Track the needed reference and replace the expression with an argument that will pull from the position.
                    let (pos, inserted) = db.insert_full(e.clone());

                    if inserted {
                        tys.push(cx.infer_expr_ty(expr, &[]));
                    }

                    *expr = stmt::Expr::arg(pos);
                }
                // Subqueries should have been removed at this point
                stmt::Expr::Stmt(_) | stmt::Expr::InSubquery(_) => todo!(),
                _ => {}
            }
        });

        let project = eval::Func::from_stmt(expr.clone(), tys);

        *stmt = stmt::Returning::from_expr_iter(db.iter().map(stmt::Expr::from));

        project
    }

    pub fn partition_maybe_returning(
        &self,
        cx: &ExprContext<'_>,
        stmt: &mut Option<stmt::Returning>,
    ) -> Option<eval::Func> {
        let Some(returning) = stmt else { return None };
        let project = self.partition_returning(cx, returning);

        if returning.as_expr().as_record().is_empty() {
            *stmt = None;
        }

        Some(project)
    }
}
