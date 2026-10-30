use toasty_core::{
    driver::Capability, schema::db::TableId, stmt, stmt::ExprContext, stmt::ValueSet,
};

use crate::engine::simplify;

use super::Exec;

impl Exec<'_> {
    /// Bind a key-value row filter to its operation's input values.
    ///
    /// Returns `false` when the bound filter matches no row, so the caller
    /// can skip the driver call. A filter that binds to `true` is removed.
    pub(super) fn bind_row_filter(
        &self,
        filter: &mut Option<stmt::Expr>,
        input: &[stmt::Value],
        table: TableId,
    ) -> bool {
        let Some(expr) = filter else {
            return true;
        };

        // An absent candidate key matches no row. Dropping it keeps the
        // key-value comparison two-valued, so a negated membership still
        // admits every other row. Only lists bound from the input carry
        // candidate keys; constant lists in the filter are left as written.
        stmt::visit_mut::for_each_expr_mut(expr, |expr| {
            if let stmt::Expr::InList(in_list) = expr
                && is_input_arg(&in_list.list)
            {
                in_list.list.substitute(input);

                if let stmt::Expr::Value(stmt::Value::List(values)) = &mut *in_list.list {
                    values.retain(|value| !is_absent_key(value));
                }
            }
        });

        expr.substitute(input);

        // Bound inputs can reduce the filter to a constant, e.g. an empty
        // candidate list turns `x IN ()` into `false`.
        let db_table = self.engine.schema.db.table(table);
        let cx = self.engine.expr_cx_for(db_table);
        simplify::simplify_expr(cx, self.engine.capability, expr);

        if expr.is_unsatisfiable() {
            return false;
        }

        if expr.is_true() {
            *filter = None;
        }

        true
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
            .filter(|v| !is_absent_key(v))
            .filter(|v| seen.insert(v.clone()))
            .filter_map(|v| {
                let mut pred = stmt::Expr::binary_op(expr.clone(), stmt::BinaryOp::Eq, v);
                simplify::simplify_expr(cx, capability, &mut pred);
                (!pred.is_unsatisfiable()).then_some(pred)
            })
            .collect()
    }
}

/// Whether `expr` is a reference to the operation's input, possibly projected.
fn is_input_arg(expr: &stmt::Expr) -> bool {
    match expr {
        stmt::Expr::Arg(_) => true,
        stmt::Expr::Project(project) => matches!(&*project.base, stmt::Expr::Arg(_)),
        _ => false,
    }
}

/// Whether a key value is absent: null itself, or a composite key with any
/// null component. An absent key identifies no row, and key-value drivers
/// reject null key attributes, so key lookups skip it.
pub(super) fn is_absent_key(value: &stmt::Value) -> bool {
    match value {
        stmt::Value::Null => true,
        stmt::Value::Record(record) => record.fields.iter().any(is_absent_key),
        _ => false,
    }
}
