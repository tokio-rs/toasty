use hashbrown::HashSet;
use toasty_core::{
    schema::db::ColumnId,
    stmt::{self, ExprReference},
};

use super::Normalize;

impl Normalize<'_> {
    /// Appends missing primary-key fields to ambiguous SQL cursor ordering.
    ///
    /// The public query retains only the user-provided ordering. Normalization
    /// runs on the cloned statement passed to the engine, making these
    /// tie-breakers internal to execution and cursor generation.
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

        let model = self.schema.app.model(model_id).as_root_unwrap();
        let mapping = self.schema.mapping.model(model_id);
        let table = self.schema.db.table(mapping.table);
        let ordered_columns: HashSet<_> = order_by
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
        for field_id in &model.primary_key.fields {
            let field_mapping = &mapping.fields[field_id.index];
            let field_columns: Vec<_> = field_mapping.columns().map(|(column, _)| column).collect();
            if field_columns
                .iter()
                .all(|column| ordered_columns.contains(column))
            {
                continue;
            }

            order_by.exprs.push(stmt::OrderByExpr {
                expr: stmt::Expr::ref_self_field(*field_id),
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
