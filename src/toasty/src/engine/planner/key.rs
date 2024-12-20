use super::*;

/// Try to convert an index filter expression to a key expression
struct TryConvert<'a> {
    planner: &'a Planner<'a>,

    /// Index being keyed on
    index: &'a Index,

    /// Eval function arguments
    args: Vec<stmt::Type>,
}

impl Planner<'_> {
    /// If the expression is shaped like a key expression, then convert it to
    /// one.
    pub(crate) fn try_build_key_filter(
        &self,
        index: &Index,
        expr: &stmt::Expr,
    ) -> Option<eval::Func> {
        let mut conv = TryConvert {
            planner: self,
            index,
            args: vec![],
        };

        conv.try_convert(expr).map(|expr| {
            let expr = match expr {
                expr @ eval::Expr::Value(stmt::Value::List(_)) => expr,
                eval::Expr::Value(value) => eval::Expr::Value(stmt::Value::List(vec![value])),
                expr @ eval::Expr::Record(_) => eval::Expr::list_from_vec(vec![expr]),
                expr @ eval::Expr::Arg(_) => expr,
                expr => todo!("expr={expr:#?}"),
            };

            let key_ty = self.index_key_ty(index);

            eval::Func {
                args: conv.args,
                ret: stmt::Type::list(key_ty),
                expr,
            }
        })
    }
}

impl<'a> TryConvert<'a> {
    fn try_convert(&mut self, expr: &stmt::Expr) -> Option<eval::Expr> {
        use stmt::Expr::*;

        match expr {
            Arg(_) => todo!("{expr:#?}"),
            BinaryOp(e) => {
                if e.op.is_eq() {
                    if self.index.columns.len() > 1 {
                        None
                    } else {
                        Some(self.key_expr_to_eval(&e.rhs))
                    }
                } else {
                    todo!("expr = {:#?}", expr);
                }
            }
            InList(e) => {
                if !self.is_key_reference(&*e.expr) {
                    return None;
                }

                Some(self.key_list_expr_to_eval(&e.list))
            }
            And(e) => {
                assert!(
                    e.operands.len() > 1,
                    "this should have been simplified before"
                );

                if e.operands.len() != self.index.columns.len() {
                    return None;
                }

                // Composite key. Try assigning the AND operands to key fields
                let mut fields = vec![eval::Expr::null(); e.operands.len()];

                for operand in &e.operands {
                    // If the AND operand is not a binary op, then not a key expression
                    let BinaryOp(binary_op) = operand else {
                        return None;
                    };

                    // If the binary op operand is not `==` then not a key expr
                    if !binary_op.op.is_eq() {
                        return None;
                    };

                    // The LHS of the operand is a column referencing an index field
                    let Column(expr_column) = &*binary_op.lhs else {
                        return None;
                    };

                    // Find the index field the operand references
                    let Some((index, _)) = self
                        .index
                        .columns
                        .iter()
                        .enumerate()
                        .find(|(_, c)| expr_column.column == c.column)
                    else {
                        return None;
                    };

                    assert!(fields[index].is_null());

                    fields[index] = self.key_expr_to_eval(&binary_op.rhs);
                }

                if fields.iter().any(|field| field.is_null()) {
                    // Not all fields were matched
                    return None;
                }

                Some(eval::Expr::record_from_vec(fields))
            }
            Or(e) => {
                let mut entries = vec![];

                for operand in &e.operands {
                    let Some(key) = self.try_convert(operand) else {
                        return None;
                    };

                    match key {
                        eval::Expr::Value(_) | eval::Expr::Record(_) => entries.push(key),
                        _ => todo!("key={:#?}", key),
                    }
                }

                Some(eval::Expr::list_from_vec(entries))
            }
            InSubquery(_) => {
                todo!("expr = {:#?}", expr);
            }
            _ => None,
        }
    }

    fn key_expr_to_eval(&self, expr: &stmt::Expr) -> eval::Expr {
        assert!(expr.is_value(), "expr={:#?}", expr);
        eval::Expr::from_stmt(expr.clone())
    }

    fn key_list_expr_to_eval(&mut self, expr: &stmt::Expr) -> eval::Expr {
        match expr {
            stmt::Expr::Arg(_) => {
                self.args
                    .push(stmt::Type::list(self.planner.index_key_ty(self.index)));
                eval::Expr::from_stmt(expr.clone())
            }
            stmt::Expr::Value(_) => eval::Expr::from_stmt(expr.clone()),
            _ => todo!("expr={:#?}", expr),
        }
    }

    fn is_key_reference(&self, expr: &stmt::Expr) -> bool {
        match expr {
            stmt::Expr::Column(expr_column) if self.index.columns.len() == 1 => true,
            stmt::Expr::Record(expr_record) if self.index.columns.len() == expr_record.len() => {
                true
            }
            _ => false,
        }
    }
}
