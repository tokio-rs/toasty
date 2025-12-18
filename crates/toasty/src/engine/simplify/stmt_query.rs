use toasty_core::stmt;

use crate::engine::simplify::Simplify;

impl Simplify<'_> {
    pub(super) fn simplify_stmt_query_when_empty(&mut self, stmt: &mut stmt::Query) {
        if stmt.with.is_some() {
            // Just skip if there are any CTEs for now
            return;
        }

        if self.stmt_query_is_empty(stmt) {
            stmt.body = stmt::ExprSet::Values(Default::default());
            stmt.order_by = None;
            stmt.limit = None;
        }
    }

    pub(super) fn stmt_query_is_empty(&self, query: &stmt::Query) -> bool {
        match &query.body {
            stmt::ExprSet::Values(values) => values.is_empty(),
            stmt::ExprSet::Select(select) => self.stmt_select_is_empty(select),
            _ => false,
        }
    }

    pub(super) fn stmt_select_is_empty(&self, select: &stmt::Select) -> bool {
        // Only check table sources (lowered queries)
        let stmt::Source::Table(source_table) = &select.source else {
            return false;
        };

        if select.filter.is_false() {
            return true;
        }

        // Check if the source guarantees empty results
        self.source_table_is_empty(source_table)
    }

    fn source_table_is_empty(&self, source_table: &stmt::SourceTable) -> bool {
        if source_table.from.is_empty() {
            todo!(
                "this case is not handled yet at this level; \
                   Do we want to handle this or transform the statement \
                   to something different first?"
            );
        }

        // Get the primary relation from the first table
        let stmt::TableFactor::Table(primary_table_id) = &source_table.from[0].relation;

        // Check if primary source is empty VALUES
        if self.table_ref_is_empty_values(&source_table.tables[primary_table_id.0]) {
            // Primary source is empty - result will be empty unless there are
            // LEFT JOINs that could still produce results
            return !source_table
                .from
                .iter()
                .flat_map(|twj| &twj.joins)
                .any(|join| matches!(join.constraint, stmt::JoinOp::Left(_)));
        }

        // Check if any non-optional join has empty VALUES
        // Currently only LEFT joins exist, so any other join would be INNER
        // For now, this is conservative - we'd add INNER/CROSS join detection here
        false
    }

    fn table_ref_is_empty_values(&self, table_ref: &stmt::TableRef) -> bool {
        match table_ref {
            stmt::TableRef::Derived(derived) => {
                // Check if the derived table is empty VALUES. Inner queries
                // would have already been simplified, so we only need to check
                // for empty values here.
                matches!(&derived.subquery.body,
                    stmt::ExprSet::Values(values) if values.is_empty()
                )
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simplify::test::test_schema;
    use toasty_core::stmt::{Direction, Limit, OrderBy, OrderByExpr, Query, Values};

    #[test]
    fn empty_values_query_is_empty() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `stmt_query_is_empty(values([])) → true`
        let query = Query::values(Values::default());
        assert!(simplify.stmt_query_is_empty(&query));
    }

    #[test]
    fn non_empty_values_query_not_empty() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `stmt_query_is_empty(values([1])) → false`
        let mut values = Values::default();
        values.rows.push(stmt::Expr::Value(stmt::Value::from(1i64)));
        let query = Query::values(values);
        assert!(!simplify.stmt_query_is_empty(&query));
    }

    #[test]
    fn simplify_clears_order_by_and_limit() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `simplify_stmt_query_when_empty(empty_query)` clears `order_by` and
        // `limit`
        let mut query = Query::values(Values::default());
        query.order_by = Some(OrderBy {
            exprs: vec![OrderByExpr {
                expr: stmt::Expr::Value(stmt::Value::from(1i64)),
                order: Some(Direction::Asc),
            }],
        });
        query.limit = Some(Limit {
            limit: stmt::Expr::Value(stmt::Value::from(10i64)),
            offset: None,
        });

        simplify.simplify_stmt_query_when_empty(&mut query);

        assert!(query.order_by.is_none());
        assert!(query.limit.is_none());
    }

    #[test]
    fn non_empty_query_keeps_order_by_and_limit() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `simplify_stmt_query_when_empty(non_empty_query)` keeps `order_by`
        // and `limit`
        let mut values = Values::default();
        values.rows.push(stmt::Expr::Value(stmt::Value::from(1i64)));
        let mut query = Query::values(values);
        query.order_by = Some(OrderBy {
            exprs: vec![OrderByExpr {
                expr: stmt::Expr::Value(stmt::Value::from(1i64)),
                order: Some(Direction::Desc),
            }],
        });
        query.limit = Some(Limit {
            limit: stmt::Expr::Value(stmt::Value::from(10i64)),
            offset: None,
        });

        simplify.simplify_stmt_query_when_empty(&mut query);

        assert!(query.order_by.is_some());
        assert!(query.limit.is_some());
    }
}
