use hashbrown::HashSet;
use toasty_core::{
    schema::db::ColumnId,
    stmt::{self, ExprReference},
};

use super::Normalize;

impl Normalize<'_> {
    /// Appends missing physical primary-key columns to ambiguous SQL cursor
    /// ordering.
    ///
    /// The public query retains only the user-provided ordering. Normalization
    /// runs on the cloned statement passed to the engine, making these
    /// tie-breakers internal to execution and cursor generation. Physical
    /// column references keep embedded newtype keys scalar after lowering.
    pub(super) fn normalize_cursor_order(&mut self, query: &mut stmt::Query) {
        if !self.capability.sql || !matches!(query.limit, Some(stmt::Limit::Cursor(_))) {
            return;
        }

        let Some(order_by) = &mut query.order_by else {
            return;
        };
        if order_by.exprs.is_empty() {
            return;
        }
        let Some(select) = query.body.as_select() else {
            return;
        };
        let Some(model_id) = select.source.model_id() else {
            return;
        };

        let mapping = self.schema.mapping.model(model_id);
        let table = self.schema.db.table(mapping.table);
        let mut ordered_columns: HashSet<_> = order_by
            .exprs
            .iter()
            .filter_map(|order| resolve_order_column(mapping, &order.expr))
            .collect();

        let unambiguous = table.indices.iter().any(|index| {
            (index.unique || index.primary_key)
                && index
                    .columns
                    .iter()
                    .all(|index_column| ordered_columns.contains(&index_column.column))
                && index
                    .columns
                    .iter()
                    .all(|index_column| !table.column(index_column.column).nullable)
        });
        if unambiguous {
            return;
        }

        let direction = order_by.exprs.last().and_then(|order| order.order);
        for column in &table.primary_key.columns {
            if !ordered_columns.insert(*column) {
                continue;
            }

            order_by.exprs.push(stmt::OrderByExpr {
                expr: stmt::Expr::column(*column),
                order: direction,
            });
        }
    }
}

/// Resolves a model field path used by `order_by` to its single physical
/// column. Expressions that do not resolve to exactly one column cannot prove
/// that an order is unique.
fn resolve_order_column(
    mapping: &toasty_core::schema::mapping::Model,
    expr: &stmt::Expr,
) -> Option<ColumnId> {
    if let stmt::Expr::Reference(ExprReference::Column(column)) = expr {
        return (column.nesting == 0 && column.table == 0).then_some(ColumnId {
            table: mapping.table,
            index: column.column,
        });
    }

    let projection = field_projection(expr)?;
    let field = mapping.resolve_field_mapping(&projection)?;
    let mut columns = field.columns();
    let (column, _) = columns.next()?;
    columns.next().is_none().then_some(column)
}

fn field_projection(expr: &stmt::Expr) -> Option<stmt::Projection> {
    match expr {
        stmt::Expr::Reference(ExprReference::Field { nesting: 0, index }) => {
            Some(stmt::Projection::single(*index))
        }
        stmt::Expr::Project(project) => {
            let mut projection = field_projection(&project.base)?;
            for step in &project.projection {
                projection.push(step);
            }
            Some(projection)
        }
        _ => None,
    }
}
