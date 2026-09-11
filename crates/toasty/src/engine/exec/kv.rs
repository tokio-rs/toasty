use toasty_core::{
    driver::Capability, schema::db::TableId, stmt, stmt::ExprContext, stmt::ValueSet,
};

use crate::engine::{fold, simplify};

use super::Exec;

impl Exec<'_> {
    /// Expand bound tuple membership without changing scalar IN semantics.
    /// In particular, KV null membership differs from SQL's three-valued
    /// logic, so a general simplification pass after binding is not valid.
    pub(super) fn normalize_scan_filter(&self, filter: &mut stmt::Expr, table: TableId) {
        let cx = self.engine.expr_cx_for(self.engine.schema.db.table(table));
        stmt::visit_mut::for_each_expr_mut(filter, |expr| {
            let replacement = match expr {
                stmt::Expr::InList(in_list)
                    if matches!(*in_list.expr, stmt::Expr::Record(_))
                        && in_list.expr.is_stable()
                        && matches!(*in_list.list, stmt::Expr::Value(stmt::Value::List(_))) =>
                {
                    let operands = Self::split_filter_in_list(
                        in_list.expr.take(),
                        in_list.list.take(),
                        cx,
                        self.engine.capability,
                    );
                    let mut expr = stmt::Expr::or_from_vec(operands);
                    fold::fold_stmt(&mut expr);
                    Some(expr)
                }
                // Only fold this boolean node, leaving scalar IN children
                // untouched. This propagates empty tuple lists through gates.
                stmt::Expr::And(_) | stmt::Expr::Or(_) | stmt::Expr::Not(_) => fold::fold_one(expr),
                _ => None,
            };
            if let Some(replacement) = replacement {
                *expr = replacement;
            }
        });
    }

    /// Split a composite filter into individual key predicates.
    ///
    /// Recognizes these forms and decomposes them:
    /// - `ANY(MAP(Value::List([v1, v2, ...]), pred))` — substitutes each vi
    ///   into pred
    /// - `InList(expr, Value::List([v1, v2, ...]))` — produces `expr == vi`
    ///   for each value
    ///
    /// For any other form (including a single equality), simplifies and returns
    /// it as a single-element vec (or empty if unsatisfiable).
    ///
    /// Each returned predicate has been simplified and is guaranteed
    /// satisfiable.
    pub(super) fn split_filter(&self, filter: stmt::Expr, table: TableId) -> Vec<stmt::Expr> {
        let db_table = self.engine.schema.db.table(table);
        let cx = self.engine.expr_cx_for(db_table);

        let capability = self.engine.capability;
        match filter {
            stmt::Expr::Any(any) => Self::split_filter_any_map(*any.expr, cx, capability),
            stmt::Expr::InList(in_list) => {
                Self::split_filter_in_list(*in_list.expr, *in_list.list, cx, capability)
            }
            mut other => {
                simplify::simplify_expr(cx, self.engine.capability, &mut other);
                if other.is_unsatisfiable() {
                    vec![]
                } else {
                    vec![other]
                }
            }
        }
    }

    /// `ANY(MAP(Value::List([v1, v2, ...]), pred))` — substitutes each value
    /// into the predicate template.
    ///
    /// Duplicate values are collapsed: each kv-layer fan-out becomes one
    /// driver call per partition key, and a downstream `HashIndex` build
    /// over the merged rows requires unique keys.
    fn split_filter_any_map(
        map_expr: stmt::Expr,
        cx: ExprContext<'_>,
        capability: &Capability,
    ) -> Vec<stmt::Expr> {
        let stmt::Expr::Map(map) = map_expr else {
            unreachable!()
        };
        let stmt::Expr::Value(stmt::Value::List(items)) = *map.base else {
            unreachable!()
        };

        let mut seen = ValueSet::with_capacity(items.len());
        items
            .into_iter()
            .filter(|item| !item.is_null())
            .filter(|item| seen.insert(item.clone()))
            .filter_map(|item| {
                let mut pred = *map.map.clone();
                // Unpack Record fields so arg(i) binds to field i.
                match item {
                    stmt::Value::Record(r) => pred.substitute(&r.fields[..]),
                    item => pred.substitute([item]),
                }
                simplify::simplify_expr(cx, capability, &mut pred);
                (!pred.is_unsatisfiable()).then_some(pred)
            })
            .collect()
    }

    /// `InList(expr, Value::List([v1, v2, ...]))` — produces `expr == vi` for
    /// each value.
    ///
    /// Duplicate values are collapsed; see `split_filter_any_map`.
    fn split_filter_in_list(
        expr: stmt::Expr,
        list: stmt::Expr,
        cx: ExprContext<'_>,
        capability: &Capability,
    ) -> Vec<stmt::Expr> {
        let stmt::Expr::Value(stmt::Value::List(values)) = list else {
            unreachable!()
        };

        let mut seen = ValueSet::with_capacity(values.len());
        values
            .into_iter()
            .filter(|v| !v.is_null())
            .filter(|v| seen.insert(v.clone()))
            .filter_map(|v| {
                let mut pred = stmt::Expr::binary_op(expr.clone(), stmt::BinaryOp::Eq, v);
                simplify::simplify_expr(cx, capability, &mut pred);
                (!pred.is_unsatisfiable()).then_some(pred)
            })
            .collect()
    }
}
