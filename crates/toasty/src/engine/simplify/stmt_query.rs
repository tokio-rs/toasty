use toasty_core::stmt;

use crate::engine::simplify::Simplify;

impl Simplify<'_> {
    pub(super) fn simplify_stmt_query_when_empty(&mut self, stmt: &mut stmt::Query) {
        if stmt.with.is_some() {
            // Just skip if there are any CTEs for now
            return;
        }

        if self.stmt_query_is_empty(stmt) {
            stmt.order_by = None;
            stmt.limit = None;

            match &mut stmt.body {
                // `COUNT(*)` still yields a row over no input, and sub-statement
                // args read columns of this query's source. Keep the select so
                // the planner can fold it to a constant.
                stmt::ExprSet::Select(select) if returning_needs_select(&select.returning) => {
                    select.filter.set(false);
                }
                body => *body = stmt::ExprSet::Values(Default::default()),
            }
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

        // SQL-style predicates retain only true rows. A constant NULL
        // comparison (for example, a cleared relation key) also matches none.
        if select.filter.is_false() || select.filter.as_expr().is_value_null() {
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

fn returning_needs_select(returning: &stmt::Returning) -> bool {
    let mut needs_select = false;
    stmt::visit::for_each_expr(returning, |expr| {
        needs_select |= matches!(
            expr,
            stmt::Expr::Arg(_)
                | stmt::Expr::Func(stmt::ExprFunc::Count(stmt::FuncCount { arg: None, .. }))
        );
    });
    needs_select
}
