//! Lowered SQL `IN` subqueries.
//!
//! Relation-key membership (`ExprInSubquery::relation_key`) compares keys,
//! and an absent key never matches. A `NULL` in the subquery's result would
//! also make a negated `IN` unknown for every row, so null keys are dropped
//! from the result. An unlimited subquery drops them with a filter:
//!
//! ```text
//! users.id IN (
//!     SELECT documents.user_id FROM documents
//!     WHERE documents.group = ? AND documents.user_id IS NOT NULL
//! )
//! ```
//!
//! A limited one must not: the filter would change which rows the `LIMIT`
//! selects. For documents ordered by position where the first is unlinked,
//! `LIMIT 1` selects that document and so matches no user; filtering first
//! would select the next, linked document instead. The limited query
//! therefore moves into a derived table and the null keys are dropped from
//! its result:
//!
//! ```text
//! users.id IN (
//!     SELECT tbl.column1 FROM (
//!         SELECT documents.user_id AS column1 FROM documents
//!         WHERE documents.group = ? ORDER BY documents.position LIMIT 1
//!     ) AS tbl
//!     WHERE tbl.column1 IS NOT NULL
//! )
//! ```
//!
//! The derived table also satisfies MySQL, which rejects `LIMIT` directly
//! inside an `IN` subquery, so a value membership (`relation_key` unset) is
//! wrapped too, without the filter, and keeps its result unchanged.

use toasty_core::stmt::{self, VisitMut, visit_mut};

/// Finish the lowered subquery of a SQL `IN`: drop absent keys from a
/// relation-key membership's result, and move a limited query into a
/// derived table. `query` must return a record.
pub(super) fn finish_in_subquery(
    cx: &stmt::ExprContext<'_>,
    query: &mut stmt::Query,
    relation_key: bool,
) {
    let select = query.body.as_select_mut_unwrap();
    let nullable = if relation_key {
        nullable_keys(cx, select)
    } else {
        vec![]
    };

    if query.limit.is_none() {
        let returning = select.returning.as_project_unwrap().as_record_unwrap();
        if let Some(filter) = present_keys(&returning.fields, &nullable) {
            select.add_filter(filter);
        }
        return;
    }

    let inner = std::mem::replace(query, stmt::Query::unit());
    let mut outer = derived_table(inner);
    let columns = &outer
        .returning
        .as_project_unwrap()
        .as_record_unwrap()
        .fields;
    if let Some(filter) = present_keys(columns, &nullable) {
        outer.filter = filter.into();
    }

    *query = stmt::Query::new(outer);
}

/// Which of `select`'s returned keys may be null.
fn nullable_keys(cx: &stmt::ExprContext<'_>, select: &stmt::Select) -> Vec<bool> {
    let cx = cx.scope(select);
    select
        .returning
        .as_project_unwrap()
        .as_record_unwrap()
        .fields
        .iter()
        .map(|field| may_be_null(&cx, field))
        .collect()
}

/// An `IS NOT NULL` check on each of `columns` flagged in `nullable`, or
/// `None` when none is flagged.
fn present_keys(columns: &[stmt::Expr], nullable: &[bool]) -> Option<stmt::Expr> {
    let checks: Vec<_> = columns
        .iter()
        .zip(nullable)
        .filter(|(_, nullable)| **nullable)
        .map(|(column, _)| stmt::Expr::is_not_null(column.clone()))
        .collect();
    (!checks.is_empty()).then(|| stmt::Expr::and_from_vec(checks))
}

/// Move a lowered query into a derived table, returning an unfiltered
/// `SELECT` of its record's columns.
///
/// The derived table keeps the query whole, `ORDER BY` and `LIMIT`/`OFFSET`
/// included, so a filter added to the returned select applies to the
/// query's result instead of the rows it scans. Column references inside
/// the query that escape it are shifted one level further out.
fn derived_table(mut query: stmt::Query) -> stmt::Select {
    ShiftOuterReferences { depth: 0 }.visit_stmt_query_mut(&mut query);

    let width = query
        .body
        .as_select_unwrap()
        .returning
        .as_project_unwrap()
        .as_record_unwrap()
        .len();

    let columns = (0..width).map(|column| {
        stmt::Expr::column(stmt::ExprColumn {
            nesting: 0,
            table: 0,
            column,
        })
    });

    stmt::Select {
        returning: stmt::Returning::Project(stmt::Expr::record(columns)),
        source: stmt::Source::Table(stmt::SourceTable::new(
            vec![stmt::TableRef::Derived(stmt::TableDerived {
                subquery: Box::new(query),
            })],
            stmt::TableWithJoins {
                relation: stmt::TableFactor::Table(stmt::SourceTableId(0)),
                joins: vec![],
            },
        )),
        filter: stmt::Filter::default(),
        distinct: false,
    }
}

/// Whether a returned key expression can be null. Only a (possibly cast)
/// reference to a non-nullable column is known to be present.
fn may_be_null(cx: &stmt::ExprContext<'_>, expr: &stmt::Expr) -> bool {
    let expr = match expr {
        stmt::Expr::Cast(cast) => &*cast.expr,
        expr => expr,
    };

    match expr {
        stmt::Expr::Reference(expr_reference @ stmt::ExprReference::Column(column))
            if column.nesting == 0 =>
        {
            match cx.resolve_expr_reference(expr_reference) {
                stmt::ResolvedRef::Column(column) => column.nullable,
                _ => true,
            }
        }
        _ => true,
    }
}

/// Moving a query into a derived table nests it one level deeper, so column
/// references that escape it must reach one level further out.
struct ShiftOuterReferences {
    depth: usize,
}

impl VisitMut for ShiftOuterReferences {
    fn visit_stmt_query_mut(&mut self, i: &mut stmt::Query) {
        self.depth += 1;
        visit_mut::visit_stmt_query_mut(self, i);
        self.depth -= 1;
    }

    fn visit_expr_column_mut(&mut self, i: &mut stmt::ExprColumn) {
        // `depth` counts the wrapped query itself, so references resolving
        // inside it have `nesting < depth`.
        if i.nesting >= self.depth {
            i.nesting += 1;
        }
    }
}
