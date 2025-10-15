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
                    let (pos, inserted) = db.insert_full(*e);

                    if inserted {
                        tys.push(cx.infer_expr_ty(expr, &[]));
                    }

                    // Project field `pos` from Arg(0), which will be the record returned by the DB
                    *expr = stmt::Expr::arg_project(0, [pos]);
                }
                // Subqueries should have been removed at this point
                stmt::Expr::Stmt(_) | stmt::Expr::InSubquery(_) => todo!(),
                _ => {}
            }
        });

        // Wrap types in a Record - the function receives a single Record argument from the DB
        let args = if tys.is_empty() {
            vec![]
        } else {
            vec![stmt::Type::Record(tys)]
        };
        let project = eval::Func::from_stmt(expr.clone(), args);

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

        if returning.as_expr_unwrap().as_record().is_empty() {
            *stmt = None;
        }

        Some(project)
    }
}
