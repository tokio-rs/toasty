use super::{eval, Planner};
use indexmap::IndexSet;
use toasty_core::stmt::{self, visit_mut, ExprContext};

/*
struct Partitioner {
    // Returning statement expressions. The returning statement will be a
    // record, these are the field expressions.
    stmt: Vec<stmt::Expr>,

    // Type of each field expression
    ty: Vec<stmt::Type>,
}

#[derive(Debug)]
enum Partition {
    /// The expr *must* be evaluated by Toasty
    Eval(stmt::Expr),

    /// The expr *must* be evaluated by the database
    Stmt,

    /// The expr *can* be evaluated by either Toasty or the database
    ConstStmt,
}
*/

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
                        tys.push(cx.infer_expr_ty(&expr, &[]));
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

/*
impl Partitioner {
    fn partition_expr(&mut self, cx: &ExprContext<'_>, stmt: &stmt::Expr) -> Partition {
        use Partition::*;

        match stmt {
            stmt::Expr::Cast(expr) => match self.partition_expr(cx, &expr.expr) {
                Stmt => {
                    let ty = cx.infer_expr_ty(&expr.expr, &[]);
                    let arg = self.push_stmt_field((*expr.expr).clone(), ty);
                    Eval(stmt::Expr::cast(arg, expr.ty.clone()))
                }
                ConstStmt => Eval(stmt::Expr::cast((*expr.expr).clone(), expr.ty.clone())),
                Eval(eval) => Eval(stmt::Expr::cast(eval, expr.ty.clone())),
            },
            stmt::Expr::Reference(stmt::ExprReference::Column { .. }) => Stmt,
            stmt::Expr::Project(expr) => match self.partition_expr(cx, &expr.base) {
                Stmt => {
                    let ty = cx.infer_expr_ty(&expr.base, &[]);
                    let arg = self.push_stmt_field((*expr.base).clone(), ty);
                    Eval(stmt::Expr::project(arg, expr.projection.clone()))
                }
                ConstStmt => todo!(),
                Eval(eval) => Eval(stmt::Expr::project(eval, expr.projection.clone())),
            },
            stmt::Expr::Record(expr) => {
                let field_partition_res: Vec<_> = expr
                    .fields
                    .iter()
                    .map(|field| self.partition_expr(cx, field))
                    .collect();

                if field_partition_res.iter().all(|res| res.is_stmt()) {
                    Stmt
                } else if field_partition_res.iter().all(|res| res.is_const_stmt()) {
                    ConstStmt
                } else {
                    let mut fields = vec![];

                    for res in field_partition_res.into_iter() {
                        match res {
                            Eval(eval) => {
                                fields.push(eval);
                            }
                            _ => todo!("res={res:#?}"),
                        }
                    }

                    Eval(stmt::Expr::record_from_vec(fields))
                }
            }
            stmt::Expr::Value(_) => ConstStmt,
            stmt::Expr::DecodeEnum(expr, ty, variant) => match self.partition_expr(cx, expr) {
                Stmt => {
                    let base_ty = cx.infer_expr_ty(expr, &[]);
                    let base = self.push_stmt_field((**expr).clone(), base_ty);
                    Eval(stmt::Expr::DecodeEnum(Box::new(base), ty.clone(), *variant))
                }
                ConstStmt => todo!(),
                Eval(eval) => Eval(stmt::Expr::DecodeEnum(Box::new(eval), ty.clone(), *variant)),
            },
            _ => todo!("stmt={stmt:#?}"),
        }
    }

    fn push_stmt_field(&mut self, expr: stmt::Expr, ty: stmt::Type) -> stmt::Expr {
        // Record expressions must be flattened
        if let stmt::Expr::Record(expr_record) = expr {
            let stmt::Type::Record(field_tys) = ty else {
                todo!()
            };

            let fields: Vec<_> = expr_record
                .fields
                .into_iter()
                .zip(field_tys)
                .map(|(field, ty)| self.push_stmt_field(field, ty))
                .collect();

            stmt::Expr::record_from_vec(fields)
        } else {
            assert_eq!(self.stmt.len(), self.ty.len());
            let i = self.stmt.len();
            self.stmt.push(expr);
            self.ty.push(ty);
            stmt::Expr::arg_project(0, [i])
        }
    }
}

impl Partition {
    fn is_stmt(&self) -> bool {
        matches!(self, Self::Stmt)
    }

    fn is_const_stmt(&self) -> bool {
        matches!(self, Self::ConstStmt)
    }
}
*/
