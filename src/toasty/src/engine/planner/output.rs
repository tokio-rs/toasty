use super::*;

impl<'stmt> Planner<'_, 'stmt> {
    pub(crate) fn partition_returning(
        &self,
        stmt: &mut stmt::Returning<'stmt>,
    ) -> eval::Expr<'stmt> {
        use Partition::*;

        let stmt::Returning::Expr(stmt::Expr::Record(stmt_record)) = stmt else {
            todo!("returning={stmt:#?}");
        };

        let mut eval_fields = vec![];
        let mut stmt_fields = vec![];
        let mut identity = true;

        for field in stmt_record.fields.drain(..) {
            match partition_returning(&field, &mut stmt_fields) {
                Stmt => {
                    let i = stmt_fields.len();
                    stmt_fields.push(field);
                    eval_fields.push(eval::Expr::arg_project(0, [i]));
                }
                Eval(eval) => {
                    identity = false;
                    eval_fields.push(eval);
                }
            }
        }

        stmt_record.fields = stmt_fields;

        if identity {
            eval::Expr::arg(0)
        } else {
            eval::Expr::record_from_vec(eval_fields)
        }
    }
}

enum Partition<'stmt> {
    Stmt,
    Eval(eval::Expr<'stmt>),
}

fn partition_returning<'stmt>(
    stmt: &stmt::Expr<'stmt>,
    returning: &mut Vec<stmt::Expr<'stmt>>,
) -> Partition<'stmt> {
    use Partition::*;

    match stmt {
        stmt::Expr::Cast(expr) => match partition_returning(&expr.expr, returning) {
            Stmt => {
                let i = returning.len();
                returning.push((*expr.expr).clone());
                Eval(eval::Expr::cast(
                    eval::Expr::arg_project(0, [i]),
                    expr.ty.clone(),
                ))
            }
            Eval(eval) => Eval(eval::Expr::cast(eval, expr.ty.clone())),
        },
        stmt::Expr::Column(_) => Stmt,
        _ => todo!("stmt={stmt:#?}"),
    }
}
