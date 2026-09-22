use toasty_core::stmt;

use super::LowerStatement;

impl LowerStatement<'_, '_> {
    pub(super) fn rewrite_offset_after_as_filter(&self, stmt: &mut stmt::Query) {
        // Only do this for SQL statements
        if !self.capability().sql() {
            return;
        }

        let Some(order_by) = &mut stmt.order_by else {
            return;
        };

        let Some(stmt::Limit::Cursor(cursor)) = &mut stmt.limit else {
            return;
        };

        let Some(after) = cursor.after.take() else {
            return;
        };

        let stmt::ExprSet::Select(body) = &mut stmt.body else {
            todo!("stmt={stmt:#?}");
        };

        match after {
            stmt::Expr::Value(stmt::Value::Record(value)) => {
                // Rows strictly beyond the cursor in lexicographic order:
                // `e0 > v0 OR (e0 = v0 AND e1 > v1) OR ...`, flipping each
                // comparison for descending columns. A plain conjunction of
                // per-column comparisons would skip rows that tie the cursor
                // on a leading column.
                let terms = value
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(index, field_value)| {
                        let mut operands: Vec<_> = order_by.exprs[..index]
                            .iter()
                            .zip(value.fields.iter())
                            .map(|(order_by, eq_value)| {
                                self.rewrite_cursor_field_as_equality(order_by, eq_value.clone())
                            })
                            .collect();

                        operands.push(self.rewrite_offset_after_field_as_filter(
                            &order_by.exprs[index],
                            field_value.clone(),
                        ));

                        stmt::Expr::and_from_vec(operands)
                    })
                    .collect();

                body.filter.add_filter(stmt::Expr::or_from_vec(terms));
            }
            stmt::Expr::Value(value) => {
                let expr = self.rewrite_offset_after_field_as_filter(&order_by.exprs[0], value);
                body.filter.add_filter(expr);
            }
            _ => todo!(),
        }
    }

    fn rewrite_offset_after_field_as_filter(
        &self,
        order_by: &stmt::OrderByExpr,
        value: stmt::Value,
    ) -> stmt::Expr {
        let nulls_first = match order_by.order {
            Some(stmt::Direction::Desc) => !self.capability().sql_nulls_first_on_asc,
            _ => self.capability().sql_nulls_first_on_asc,
        };

        if value.is_null() {
            return if nulls_first {
                stmt::Expr::is_not_null(order_by.expr.clone())
            } else {
                false.into()
            };
        }

        let op = match order_by.order {
            Some(stmt::Direction::Desc) => stmt::BinaryOp::Lt,
            _ => stmt::BinaryOp::Gt,
        };
        let comparison = stmt::Expr::binary_op(order_by.expr.clone(), op, value);

        if nulls_first {
            comparison
        } else {
            stmt::Expr::or(comparison, stmt::Expr::is_null(order_by.expr.clone()))
        }
    }

    fn rewrite_cursor_field_as_equality(
        &self,
        order_by: &stmt::OrderByExpr,
        value: stmt::Value,
    ) -> stmt::Expr {
        if value.is_null() {
            stmt::Expr::is_null(order_by.expr.clone())
        } else {
            stmt::Expr::eq(order_by.expr.clone(), value)
        }
    }
}
