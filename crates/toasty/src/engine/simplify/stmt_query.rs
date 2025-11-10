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
        // Get the primary relation
        let stmt::TableFactor::Table(primary_table_id) = &source_table.from_item.relation;

        // Check if primary source is empty VALUES
        if self.table_ref_is_empty_values(&source_table.tables[primary_table_id.0]) {
            // Primary source is empty - result will be empty unless there are
            // LEFT JOINs that could still produce results
            return !source_table
                .from_item
                .joins
                .iter()
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
