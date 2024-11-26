use super::*;

pub(crate) enum PartitionedReturning {
    /// How to project values returned by the database statement
    Expr(eval::Expr),

    /// The statement returns a constant value.
    Value(stmt::Value),
}

impl Planner<'_> {
    pub(crate) fn partition_returning(&self, stmt: &mut stmt::Returning) -> eval::Expr {
        use Partition::*;

        match stmt {
            stmt::Returning::Expr(stmt::Expr::Record(expr_record)) => {
                // returning an expression record is special-cased because it
                // might be able to be passed through to the database as an
                // identity projection.
                self.partition_returning_expr_record(expr_record)
            }
            stmt::Returning::Expr(expr) => self.partition_returning_expr(expr),
            _ => todo!("returning={stmt:#?}"),
        }
    }

    fn partition_returning_expr(&self, stmt: &mut stmt::Expr) -> eval::Expr {
        let mut stmt_fields = vec![];

        match partition_returning(stmt, &mut stmt_fields) {
            Partition::Stmt => {
                todo!()
            }
            Partition::Eval(eval) => {
                *stmt = stmt::Expr::record_from_vec(stmt_fields);
                eval
            }
        }
    }

    fn partition_returning_expr_record(&self, stmt_record: &mut stmt::ExprRecord) -> eval::Expr {
        use Partition::*;

        let mut eval_fields = vec![];
        let mut stmt_fields = vec![];
        let mut identity = true;

        for field in stmt_record.fields.drain(..) {
            if let stmt::Expr::Value(value) = field {
                identity = false;
                eval_fields.push(value.into());
            } else {
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
        }

        stmt_record.fields = stmt_fields;

        if identity {
            eval::Expr::arg(0)
        } else {
            eval::Expr::record_from_vec(eval_fields)
        }
    }

    pub fn partition_maybe_returning(
        &self,
        stmt: &mut Option<stmt::Returning>,
    ) -> Option<eval::Expr> {
        let Some(returning) = stmt else { return None };
        let project = self.partition_returning(returning);

        if returning.as_expr().as_record().is_empty() {
            *stmt = None;
        }

        Some(project)
    }
}

enum Partition {
    Stmt,
    Eval(eval::Expr),
}

fn partition_returning(stmt: &stmt::Expr, returning: &mut Vec<stmt::Expr>) -> Partition {
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
        stmt::Expr::Column(_) | stmt::Expr::Value(_) => Stmt,
        stmt::Expr::Project(expr) => match partition_returning(&expr.base, returning) {
            Stmt => {
                let i = returning.len();
                returning.push((*expr.base).clone());
                let base = eval::Expr::arg_project(0, [i]);
                Eval(eval::Expr::project(base, expr.projection.clone()))
            }
            Eval(eval) => Eval(eval::Expr::project(eval, expr.projection.clone())),
        },
        stmt::Expr::DecodeEnum(expr, ty, ..) => match partition_returning(expr, returning) {
            Stmt => {
                let i = returning.len();
                returning.push((**expr).clone());
                let base = eval::Expr::arg_project(0, [i]);
                Eval(eval::Expr::DecodeEnum(Box::new(base), ty.clone()))
            }
            Eval(eval) => Eval(eval::Expr::DecodeEnum(Box::new(eval), ty.clone())),
        },
        _ => todo!("stmt={stmt:#?}"),
    }
}
