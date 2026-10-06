//! SQL `IN` subqueries with `LIMIT`/`OFFSET`.
//!
//! Relation membership compares keys, and an absent key never matches. An
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
//! inside an `IN` subquery, so a value membership that is not a relation's
//! (`ExprInSubquery::exclude_null_keys` unset) is wrapped too, without the
//! filter, and keeps SQL's `NULL` semantics.

use toasty_core::stmt::{self, VisitMut, visit_mut};

/// Move the lowered subquery of a SQL `IN` into a derived table. With
/// `exclude_null_keys`, the null keys are dropped after its `LIMIT` applies;
/// otherwise the result is unchanged. `query` must return a record.
pub(super) fn wrap_limited_in_subquery(
    cx: &stmt::ExprContext<'_>,
    query: &mut stmt::Query,
    exclude_null_keys: bool,
) {
    debug_assert!(query.limit.is_some());

    let mut inner = std::mem::replace(query, stmt::Query::unit());
    ShiftOuterReferences { depth: 0 }.visit_stmt_query_mut(&mut inner);

    let select = inner.body.as_select_unwrap();
    let inner_cx = cx.scope(select);
    let fields = &select
        .returning
        .as_project_unwrap()
        .as_record_unwrap()
        .fields;

    let columns: Vec<_> = (0..fields.len())
        .map(|column| {
            stmt::Expr::column(stmt::ExprColumn {
                nesting: 0,
                table: 0,
                column,
            })
        })
        .collect();

    let filter = fields
        .iter()
        .zip(&columns)
        .filter(|(field, _)| exclude_null_keys && may_be_null(&inner_cx, field))
        .map(|(_, column)| stmt::Expr::is_not_null(column.clone()))
        .collect();

    *query = stmt::Query::new(stmt::Select {
        returning: stmt::Returning::Project(stmt::Expr::record(columns)),
        source: stmt::Source::Table(stmt::SourceTable::new(
            vec![stmt::TableRef::Derived(stmt::TableDerived {
                subquery: Box::new(inner),
            })],
            stmt::TableWithJoins {
                relation: stmt::TableFactor::Table(stmt::SourceTableId(0)),
                joins: vec![],
            },
        )),
        filter: stmt::Expr::and_from_vec(filter).into(),
        distinct: false,
    });
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
