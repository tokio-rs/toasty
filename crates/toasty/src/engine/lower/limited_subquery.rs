//! SQL `IN` subqueries with `LIMIT`/`OFFSET`.
//!
//! Relation-key membership (`ExprInSubquery::relation_key`) compares keys,
//! and an absent key never matches. An
//! unlimited subquery drops its null keys with a filter (see
//! `LiftInSubquery`), but a limited one must not: the filter would change
//! which rows the `LIMIT` selects. For documents ordered by position where
//! the first is unlinked, `LIMIT 1` selects that document and so matches no
//! user; filtering first would select the next, linked document instead.
//!
//! The limited query therefore moves into a derived table and the null keys
//! are dropped from its result:
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

/// Move the lowered subquery of a SQL `IN` into a derived table. For a
/// relation-key membership, absent keys are dropped after its `LIMIT`
/// applies. `query` must return a record.
pub(super) fn wrap_limited_in_subquery(
    cx: &stmt::ExprContext<'_>,
    query: &mut stmt::Query,
    relation_key: bool,
) {
    debug_assert!(query.limit.is_some());

    let inner = std::mem::replace(query, stmt::Query::unit());
    let select = inner.body.as_select_unwrap();
    let inner_cx = cx.scope(select);
    let nullable: Vec<_> = select
        .returning
        .as_project_unwrap()
        .as_record_unwrap()
        .fields
        .iter()
        .map(|field| relation_key && may_be_null(&inner_cx, field))
        .collect();

    let mut outer = derived_table(inner);
    let columns = &outer
        .returning
        .as_project_unwrap()
        .as_record_unwrap()
        .fields;
    let filter = columns
        .iter()
        .zip(nullable)
        .filter(|(_, nullable)| *nullable)
        .map(|(column, _)| stmt::Expr::is_not_null(column.clone()))
        .collect();
    outer.filter = stmt::Expr::and_from_vec(filter).into();

    *query = stmt::Query::new(outer);
}

/// Move a lowered query into a derived table, returning an unfiltered
/// `SELECT` of its record's columns.
///
/// The derived table keeps the query whole, `ORDER BY` and `LIMIT`/`OFFSET`
/// included, so a filter added to the returned select applies to the
/// query's result instead of the rows it scans. Column references inside
/// the query that escape it are shifted one level further out.
pub(super) fn derived_table(mut query: stmt::Query) -> stmt::Select {
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
