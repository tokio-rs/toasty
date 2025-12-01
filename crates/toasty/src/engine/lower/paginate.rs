use toasty_core::stmt::{self, Limit};

use super::LowerStatement;

impl LowerStatement<'_, '_> {
    pub(super) fn rewrite_offset_after_as_filter(&self, stmt: &mut stmt::Query) {
        // Only do this for SQL statements
        if !self.capability().sql {
            return;
        }

        let Some(Limit::PaginateForward { limit, after }) = &stmt.limit else {
            return;
        };

        let Some(order_by) = &mut stmt.order_by else {
            panic!("Cursor-based pagination requires `order by` clause");
        };

        let stmt::ExprSet::Select(body) = &mut stmt.body else {
            todo!("stmt={stmt:#?}");
        };

        match after {
            Some(stmt::Expr::Value(stmt::Value::Record(_))) => {
                todo!()
            }
            Some(stmt::Expr::Value(value)) => {
                let expr = self.rewrite_offset_after_field_as_filter(
                    &order_by.exprs[0],
                    value.clone(),
                    true,
                );
                body.filter.add_filter(expr);
            }
            None => {
                // No filter necessary.
            }
            _ => todo!(),
        }

        // Lower `limit` clause to SQL compatible `Limit::Offset`.
        stmt.limit = Some(Limit::Offset {
            limit: limit.clone(),
            offset: None,
        })
    }

    fn rewrite_offset_after_field_as_filter(
        &self,
        order_by: &stmt::OrderByExpr,
        value: stmt::Value,
        last: bool,
    ) -> stmt::Expr {
        let op = match (order_by.order, last) {
            (Some(stmt::Direction::Desc), true) => stmt::BinaryOp::Lt,
            (Some(stmt::Direction::Desc), false) => stmt::BinaryOp::Le,
            (_, true) => stmt::BinaryOp::Gt,
            (_, false) => stmt::BinaryOp::Ge,
        };

        stmt::Expr::binary_op(order_by.expr.clone(), op, value)
    }
}
