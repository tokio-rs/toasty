mod index_match;
use index_match::{IndexColumnMatch, IndexMatch};

mod index_plan;
pub(crate) use index_plan::IndexPlan;

use crate::{engine::Engine, Result};
use std::collections::HashMap;
use toasty_core::{driver::Capability, schema::db::Table, stmt};

impl Engine {
    pub(crate) fn plan_index_path<'a>(&'a self, stmt: &stmt::Statement) -> Result<IndexPlan<'a>> {
        let cx = self.expr_cx();
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

        let mut cx = PartitionCtx {
            capability: self.capability(),
            apply_result_filter_on_results: false,
        };

        let index_match = &index_planner.index_matches[index_path.index_match];
        let (index_filter, result_filter) = index_match.partition_filter(&mut cx, filter);

        Ok(IndexPlan {
            // Reload the index to make lifetimes happy.
            index: self.schema.db.index(index_match.index.id),
            index_filter,
            result_filter: if result_filter.is_true() {
                None
            } else {
                Some(result_filter)
            },
            post_filter: if cx.apply_result_filter_on_results {
                Some(filter.clone())
            } else {
                None
            },
        })
    }
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
