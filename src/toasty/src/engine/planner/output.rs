use super::*;

impl<'stmt> Planner<'_, 'stmt> {
    /// Partition an output statement into
    pub(crate) fn partition_output(&self, stmt: &mut stmt::Expr<'stmt>) -> eval::Expr<'stmt> {
        let partition = partition_output(stmt, &mut vec![]);

        match partition {
            Partition::Stmt => eval::Expr::arg(0),
            Partition::Eval(expr) => expr,
        }
    }
}

enum Partition<'stmt> {
    Stmt,
    Eval(eval::Expr<'stmt>),
}

fn partition_output<'stmt>(
    stmt: &mut stmt::Expr<'stmt>,
    steps: &mut Vec<stmt::PathStep>,
) -> Partition<'stmt> {
    use Partition::*;

    match stmt {
        stmt::Expr::Cast(expr) => match partition_output(&mut *expr.expr, steps) {
            Stmt => {
                let eval = eval::Expr::cast(
                    eval::Expr::project(eval::Expr::arg(0), steps.clone()),
                    expr.ty.clone(),
                );

                *stmt = expr.expr.take();
                Eval(eval)
            }
            Eval(eval) => Eval(eval::Expr::cast(eval, expr.ty.clone())),
        },
        stmt::Expr::Record(expr) => {
            let mut ret = Stmt;

            for (i, expr) in expr.iter_mut().enumerate() {
                steps.push(i.into());
                let partition = partition_output(expr, steps);
                steps.pop();

                match (&mut ret, partition) {
                    (Stmt, Eval(e)) => {
                        let mut eval = (0..i)
                            .map(|step| {
                                let mut steps = steps.clone();
                                steps.push(step.into());
                                eval::Expr::project(eval::Expr::arg(0), steps)
                            })
                            .collect::<Vec<_>>();

                        let mut steps = steps.clone();
                        steps.push(i.into());
                        eval.push(eval::Expr::project(eval::Expr::arg(0), steps));

                        ret = Eval(eval::Expr::record_from_vec(eval));
                    }
                    (Eval(eval), Stmt) => {
                        let eval::Expr::Record(eval) = eval else {
                            todo!()
                        };
                        let mut steps = steps.clone();
                        steps.push(i.into());
                        eval.fields
                            .push(eval::Expr::project(eval::Expr::arg(0), steps));
                    }
                    (Eval(eval), Eval(expr)) => {
                        let eval::Expr::Record(eval) = eval else {
                            todo!()
                        };
                        eval.fields.push(expr);
                    }
                    (Stmt, Stmt) => {}
                }
            }

            ret
        }
        stmt::Expr::Column(_) => Stmt,
        _ => todo!("stmt={stmt:#?}"),
    }
}

impl Partition<'_> {
    fn is_stmt(&self) -> bool {
        matches!(self, Partition::Stmt)
    }
}
