use super::eval;
use crate::engine::Engine;
use toasty_core::{schema::db::Index, stmt};

/// Try to convert an index filter expression to a key expression
struct TryConvert<'a, 'stmt> {
    cx: stmt::ExprContext<'stmt>,

    /// Index being keyed on
    index: &'a Index,

    /// Eval function arguments
    args: Vec<stmt::Type>,
}

impl Engine {
    /// Attempts to optimize a WHERE clause filter into a direct primary key lookup.
    ///
    /// This function analyzes filter expressions to detect when they're actually specifying
    /// exact primary key values, enabling Toasty to use optimized "get by key" operations.
    /// While SQL databases automatically perform this optimization themselves, NoSQL databases
    /// like DynamoDB require Toasty to explicitly recognize key patterns and use their
    /// dedicated key-based APIs (GetItem, BatchGetItem) instead of slower scan operations.
    ///
    /// For example, it transforms queries like:
    /// - `WHERE id = 42` → direct key lookup for single-column primary key
    /// - `WHERE user_id = 1 AND post_id = 5` → direct key lookup for composite primary key
    /// - `WHERE id IN (1, 2, 3)` → batch key lookup for multiple records
    /// - `WHERE user_id = 1 AND post_id IN (5, 6)` → batch lookup for composite keys
    ///
    /// This optimization is essential for NoSQL backends where the difference between key
    /// lookups and scans can be orders of magnitude in both performance and cost. Without
    /// this analysis, even simple `find_by_id()` calls would become expensive table scans.
    ///
    /// Returns `Some(eval::Func)` if the filter can be converted to key lookups, `None` if
    /// the query requires a full scan with filtering.
    pub(crate) fn try_build_key_filter2(
        &self,
        cx: stmt::ExprContext<'_>,
        index: &Index,
        expr: &stmt::Expr,
        // Build the key filter as a projection of input
        args: Vec<stmt::Type>,
    ) -> Option<eval::Func> {
        let mut conv = TryConvert {
            cx,
            index,
            args,
        };

        conv.try_convert(expr).map(|expr| {
            if conv.args.is_empty() {
                // Extract constant
                let expr = match expr {
                    expr @ stmt::Expr::Value(stmt::Value::List(_)) => expr,
                    expr @ stmt::Expr::Record(_) => stmt::Expr::list_from_vec(vec![expr]),
                    /*
                    expr @ stmt::Expr::Value(stmt::Value::List(_)) => expr,
                    stmt::Expr::Value(value) => stmt::Expr::Value(stmt::Value::List(vec![value])),
                    expr @ stmt::Expr::Arg(_) => expr,
                    */
                    expr => todo!("expr={expr:#?}"),
                };

                // We can't always infer here (e.g. empty list)
                let ty = stmt::Type::list(self.index_key_record_ty(index));
                eval::Func::from_stmt_typed(expr, conv.args, ty)
            } else {
                assert!(expr.is_record(), "TODO; expr={expr:#?}");
                let project = eval::Func::from_stmt(expr, conv.args);
                debug_assert_eq!(project.ret, self.index_key_record_ty(index));
                project
            }
        })
    }
}

impl TryConvert<'_, '_> {
    fn try_convert(&mut self, expr: &stmt::Expr) -> Option<stmt::Expr> {
        use stmt::Expr::*;

        match expr {
            Arg(_) => todo!("{expr:#?}"),
            BinaryOp(e) => {
                if e.op.is_eq() {
                    if self.index.columns.len() > 1 {
                        None
                    } else {
                        Some(stmt::Expr::record([self.key_expr_to_eval(&e.rhs)]))
                    }
                } else {
                    todo!("expr = {:#?}", expr);
                }
            }
            InList(e) => {
                if !self.is_key_reference(&e.expr) {
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
                let mut fields = vec![stmt::Expr::null(); e.operands.len()];

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
                    let stmt::Expr::Reference(expr_ref) = &*binary_op.lhs else {
                        return None;
                    };

                    let column = self.cx.resolve_expr_reference(expr_ref).expect_column();

                    // Find the index field the operand references
                    let (index, _) = self
                        .index
                        .columns
                        .iter()
                        .enumerate()
                        .find(|(_, c)| column.id == c.column)?;

                    assert!(fields[index].is_value_null());

                    fields[index] = self.key_expr_to_eval(&binary_op.rhs);
                }

                if fields.iter().any(|field| field.is_value_null()) {
                    // Not all fields were matched
                    return None;
                }

                Some(stmt::Expr::record_from_vec(fields))
            }
            Or(e) => {
                let mut entries = vec![];

                for operand in &e.operands {
                    let key = self.try_convert(operand)?;

                    match key {
                        stmt::Expr::Value(_) | stmt::Expr::Record(_) => entries.push(key),
                        _ => todo!("key={:#?}", key),
                    }
                }

                Some(stmt::Expr::list_from_vec(entries))
            }
            InSubquery(_) => {
                todo!("expr = {:#?}", expr);
            }
            _ => None,
        }
    }

    fn key_expr_to_eval(&self, expr: &stmt::Expr) -> stmt::Expr {
        expr.clone()
    }

    fn key_list_expr_to_eval(&mut self, expr: &stmt::Expr) -> stmt::Expr {
        match expr {
            stmt::Expr::Arg(_) => {
                expr.clone()
            }
            stmt::Expr::Value(stmt::Value::List(items)) => {
                let mut ret = vec![];

                for item in items {
                    ret.push(match item {
                        record @ stmt::Value::Record(_) => record.clone(),
                        value => stmt::Value::record_from_vec(vec![value.clone()]),
                    });
                }

                stmt::Expr::Value(ret.into())
            }
            _ => todo!("expr={:#?}", expr),
        }
    }

    fn is_key_reference(&self, expr: &stmt::Expr) -> bool {
        match expr {
            stmt::Expr::Reference(stmt::ExprReference::Column { .. })
                if self.index.columns.len() == 1 =>
            {
                true
            }
            stmt::Expr::Record(expr_record) if self.index.columns.len() == expr_record.len() => {
                true
            }
            _ => false,
        }
    }
}
