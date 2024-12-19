use super::*;

/// Try to convert an index filter expression to a key expression
struct TryConvert<'a> {
    index: &'a Index,
}

impl Planner<'_> {
    /// If the expression is shaped like a key expression, then convert it to
    /// one.
    pub(crate) fn try_build_key_filter(
        &self,
        index: &Index,
        expr: &stmt::Expr,
    ) -> Option<eval::Func> {
        TryConvert { index }.try_convert(expr).map(|expr| {
            let expr = match expr {
                expr @ eval::Expr::Value(stmt::Value::List(_)) => expr,
                eval::Expr::Value(value) => eval::Expr::Value(stmt::Value::List(vec![value])),
                expr => todo!("expr={expr:#?}"),
            };

            let key_ty = match &index.columns[..] {
                [column] => self.schema.column(column).ty.clone(),
                columns => {
                    todo!("columns={columns:#?}");
                }
            };

            eval::Func {
                args: vec![],
                ret: stmt::Type::list(key_ty),
                expr,
            }
        })
    }
}

impl<'a> TryConvert<'a> {
    fn try_convert(&self, expr: &stmt::Expr) -> Option<eval::Expr> {
        use stmt::Expr::*;

        match expr {
            Arg(_) => todo!("{expr:#?}"),
            BinaryOp(e) => {
                if e.op.is_eq() {
                    if self.index.columns.len() > 1 {
                        None
                    } else {
                        Some(self.expr_arg_to_project(&e.rhs))
                    }
                } else {
                    todo!("expr = {:#?}", expr);
                }
            }
            InList(e) => {
                if !self.is_key_projection(&*e.expr) {
                    return None;
                }

                Some(self.expr_arg_to_project(&e.list))
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

                    // The LHS of the operand is a projection referencing an index field
                    let Project(p) = &*binary_op.lhs else {
                        return None;
                    };

                    // Find the index field the operand references
                    let Some((index, _)) = self
                        .index
                        .columns
                        .iter()
                        .enumerate()
                        .find(|(_, c)| p.projection.resolves_to(c.column))
                    else {
                        return None;
                    };

                    assert!(fields[index].is_null());

                    fields[index] = self.expr_arg_to_project(&binary_op.rhs);
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

    fn expr_arg_to_project(&self, expr: &stmt::Expr) -> eval::Expr {
        assert!(expr.is_value());
        eval::Expr::from_stmt(expr.clone())
    }

    fn is_key_projection(&self, expr: &stmt::Expr) -> bool {
        match expr {
            stmt::Expr::Column(expr_column) if self.index.columns.len() == 1 => true,
            stmt::Expr::Record(expr_record) if self.index.columns.len() == expr_record.len() => {
                true
            }
            _ => false,
        }
    }
}
