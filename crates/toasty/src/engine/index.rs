mod index_match;
use index_match::{IndexColumnMatch, IndexMatch};

mod or_rewrite;

mod index_plan;
pub(crate) use index_plan::IndexPlan;

use crate::{Result, engine::Engine};
use hashbrown::HashMap;
use toasty_core::{Schema, driver::Capability, schema::db::Index, stmt};

impl Engine {
    pub(crate) fn plan_index_path<'a>(
        &'a self,
        stmt: &stmt::Statement,
    ) -> Result<Option<IndexPlan<'a>>> {
        plan_index_path(&self.schema, self.capability(), stmt)
    }
}

pub(crate) fn plan_index_path<'a>(
    schema: &'a Schema,
    capability: &'a Capability,
    stmt: &stmt::Statement,
) -> Result<Option<IndexPlan<'a>>> {
    let cx = stmt::ExprContext::new(schema);
    let cx = cx.scope(stmt);
    // Get a handle to the expression target so it can be passed into the planner
    let target = cx.target();
    let stmt::ExprTarget::Table(table) = target else {
        todo!("target={target:#?}")
    };

    // Get the statement filter
    let filter = stmt.filter_expr_unwrap();

    // Extract the pre-filter: the part of the filter that depends only on
    // args (no table column references) and can be evaluated before issuing
    // the database operation. The invariant is:
    //   filter == AND(pre_filter, remaining_filter)
    let (pre_filter, remaining_filter) = extract_pre_filter(filter);

    let index_match = table
        .indices
        .iter()
        .filter_map(|index| {
            let mut index_match = IndexMatch {
                index,
                columns: index
                    .columns
                    .iter()
                    .map(|_| IndexColumnMatch {
                        exprs: HashMap::new(),
                    })
                    .collect(),
            };

            if !index_match.match_restriction(&cx, &remaining_filter)
                || index_match.columns[0].exprs.is_empty()
            {
                return None;
            }

            Some(index_match)
        })
        .min_by_key(|index_match| index_match.compute_cost(&remaining_filter));

    let Some(index_match) = index_match else {
        if capability.scan {
            return Ok(None);
        }
        return Err(toasty_core::Error::unsupported_feature(format!(
            "{} requires queries to use an index. The current filter cannot be satisfied by \
             any available index. Consider adding an index that matches your query filter, or \
             restructure the query to use indexed fields.",
            capability.driver_name
        )));
    };

    let mut partition_cx = PartitionCtx {
        capability,
        apply_result_filter_on_results: false,
    };

    let (index_filter, mut result_filter) =
        index_match.partition_filter(&mut partition_cx, &remaining_filter);

    // Extract literal key values before OR rewrite, while index_filter is still
    // in Expr::Or form. After rewrite it becomes ANY(MAP(...)) and the Or arm
    // in try_extract_key_values would no longer fire.
    let key_values = match try_extract_key_values(&cx, index_match.index, &index_filter) {
        Some((keys, residual)) if residual.is_true() => Some(keys),
        // A complete literal primary key still permits a direct lookup when
        // another predicate constrains the same key, such as `id = x AND id IN
        // (subquery)`. The residual becomes a result filter. Mutations keep
        // using QueryPk, which binds subquery results before collecting keys;
        // direct mutation operations cannot bind filter args.
        Some((keys, residual)) if stmt.is_query() && index_match.index.primary_key => {
            result_filter = stmt::Expr::and(residual, result_filter);
            Some(keys)
        }
        _ => None,
    };

    // For backends that do not support OR in key conditions (e.g. DynamoDB), rewrite
    // any OR in the index filter to canonical ANY(MAP(...)) fan-out form.
    let index_filter = if !capability.index_or_predicate {
        match or_rewrite::index_filter_to_any_map(index_filter) {
            Some(filter) => filter,
            None if capability.scan && stmt.is_query() => return Ok(None),
            None => {
                return Err(toasty_core::Error::unsupported_feature(
                    "this OR filter requires a full-table scan, which is not supported for this operation",
                ));
            }
        }
    } else {
        index_filter
    };

    let index = schema.db.index(index_match.index.id);
    let has_pk_keys = index.primary_key && key_values.is_some();

    Ok(Some(IndexPlan {
        // Reload the index to make lifetimes happy.
        index,
        index_filter,
        result_filter: if result_filter.is_true() {
            None
        } else {
            Some(result_filter)
        },
        post_filter: if partition_cx.apply_result_filter_on_results {
            Some(remaining_filter.clone())
        } else {
            None
        },
        pre_filter,
        key_values,
        has_pk_keys,
    }))
}

struct PartitionCtx<'a> {
    capability: &'a Capability,
    apply_result_filter_on_results: bool,
}

/// Try to extract a key expression from `index_filter` for direct `GetByKey` routing.
///
/// Returns `Some(Expr::Value(Value::List([Value::Record([...]), ...])))` when all key
/// columns have literal equality or IN predicates. Returns `Some(Expr::Arg(i))` for
/// `pk IN (arg[i])` batch-load form. Range predicates and `ANY(MAP(...))` return `None`.
/// The keys come with the residual predicate they do not capture (`true` if none).
///
/// Must be called on the `index_filter` produced by `partition_filter` — before the
/// OR-rewrite step converts `Expr::Or` into `ANY(MAP(...))`.
fn try_extract_key_values(
    cx: &stmt::ExprContext<'_>,
    index: &Index,
    index_filter: &stmt::Expr,
) -> Option<(stmt::Expr, stmt::Expr)> {
    let mut residual = stmt::Expr::from(true);
    match index_filter {
        stmt::Expr::InList(in_list) => match &*in_list.list {
            stmt::Expr::Arg(arg) => Some(stmt::Expr::Arg(*arg)),
            stmt::Expr::Value(stmt::Value::List(items)) => {
                let records = items
                    .iter()
                    .map(|item| match item {
                        record @ stmt::Value::Record(_) => Some(record.clone()),
                        // Only wrap scalar values as single-field Records for single-column
                        // indexes. For composite indexes the scalar covers only the partition
                        // key, so we cannot form a full key record.
                        value if index.columns.len() == 1 => {
                            Some(stmt::Value::Record(stmt::ValueRecord::from_vec(vec![
                                value.clone(),
                            ])))
                        }
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(stmt::Expr::Value(stmt::Value::List(records)))
            }
            _ => None,
        },
        stmt::Expr::Or(or) => {
            let mut records = vec![];
            for branch in &or.operands {
                let (record, rest) = extract_key_record(cx, index, branch)?;
                records.push(rest.is_true().then_some(record)?);
            }
            Some(stmt::Expr::Value(stmt::Value::List(records)))
        }
        single => {
            let (record, rest) = extract_key_record(cx, index, single)?;
            residual = rest;
            Some(stmt::Expr::Value(stmt::Value::List(vec![record])))
        }
    }
    .map(|keys| (keys, residual))
}

/// Extract a single `Value::Record` from one equality branch of the index filter.
///
/// - `col = literal` (single-column index) → `Value::Record([literal])`
/// - `col1 = v1 AND col2 = v2 ...` (an equality per key column) → `Value::Record([v1, v2, ...])`
/// - Anything else → `None`
///
/// Also returns the residual: the operands not used as key fields.
fn extract_key_record(
    cx: &stmt::ExprContext<'_>,
    index: &Index,
    expr: &stmt::Expr,
) -> Option<(stmt::Value, stmt::Expr)> {
    match expr {
        stmt::Expr::BinaryOp(b) if b.op.is_eq() && index.columns.len() == 1 => {
            let stmt::Expr::Value(v) = &*b.rhs else {
                return None;
            };
            let record = stmt::ValueRecord::from_vec(vec![v.clone()]);
            Some((stmt::Value::Record(record), true.into()))
        }
        stmt::Expr::And(and) => {
            let mut fields = vec![stmt::Value::Null; index.columns.len()];
            let mut residual = vec![];

            for operand in &and.operands {
                // The first equality on each key column fills that field. Any
                // other operand, including a repeat equality on a filled
                // column, stays in the residual.
                match key_column_eq(cx, index, operand) {
                    Some((idx, v)) if fields[idx].is_null() => fields[idx] = v.clone(),
                    _ => residual.push(operand.clone()),
                }
            }

            if fields.iter().any(|v| matches!(v, stmt::Value::Null)) {
                return None;
            }

            let record = stmt::Value::Record(stmt::ValueRecord::from_vec(fields));
            Some((record, stmt::Expr::and_from_vec(residual)))
        }
        _ => None,
    }
}

/// Match `key_column = literal` and return the column's position in `index`
/// with the literal.
///
/// `key_column = NULL` does not match: it is never true, so it must stay in
/// the residual instead of being dropped.
fn key_column_eq<'a>(
    cx: &stmt::ExprContext<'_>,
    index: &Index,
    expr: &'a stmt::Expr,
) -> Option<(usize, &'a stmt::Value)> {
    let stmt::Expr::BinaryOp(b) = expr else {
        return None;
    };
    if !b.op.is_eq() {
        return None;
    }
    let stmt::Expr::Reference(expr_ref) = &*b.lhs else {
        return None;
    };
    let stmt::Expr::Value(value) = &*b.rhs else {
        return None;
    };
    if value.is_null() {
        return None;
    }
    let column = cx.resolve_expr_reference(expr_ref).as_column_unwrap();
    let idx = index.columns.iter().position(|c| c.column == column.id)?;
    Some((idx, value))
}

/// Extract the args-only component of a filter expression.
///
/// Splits the filter into `(pre_filter, remaining_filter)` such that
/// `filter == AND(pre_filter, remaining_filter)`. The `pre_filter` contains
/// only sub-expressions that can be evaluated using args alone (no table
/// column references).
fn extract_pre_filter(expr: &stmt::Expr) -> (Option<stmt::Expr>, stmt::Expr) {
    // Only AND nodes can be split into pre_filter and remaining components.
    let stmt::Expr::And(and) = expr else {
        if references_column(expr) {
            return (None, expr.clone());
        } else {
            return (Some(expr.clone()), true.into());
        }
    };

    let mut pre = vec![];
    let mut remaining = vec![];

    for operand in &and.operands {
        if references_column(operand) {
            remaining.push(operand.clone());
        } else {
            pre.push(operand.clone());
        }
    }

    // If all operands are args-only, the entire AND is the pre_filter.
    if remaining.is_empty() {
        return (Some(expr.clone()), true.into());
    }

    let pre_filter = match pre.len() {
        0 => None,
        1 => Some(pre.into_iter().next().unwrap()),
        _ => Some(stmt::ExprAnd { operands: pre }.into()),
    };

    let remaining_filter = match remaining.len() {
        1 => remaining.into_iter().next().unwrap(),
        _ => stmt::ExprAnd {
            operands: remaining,
        }
        .into(),
    };

    (pre_filter, remaining_filter)
}

/// Returns `true` if the expression references any table column.
fn references_column(expr: &stmt::Expr) -> bool {
    let mut found = false;
    stmt::visit::for_each_expr(expr, |e| {
        if matches!(e, stmt::Expr::Reference(stmt::ExprReference::Column(_))) {
            found = true;
        }
    });
    found
}

#[cfg(test)]
mod tests;
