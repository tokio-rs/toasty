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
    /// The expr *must* be evaluated by Toasty
    Eval(stmt::Expr),

    /// The expr *must* be evaluated by the database
    Stmt,

    /// The expr *can* be evaluated by either Toasty or the database
    ConstStmt,
}

impl Planner<'_> {
    /// Partition a returning statement between what can be handled by the
    /// target database and what Toasty handles in-memory.
    pub(crate) fn partition_returning(&self, stmt: &mut stmt::Returning) -> eval::Func {
        let ret = self.infer_expr_ty(stmt.as_expr(), &[]);

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
            ConstStmt => todo!(),
            Eval(expr) => {
                *stmt = stmt::Expr::record_from_vec(partitioner.stmt);

                let args = if partitioner.ty.is_empty() {
                    vec![]
                } else {
                    vec![stmt::Type::Record(partitioner.ty)]
                };

                eval::Func::from_stmt_unchecked(expr, args, ret)
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
                eval_fields.push(stmt::Expr::Value(value));
            } else {
                match partitioner.partition_expr(&field) {
                    Stmt => {
                        let ty = self.infer_expr_ty(&field, &[]);
                        let arg = partitioner.push_stmt_field(field, ty);
                        eval_fields.push(arg);
                    }
                    ConstStmt => todo!(),
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
            let expr = stmt::Expr::record_from_vec(eval_fields);
            eval::Func::from_stmt_unchecked(expr, vec![stmt::Type::Record(partitioner.ty)], ret)
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
                    let ty = self.planner.infer_expr_ty(&expr.expr, &[]);
                    let arg = self.push_stmt_field((*expr.expr).clone(), ty);
                    Eval(stmt::Expr::cast(arg, expr.ty.clone()))
                }
                ConstStmt => Eval(stmt::Expr::cast((*expr.expr).clone(), expr.ty.clone())),
                Eval(eval) => Eval(stmt::Expr::cast(eval, expr.ty.clone())),
            },
            stmt::Expr::Column(_) => Stmt,
            stmt::Expr::Project(expr) => match self.partition_expr(&expr.base) {
                Stmt => {
                    let ty = self.planner.infer_expr_ty(&expr.base, &[]);
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
                    .map(|field| self.partition_expr(field))
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
            stmt::Expr::DecodeEnum(expr, ty, variant) => match self.partition_expr(expr) {
                Stmt => {
                    let base_ty = self.planner.infer_expr_ty(expr, &[]);
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
