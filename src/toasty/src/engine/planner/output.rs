use super::*;

struct Partitioner<'a> {
    // Schema we are basing all statements on
    schema: &'a Schema,

    // Returning statement expressions. The returning statement will be a
    // record, these are the field expressions.
    stmt: Vec<stmt::Expr>,

    // Type of each field expression
    ty: Vec<stmt::Type>,
}

#[derive(Debug)]
enum Partition {
    /// The expr can evaluate on the databases as a statement. Includes the
    /// type.
    Stmt(stmt::Type),
    Eval(eval::Expr),
}

impl Planner<'_> {
    /// Partition a returning statement between what can be handled by the
    /// target database and what Toasty handles in-memory.
    pub(crate) fn partition_returning(
        &self,
        stmt: &mut stmt::Returning,
        ty: stmt::Type,
    ) -> eval::Func {
        use Partition::*;

        match stmt {
            stmt::Returning::Expr(stmt::Expr::Record(expr_record)) => {
                // returning an expression record is special-cased because it
                // might be able to be passed through to the database as an
                // identity projection.
                self.partition_returning_expr_record(expr_record, ty)
            }
            stmt::Returning::Expr(expr) => self.partition_returning_expr(expr),
            _ => todo!("returning={stmt:#?}"),
        }
    }

    fn partition_returning_expr(&self, stmt: &mut stmt::Expr) -> eval::Func {
        /*
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
        */
        todo!()
    }

    fn partition_returning_expr_record(
        &self,
        stmt_record: &mut stmt::ExprRecord,
        ty: stmt::Type,
    ) -> eval::Func {
        use Partition::*;

        let mut partitioner = Partitioner {
            schema: self.schema,
            stmt: vec![],
            ty: vec![],
        };

        let mut eval_fields = vec![];
        let mut identity = true;

        for field in stmt_record.fields.drain(..) {
            if let stmt::Expr::Value(value) = field {
                identity = false;
                eval_fields.push(eval::Expr::Value(value));
            } else {
                match partitioner.partition_expr(&field) {
                    Stmt(ty) => {
                        let arg = partitioner.push_stmt_field(field, ty);
                        eval_fields.push(arg);
                    }
                    Eval(eval) => {
                        identity = false;
                        eval_fields.push(eval);
                    }
                }
            }
        }

        stmt_record.fields = partitioner.stmt;

        if identity {
            eval::Func::identity(ty)
        } else {
            let expr = eval::Expr::record_from_vec(eval_fields);
            eval::Func {
                args: vec![stmt::Type::Record(partitioner.ty)],
                ret: ty,
                expr,
            }
        }
    }

    pub fn partition_maybe_returning(
        &self,
        stmt: &mut Option<stmt::Returning>,
    ) -> Option<eval::Func> {
        /*
        let Some(returning) = stmt else { return None };
        let project = self.partition_returning(returning);

        if returning.as_expr().as_record().is_empty() {
            *stmt = None;
        }

        Some(project)
        */
        todo!()
    }
}

impl Partitioner<'_> {
    fn partition_expr(&mut self, stmt: &stmt::Expr) -> Partition {
        use Partition::*;

        match stmt {
            stmt::Expr::Cast(expr) => match self.partition_expr(&expr.expr) {
                Stmt(ty) => {
                    let arg = self.push_stmt_field((*expr.expr).clone(), ty);
                    Eval(eval::Expr::cast(arg, expr.ty.clone()))
                }
                Eval(eval) => Eval(eval::Expr::cast(eval, expr.ty.clone())),
            },
            stmt::Expr::Column(expr) => Stmt(self.schema.column(expr.column).ty.clone()),
            stmt::Expr::Value(expr) => Stmt(ty::value(expr)),
            stmt::Expr::Project(expr) => match self.partition_expr(&expr.base) {
                Stmt(ty) => {
                    let arg = self.push_stmt_field((*expr.base).clone(), ty);
                    Eval(eval::Expr::project(arg, expr.projection.clone()))
                }
                Eval(eval) => Eval(eval::Expr::project(eval, expr.projection.clone())),
            },
            stmt::Expr::DecodeEnum(expr, ty, ..) => match self.partition_expr(expr) {
                Stmt(base_ty) => {
                    let base = self.push_stmt_field((**expr).clone(), base_ty);
                    Eval(eval::Expr::DecodeEnum(Box::new(base), ty.clone()))
                }
                Eval(eval) => Eval(eval::Expr::DecodeEnum(Box::new(eval), ty.clone())),
            },
            _ => todo!("stmt={stmt:#?}"),
        }
    }

    fn push_stmt_field(&mut self, expr: stmt::Expr, ty: stmt::Type) -> eval::Expr {
        assert_eq!(self.stmt.len(), self.ty.len());
        let i = self.stmt.len();
        self.stmt.push(expr);
        self.ty.push(ty);
        eval::Expr::arg_project(0, [i])
    }
}
