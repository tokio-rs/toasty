use super::*;

impl<'stmt> Planner<'_, 'stmt> {
    pub(crate) fn extract_input(
        &mut self,
        expr: &mut stmt::Expr<'stmt>,
        sources: &[plan::InputSource],
        in_subquery: bool,
    ) -> Vec<plan::Input<'stmt>> {
        let mut inputs = vec![];
        self.extract_input_into(&mut inputs, expr, sources, in_subquery);
        inputs
    }

    fn extract_input_into(
        &mut self,
        inputs: &mut Vec<plan::Input<'stmt>>,
        expr: &mut stmt::Expr<'stmt>,
        sources: &[plan::InputSource],
        in_subquery: bool,
    ) -> Extract {
        use stmt::Expr::*;

        match expr {
            Arg(expr_arg) => {
                assert_eq!(0, expr_arg.position);

                let position = expr_arg.position;

                // Replace w/ a project... because
                *expr = stmt::Expr::project(&[position]);

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
            Project(expr_project) => match expr_project.base {
                stmt::ProjectBase::ExprSelf => Extract::Field,
                _ => todo!("project = {:#?}", expr_project),
            },
            _ => todo!("expr = {:#?}", expr),
        }
    }
}

enum Extract {
    /// Constant
    Const,

    /// References an argument
    Arg(usize),

    /// References a field, so cannot be extracted
    Field,
}

fn do_extract<'stmt>(
    inputs: &mut Vec<plan::Input<'stmt>>,
    expr: &mut stmt::Expr<'stmt>,
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
                    Some(eval::Expr::from_stmt(e))
                },
            });
        }
        _ => {}
    }
}
