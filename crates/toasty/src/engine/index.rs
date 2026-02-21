mod index_match;
use index_match::{IndexColumnMatch, IndexMatch};

mod or_rewrite;

mod index_plan;
pub(crate) use index_plan::IndexPlan;

use crate::{engine::Engine, Result};
use std::collections::HashMap;
use toasty_core::{
    driver::Capability,
    schema::db::{Index, Table},
    stmt, Schema,
};

impl Engine {
    pub(crate) fn plan_index_path<'a>(&'a self, stmt: &stmt::Statement) -> Result<IndexPlan<'a>> {
        plan_index_path(&self.schema, self.capability(), stmt)
    }
}

pub(crate) fn plan_index_path<'a, 'stmt>(
    schema: &'a Schema,
    capability: &'a Capability,
    stmt: &'stmt stmt::Statement,
) -> Result<IndexPlan<'a>> {
    let cx = stmt::ExprContext::new(schema);
    let cx = cx.scope(stmt);
    // Get a handle to the expression target so it can be passed into the planner
    let target = cx.target();
    let stmt::ExprTarget::Table(table) = target else {
        todo!("target={target:#?}")
    };

    // Get the statement filter
    let filter = stmt.filter_expr_unwrap();

    let mut index_planner = IndexPlanner {
        cx,
        table,
        filter,
        index_matches: vec![],
        index_paths: vec![],
    };

    let index_path = index_planner.plan_index_path()?;

    let mut partition_cx = PartitionCtx {
        capability,
        apply_result_filter_on_results: false,
    };

    let index_match = &index_planner.index_matches[index_path.index_match];
    let (index_filter, result_filter) = index_match.partition_filter(&mut partition_cx, filter);

    // Extract literal key values before OR rewrite, while index_filter is still
    // in Expr::Or form. After rewrite it becomes ANY(MAP(...)) and the Or arm
    // in try_extract_key_values would no longer fire.
    let key_values = try_extract_key_values(&index_planner.cx, index_match.index, &index_filter);

    // For backends that do not support OR in key conditions (e.g. DynamoDB), rewrite
    // any OR in the index filter to canonical ANY(MAP(...)) fan-out form.
    let index_filter = if !capability.index_or_predicate {
        or_rewrite::index_filter_to_any_map(index_filter)
    } else {
        index_filter
    };

    let index = schema.db.index(index_match.index.id);
    let has_pk_keys = index.primary_key && key_values.is_some();

    Ok(IndexPlan {
        // Reload the index to make lifetimes happy.
        index,
        index_filter,
        result_filter: if result_filter.is_true() {
            None
        } else {
            Some(result_filter)
        },
        post_filter: if partition_cx.apply_result_filter_on_results {
            Some(filter.clone())
        } else {
            None
        },
        key_values,
        has_pk_keys,
    })
}

struct IndexPlanner<'stmt> {
    cx: stmt::ExprContext<'stmt>,

    table: &'stmt Table,

    /// Query filter
    filter: &'stmt stmt::Expr,

    /// Matches clauses in the filter with available indices
    index_matches: Vec<IndexMatch<'stmt>>,

    /// Possible ways to execute the query using one or more index
    index_paths: Vec<IndexPath>,
}

#[derive(Debug, Clone)]
struct IndexPath {
    index_match: usize,
    cost: usize,
}

struct PartitionCtx<'a> {
    capability: &'a Capability,
    apply_result_filter_on_results: bool,
}

impl IndexPlanner<'_> {
    fn plan_index_path(&mut self) -> Result<IndexPath> {
        // A preprocessing step that matches filter clauses to various index columns.
        self.build_index_matches();

        // Populate index_paths with possible ways to execute the query.
        self.build_single_index_paths();

        if self.index_paths.is_empty() {
            return Err(toasty_core::Error::unsupported_feature(
                "This database requires queries to use an index. The current filter cannot be \
                 satisfied by any available index. Consider adding an index that matches your \
                 query filter, or restructure the query to use indexed fields.",
            ));
        }

        Ok(self
            .index_paths
            .iter()
            .min_by_key(|index_path| index_path.cost)
            .unwrap()
            .clone())
    }

    fn build_index_matches(&mut self) {
        for index in &self.table.indices {
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

            if !index_match.match_restriction(&self.cx, self.filter) {
                continue;
            }

            // Check if the *first* index column matched any sub expression. If
            // not, the index is not useful to us.
            if index_match.columns[0].exprs.is_empty() {
                continue;
            }

            // The index might be useful, so track it
            self.index_matches.push(index_match);
        }
    }

    fn build_single_index_paths(&mut self) {
        let mut index_paths = vec![];

        for (i, index_match) in self.index_matches.iter().enumerate() {
            index_paths.push(IndexPath {
                cost: index_match.compute_cost(self.filter),
                index_match: i,
            });
        }

        self.index_paths = index_paths;
    }
}

/// Try to extract a key expression from `index_filter` for direct `GetByKey` routing.
///
/// Returns `Some(Expr::Value(Value::List([Value::Record([...]), ...])))` when all key
/// columns have literal equality or IN predicates. Returns `Some(Expr::Arg(i))` for
/// `pk IN (arg[i])` batch-load form. Range predicates and `ANY(MAP(...))` return `None`.
///
/// Must be called on the `index_filter` produced by `partition_filter` — before the
/// OR-rewrite step converts `Expr::Or` into `ANY(MAP(...))`.
fn try_extract_key_values(
    cx: &stmt::ExprContext<'_>,
    index: &Index,
    index_filter: &stmt::Expr,
) -> Option<stmt::Expr> {
    match index_filter {
        stmt::Expr::InList(in_list) => match &*in_list.list {
            stmt::Expr::Arg(arg) => Some(stmt::Expr::Arg(arg.clone())),
            stmt::Expr::Value(stmt::Value::List(items)) => {
                let records = items
                    .iter()
                    .map(|item| match item {
                        record @ stmt::Value::Record(_) => record.clone(),
                        value => stmt::Value::Record(stmt::ValueRecord::from_vec(vec![
                            value.clone()
                        ])),
                    })
                    .collect();
                Some(stmt::Expr::Value(stmt::Value::List(records)))
            }
            _ => None,
        },
        stmt::Expr::Or(or) => {
            let mut records = vec![];
            for branch in &or.operands {
                records.push(extract_key_record(cx, index, branch)?);
            }
            Some(stmt::Expr::Value(stmt::Value::List(records)))
        }
        single => {
            let record = extract_key_record(cx, index, single)?;
            Some(stmt::Expr::Value(stmt::Value::List(vec![record])))
        }
    }
}

/// Extract a single `Value::Record` from one equality branch of the index filter.
///
/// - `col = literal` (single-column index) → `Value::Record([literal])`
/// - `col1 = v1 AND col2 = v2 ...` (all key columns, all equality) → `Value::Record([v1, v2, ...])`
/// - Anything else → `None`
fn extract_key_record(
    cx: &stmt::ExprContext<'_>,
    index: &Index,
    expr: &stmt::Expr,
) -> Option<stmt::Value> {
    match expr {
        stmt::Expr::BinaryOp(b) if b.op.is_eq() && index.columns.len() == 1 => {
            let stmt::Expr::Value(v) = &*b.rhs else {
                return None;
            };
            Some(stmt::Value::Record(stmt::ValueRecord::from_vec(vec![
                v.clone()
            ])))
        }
        stmt::Expr::And(and) if and.operands.len() == index.columns.len() => {
            let mut fields = vec![stmt::Value::Null; index.columns.len()];

            for operand in &and.operands {
                let stmt::Expr::BinaryOp(b) = operand else {
                    return None;
                };
                if !b.op.is_eq() {
                    return None;
                }
                let stmt::Expr::Reference(expr_ref) = &*b.lhs else {
                    return None;
                };
                let column = cx.resolve_expr_reference(expr_ref).expect_column();
                let (idx, _) = index
                    .columns
                    .iter()
                    .enumerate()
                    .find(|(_, c)| c.column == column.id)?;
                let stmt::Expr::Value(v) = &*b.rhs else {
                    return None;
                };
                fields[idx] = v.clone();
            }

            if fields.iter().any(|v| matches!(v, stmt::Value::Null)) {
                return None;
            }

            Some(stmt::Value::Record(stmt::ValueRecord::from_vec(fields)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
