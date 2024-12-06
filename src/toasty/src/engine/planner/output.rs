use super::*;

struct Partitioner<'a> {
    planner: &'a Planner<'a>,

    // Returning statement expressions. The returning statement will be a
    // record, these are the field expressions.
    stmt: Vec<stmt::Expr>,

    // Type of each field expression
    ty: Vec<stmt::Type>,
}

#[derive(Debug)]
enum Partition {
    /// The expr can evaluate on the databases as a statement.
    Stmt,
    Eval(eval::Expr),
}

impl Planner<'_> {
    /// Partition a returning statement between what can be handled by the
    /// target database and what Toasty handles in-memory.
    pub(crate) fn partition_returning(&self, stmt: &mut stmt::Returning) -> eval::Func {
        use Partition::*;

        let ret = self.infer_expr_ty(stmt.as_expr());

        match stmt {
            stmt::Returning::Expr(stmt::Expr::Record(expr_record)) => {
                // returning an expression record is special-cased because it
                // might be able to be passed through to the database as an
                // identity projection.
                self.partition_returning_expr_record(expr_record, ret)
            }
            stmt::Returning::Expr(expr) => self.partition_returning_expr(expr, ret),
            _ => todo!("returning={stmt:#?}"),
        }
    }

    fn partition_returning_expr(&self, stmt: &mut stmt::Expr, ret: stmt::Type) -> eval::Func {
        use Partition::*;

        let mut partitioner = Partitioner {
            planner: self,
            stmt: vec![],
            ty: vec![],
        };

        match partitioner.partition_expr(stmt) {
            Stmt => {
                todo!()
            }
            Eval(expr) => {
                *stmt = stmt::Expr::record_from_vec(partitioner.stmt);
                eval::Func {
                    args: vec![stmt::Type::Record(partitioner.ty)],
                    ret,
                    expr,
                }
            }
        }
    }

    fn partition_returning_expr_record(
        &self,
        stmt_record: &mut stmt::ExprRecord,
        ret: stmt::Type,
    ) -> eval::Func {
        use Partition::*;

        let mut partitioner = Partitioner {
            planner: self,
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
                    Stmt => {
                        let ty = self.infer_expr_ty(&field);
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
            eval::Func::identity(ret)
        } else {
            let expr = eval::Expr::record_from_vec(eval_fields);
            eval::Func {
                args: vec![stmt::Type::Record(partitioner.ty)],
                ret,
                expr,
            }
        }
    }

    pub fn partition_maybe_returning(
        &self,
        stmt: &mut Option<stmt::Returning>,
    ) -> Option<eval::Func> {
        let Some(returning) = stmt else { return None };
        let project = self.partition_returning(returning);

        if returning.as_expr().as_record().is_empty() {
            *stmt = None;
        }

        Some(project)
    }
}

impl Partitioner<'_> {
    fn partition_expr(&mut self, stmt: &stmt::Expr) -> Partition {
        use Partition::*;

        match stmt {
            stmt::Expr::Cast(expr) => match self.partition_expr(&expr.expr) {
                Stmt => {
                    let ty = self.planner.infer_expr_ty(&expr.expr);
                    let arg = self.push_stmt_field((*expr.expr).clone(), ty);
                    Eval(eval::Expr::cast(arg, expr.ty.clone()))
                }
                Eval(eval) => Eval(eval::Expr::cast(eval, expr.ty.clone())),
            },
            stmt::Expr::Column(_) => Stmt,
            stmt::Expr::Project(expr) => match self.partition_expr(&expr.base) {
                Stmt => {
                    let ty = self.planner.infer_expr_ty(&expr.base);
                    let arg = self.push_stmt_field((*expr.base).clone(), ty);
                    Eval(eval::Expr::project(arg, expr.projection.clone()))
                }
                Eval(eval) => Eval(eval::Expr::project(eval, expr.projection.clone())),
            },
            stmt::Expr::Record(expr) => {
                let field_partition_res: Vec<_> = expr
                    .fields
                    .iter()
                    .map(|field| self.partition_expr(field))
                    .collect();

                if field_partition_res.iter().all(|res| res.is_stmt()) {
                    Stmt
                } else {
                    todo!()
                }
            }
            stmt::Expr::Value(_) => Stmt,
            stmt::Expr::DecodeEnum(expr, ty, ..) => match self.partition_expr(expr) {
                Stmt => {
                    let base_ty = self.planner.infer_expr_ty(expr);
                    let base = self.push_stmt_field((**expr).clone(), base_ty);
                    Eval(eval::Expr::DecodeEnum(Box::new(base), ty.clone()))
                }
                Eval(eval) => Eval(eval::Expr::DecodeEnum(Box::new(eval), ty.clone())),
            },
            _ => todo!("stmt={stmt:#?}"),
        }
    }

    fn push_stmt_field(&mut self, expr: stmt::Expr, ty: stmt::Type) -> eval::Expr {
        // Record expressions must be flattened
        if let stmt::Expr::Record(expr_record) = expr {
            let stmt::Type::Record(field_tys) = ty else {
                todo!()
            };

            let fields: Vec<_> = expr_record
                .fields
                .into_iter()
                .zip(field_tys.into_iter())
                .map(|(field, ty)| self.push_stmt_field(field, ty))
                .collect();

            eval::Expr::record_from_vec(fields)
        } else {
            assert_eq!(self.stmt.len(), self.ty.len());
            let i = self.stmt.len();
            self.stmt.push(expr);
            self.ty.push(ty);
            eval::Expr::arg_project(0, [i])
        }
    }
}

impl Partition {
    fn is_stmt(&self) -> bool {
        matches!(self, Partition::Stmt)
    }
}
