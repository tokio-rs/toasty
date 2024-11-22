use super::*;

// fn partition_expr_input(expr: &mut stmt::Expr, sources: &[plan::InputSource])

struct Partitioner<'a> {
    sources: &'a [plan::InputSource],
    input: Vec<plan::Input>,
}

enum Partition {
    Stmt,
    Eval {
        source: plan::InputSource,
        project: Option<eval::Expr>,
    },
}

impl Partition {
    fn is_stmt(&self) -> bool {
        matches!(self, Partition::Stmt)
    }
}

impl Partitioner<'_> {
    fn partition_expr(&mut self, expr: &mut stmt::Expr) -> Partition {
        match expr {
            stmt::Expr::Arg(expr) => Partition::Eval {
                source: self.sources[expr.position].clone(),
                project: None,
            },
            stmt::Expr::Column(_) => Partition::Stmt,
            stmt::Expr::InList(expr) => {
                assert!(self.partition_expr(&mut *expr.expr).is_stmt(), "TODO");

                if let Partition::Eval { source, project } = self.partition_expr(&mut *expr.list) {
                    let position = self.input.len();
                    self.input.push(plan::Input { source, project });
                    *expr.list = stmt::Expr::arg(position);
                }

                Partition::Stmt
            }
            stmt::Expr::Map(expr) => {
                let Partition::Eval { source, project } = self.partition_expr(&mut *expr.base)
                else {
                    todo!()
                };

                assert!(project.is_none());

                // For now, assume the mapping is fine w/o checking it Also,
                // this is a pretty mega hack since we are just removing the
                // map, assuming that this is the top-level projection.
                Partition::Eval {
                    source,
                    project: Some(eval::Expr::from(expr.map.take())),
                }
            }
            _ => todo!("{expr:#?}"),
        }
    }
}

impl Planner<'_> {
    pub(crate) fn partition_query_input(
        &mut self,
        stmt: &mut stmt::Query,
        sources: &[plan::InputSource],
    ) -> Vec<plan::Input> {
        let mut partitioner = Partitioner {
            sources,
            input: vec![],
        };

        match &mut *stmt.body {
            stmt::ExprSet::Select(select) => {
                assert!(select.source.is_table());
                let partition = partitioner.partition_expr(&mut select.filter);
                assert!(partition.is_stmt());
            }
            _ => todo!("{stmt:#?}"),
        }

        partitioner.input
    }
    /*
    pub(crate) fn extract_input(
        &mut self,
        expr: &mut stmt::Expr,
        sources: &[plan::InputSource],
        in_subquery: bool,
    ) -> Vec<plan::Input> {
        let mut inputs = vec![];
        self.extract_input_into(&mut inputs, expr, sources, in_subquery);
        inputs
    }

    fn extract_input_into(
        &mut self,
        inputs: &mut Vec<plan::Input>,
        expr: &mut stmt::Expr,
        sources: &[plan::InputSource],
        in_subquery: bool,
    ) -> Extract {
        use stmt::Expr::*;

        match expr {
            Arg(expr_arg) => {
                assert_eq!(0, expr_arg.position);

                let position = expr_arg.position;

                // Replace w/ a project... because
                // *expr = stmt::Expr::project([position]);
                todo!("expr={:#?}", expr);

                Extract::Arg(position)
            }
            And(expr_and) => {
                // TODO: make this smarter
                for operand in &mut expr_and.operands {
                    let action = self.extract_input_into(inputs, operand, sources, in_subquery);
                    do_extract(inputs, operand, sources, action);
                }

                Extract::Field
            }
            Or(expr_or) => {
                for operand in &mut expr_or.operands {
                    let action = self.extract_input_into(inputs, operand, sources, in_subquery);
                    do_extract(inputs, operand, sources, action);
                }

                Extract::Field
            }
            BinaryOp(expr_binary_op) => {
                // TODO: make smarter

                let action =
                    self.extract_input_into(inputs, &mut *expr_binary_op.lhs, sources, in_subquery);
                do_extract(inputs, &mut *expr_binary_op.lhs, sources, action);

                let action =
                    self.extract_input_into(inputs, &mut *expr_binary_op.rhs, sources, in_subquery);
                do_extract(inputs, &mut *expr_binary_op.rhs, sources, action);

                Extract::Field
            }
            InSubquery(expr_in_subquery) => {
                let position = inputs.len();

                inputs.push(plan::Input::from_var(
                    self.subquery_var(&*expr_in_subquery.query),
                ));

                let arg = stmt::ExprArg { position };

                *expr = stmt::Expr::in_list((*expr_in_subquery.expr).clone(), arg);

                // We probably should go through the lhs
                Extract::Field
            }
            Value(_) => Extract::Const,
            Record(_) => Extract::Field, // TODO: not correct
            List(_) => Extract::Field,   // TODO: not correct
            /*
            Project(expr_project) => match expr_project.base {
                stmt::ProjectBase::ExprSelf => Extract::Field,
                _ => todo!("project = {:#?}", expr_project),
            },
            */
            _ => todo!("expr = {:#?}", expr),
        }
    }
    */
}

/*
enum Extract {
    /// Constant
    Const,

    /// References an argument
    Arg(usize),

    /// References a field, so cannot be extracted
    Field,
}

fn do_extract(
    inputs: &mut Vec<plan::Input>,
    expr: &mut stmt::Expr,
    sources: &[plan::InputSource],
    action: Extract,
) {
    match action {
        Extract::Arg(position) => {
            // TODO: not always true, but for now...
            assert_eq!(inputs.len(), position);

            let e = expr.take();
            *expr = stmt::Expr::arg(position);

            inputs.push(plan::Input {
                source: sources[position],
                project: if e.is_arg() {
                    None
                } else {
                    Some(eval::Expr::from(e))
                },
            });
        }
        _ => {}
    }
}
*/
