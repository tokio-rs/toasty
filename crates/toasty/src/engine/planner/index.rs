use super::*;

use db::{Index, Table};

use by_address::ByAddress;
use std::collections::hash_map;

/*
1) Match all restrictions with related indices.
2) Compute top-level index paths
3) If no index path matches full restriction, and there are ORs, try multiple paths.
 */

impl<'a> Planner<'a> {
    pub(crate) fn plan_index_path2(
        &mut self,
        table: &'a Table,
        filter: &stmt::Expr,
    ) -> IndexPlan<'a> {
        let mut index_planner = IndexPlanner {
            table,
            filter,
            index_matches: vec![],
            index_paths: vec![],
        };

        let index_path = index_planner.plan_index_path();

        let mut cx = PartitionCtx {
            capability: self.capability,
            apply_result_filter_on_results: false,
        };

        let index_match = &index_planner.index_matches[index_path.index_match];
        let (index_filter, result_filter) = index_match.partition_filter(&mut cx, filter);

        IndexPlan {
            index: index_match.index,
            index_filter,
            result_filter: if result_filter.is_true() {
                None
            } else {
                Some(result_filter)
            },
            // TODO: not actually correct
            post_filter: if cx.apply_result_filter_on_results {
                Some(filter.clone())
            } else {
                None
            },
        }
    }
}

#[derive(Debug)]
pub(crate) struct IndexPlan<'a> {
    /// The index to use to execute the query
    pub(crate) index: &'a Index,

    /// Filter to apply to the index
    pub(crate) index_filter: stmt::Expr,

    /// How to filter results after applying the index filter
    pub(crate) result_filter: Option<stmt::Expr>,

    /// True if we have to apply the result filter our self
    pub(crate) post_filter: Option<stmt::Expr>,
}

struct IndexPlanner<'a, 'stmt> {
    table: &'a Table,

    /// Query filter
    filter: &'stmt stmt::Expr,

    /// Matches clauses in the filter with available indices
    index_matches: Vec<IndexMatch<'a, 'stmt>>,

    /// Possible ways to execute the query using one or more index
    index_paths: Vec<IndexPath>,
}

#[derive(Debug)]
struct IndexMatch<'a, 'stmt> {
    /// The index in question
    index: &'a Index,

    /// Restriction matches for each column
    columns: Vec<IndexColumnMatch<'stmt>>,
}

#[derive(Debug)]
struct IndexColumnMatch<'stmt> {
    exprs: HashMap<ByAddress<&'stmt stmt::Expr>, ExprMatch>,
}

#[derive(Debug, Copy, Clone)]
struct ExprMatch {
    eq: bool,
}

#[derive(Debug, Clone)]
struct IndexPath {
    index_match: usize,
    cost: usize,
}

type ExprPair = (stmt::Expr, stmt::Expr);

struct PartitionCtx<'a> {
    capability: &'a Capability,
    apply_result_filter_on_results: bool,
}

impl IndexPlanner<'_, '_> {
    fn plan_index_path(&mut self) -> IndexPath {
        // A preprocessing step that matches filter clauses to various index columns.
        self.build_index_matches();

        // Populate index_paths with possible ways to execute the query.
        self.build_single_index_paths();

        if self.index_paths.is_empty() {
            todo!("check OR options; matches={:#?}", self.index_matches);
        }

        self.index_paths
            .iter()
            .min_by_key(|index_path| index_path.cost)
            .unwrap()
            .clone()
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

            if !index_match.match_restriction(self.filter) {
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

impl<'stmt> IndexMatch<'_, 'stmt> {
    fn match_restriction(&mut self, expr: &'stmt stmt::Expr) -> bool {
        use stmt::Expr::*;

        match expr {
            Pattern(stmt::ExprPattern::BeginsWith(e)) => {
                debug_assert!(e.pattern.is_value(), "TODO");

                // Equivalent to a binary op with a `<=` operator.
                match &*e.expr {
                    Column(expr_column) => {
                        let m =
                            self.match_expr_binary_op_column(expr_column, expr, stmt::BinaryOp::Le);
                        assert!(m, "TODO; expr={expr:#?}");
                        m
                    }
                    _ => todo!("expr={:#?}", expr),
                }
            }
            BinaryOp(e) => match (&*e.lhs, &*e.rhs) {
                (Column(lhs), Value(..)) => self.match_expr_binary_op_column(lhs, expr, e.op),
                (Value(..), Column(rhs)) => {
                    let mut op = e.op;
                    op.reverse();

                    self.match_expr_binary_op_column(rhs, expr, op)
                }
                _ => todo!("expr={:#?}", expr),
            },
            InList(e) => self.match_expr_in_list(&e.expr, expr),
            IsNull(e) => match &*e.expr {
                Column(expr_column) => {
                    self.match_expr_binary_op_column(expr_column, expr, stmt::BinaryOp::Eq)
                }
                _ => todo!("expr={:#?}", expr),
            },
            And(and_exprs) => {
                let matched = self.match_all_restrictions(and_exprs);

                if matched {
                    // Union all matched columns for each operand
                    for column in &mut self.columns {
                        for and_expr in and_exprs {
                            if let Some(operand_match) =
                                column.exprs.get(&ByAddress(and_expr)).copied()
                            {
                                match column.exprs.entry(ByAddress(expr)) {
                                    hash_map::Entry::Vacant(e) => {
                                        e.insert(ExprMatch {
                                            eq: operand_match.eq,
                                        });
                                    }
                                    hash_map::Entry::Occupied(mut e) if operand_match.eq => {
                                        e.get_mut().eq = true;
                                    }
                                    _ => continue,
                                }

                                if operand_match.eq {
                                    break;
                                }
                            }
                        }
                    }
                }

                matched
            }
            Or(or_exprs) => {
                let matched_any_expr = self.match_all_restrictions(or_exprs);
                let mut matched_operand = false;

                // For OR expressions, we need to ensure that the index match is
                // balanced for each column. To do this, if operand matched, then
                // we do a deeper search to see if any index column matched with
                // *all* or branches.
                if matched_any_expr {
                    'columns: for column in &mut self.columns {
                        let mut eq = true;

                        for or_expr in or_exprs {
                            if let Some(operand_match) = column.exprs.get(&ByAddress(or_expr)) {
                                eq &= operand_match.eq;
                            } else {
                                continue 'columns;
                            }
                        }

                        // At this point, we verified *each* operand matched a
                        // column, so we can consider the entire expression as
                        // having matched that column.
                        column.exprs.insert(ByAddress(expr), ExprMatch { eq });
                        matched_operand = true;
                    }
                }

                matched_operand
            }
            _ => todo!("expr={:#?}", expr),
        }
    }

    /// Returns true if **any** expression in the provided list match with
    /// **any** index column.
    fn match_all_restrictions(&mut self, exprs: &'stmt [stmt::Expr]) -> bool {
        let mut matched = false;

        for expr in exprs {
            matched |= self.match_restriction(expr);
        }

        matched
    }

    fn match_expr_in_list(&mut self, lhs: &'stmt stmt::Expr, expr: &'stmt stmt::Expr) -> bool {
        match lhs {
            stmt::Expr::Column(expr_column) => {
                self.match_expr_binary_op_column(expr_column, expr, stmt::BinaryOp::Eq)
            }
            stmt::Expr::Record(expr_record) => {
                let mut matched = false;

                for sub_expr in expr_record {
                    let stmt::Expr::Column(expr_column) = sub_expr else {
                        todo!()
                    };
                    matched |=
                        self.match_expr_binary_op_column(expr_column, sub_expr, stmt::BinaryOp::Eq);
                }

                if matched {
                    for column in &mut self.columns {
                        for sub_expr in expr_record {
                            if column.exprs.contains_key(&ByAddress(sub_expr)) {
                                column.exprs.insert(ByAddress(expr), ExprMatch { eq: true });

                                break;
                            }
                        }
                    }
                }

                matched
            }
            _ => todo!("expr={:#?}", expr),
        }
    }

    fn match_expr_binary_op_column(
        &mut self,
        expr_column: &stmt::ExprColumn,
        expr: &'stmt stmt::Expr,
        op: stmt::BinaryOp,
    ) -> bool {
        let mut matched = false;

        for (i, index_column) in self.index.columns.iter().enumerate() {
            // Check that the path matches an index column
            if !expr_column.references(index_column.column) {
                continue;
            }

            // If the index column is scoped as a partition key, then only
            // equality predicates are supported.
            if index_column.scope.is_partition() && !op.is_eq() {
                continue;
            }

            self.columns[i]
                .exprs
                .insert(ByAddress(expr), ExprMatch { eq: op.is_eq() });

            matched = true;
        }

        matched
    }

    /// Copute the cost of using this index match to execute the query.
    fn compute_cost(&self, filter: &stmt::Expr) -> usize {
        // TODO: factor in post query in-memory filtering.
        if self.index.unique {
            let mut cost = 0;

            for column in &self.columns {
                let Some(expr_match) = column.exprs.get(&ByAddress(filter)) else {
                    break;
                };

                if expr_match.eq {
                    cost += 1;
                } else {
                    cost += 10;
                }
            }

            cost
        } else {
            // Arbitrary
            10
        }
    }

    fn partition_filter(&self, ctx: &mut PartitionCtx<'_>, expr: &stmt::Expr) -> ExprPair {
        use stmt::Expr::*;

        match expr {
            Pattern(stmt::ExprPattern::BeginsWith(_)) | InList(_) | IsNull(_) => {
                if self
                    .columns
                    .iter()
                    .any(|f| f.exprs.contains_key(&ByAddress(expr)))
                {
                    (expr.clone(), true.into())
                } else {
                    (true.into(), expr.clone())
                }
            }
            BinaryOp(binary_op) => {
                if self
                    .columns
                    .iter()
                    .any(|f| f.exprs.contains_key(&ByAddress(expr)))
                {
                    if binary_op.op.is_ne() && !ctx.capability.primary_key_ne_predicate {
                        ctx.apply_result_filter_on_results = true;
                        return (true.into(), true.into());
                    }

                    // Normalize the expression to include the column on the LHS
                    // TODO: is this needed?
                    let expr = match (&*binary_op.lhs, &*binary_op.rhs) {
                        (Column(_), Value(_)) => expr.clone(),
                        (Value(value), Column(path)) => {
                            let mut op = binary_op.op;
                            op.reverse();

                            stmt::ExprBinaryOp {
                                lhs: Box::new(path.clone().into()),
                                rhs: Box::new(value.clone().into()),
                                op,
                            }
                            .into()
                        }
                        _ => todo!("binary_op={binary_op:#?}"),
                    };

                    (expr, true.into())
                } else {
                    (true.into(), expr.clone())
                }
            }
            And(expr_and) => {
                let mut index_filter_operands = vec![];
                let mut result_filter_operands = vec![];

                for operand in &expr_and.operands {
                    let (index_filter, result_filter) = self.partition_filter(ctx, operand);

                    if !index_filter.is_true() {
                        index_filter_operands.push(index_filter);
                    }

                    if !result_filter.is_true() {
                        result_filter_operands.push(result_filter);
                    }
                }

                let index_filter = match index_filter_operands.len() {
                    0 => true.into(),
                    1 => index_filter_operands.into_iter().next().unwrap(),
                    _ => stmt::ExprAnd {
                        operands: index_filter_operands,
                    }
                    .into(),
                };
                let result_filter = match result_filter_operands.len() {
                    0 => true.into(),
                    1 => result_filter_operands.into_iter().next().unwrap(),
                    _ => stmt::ExprAnd {
                        operands: result_filter_operands,
                    }
                    .into(),
                };

                (index_filter, result_filter)
            }
            Or(expr_or) => {
                let mut index_filter_operands = vec![];
                let mut result_filter_operands = vec![];

                for operand in &expr_or.operands {
                    let (index_filter, result_filter) = self.partition_filter(ctx, operand);

                    if !index_filter.is_true() && !result_filter.is_true() {
                        todo!("expr_or={:#?}", expr_or);
                    }

                    if !index_filter.is_true() {
                        assert!(result_filter_operands.is_empty());
                        index_filter_operands.push(index_filter);
                    } else if !result_filter.is_true() {
                        assert!(index_filter_operands.is_empty());
                        result_filter_operands.push(result_filter);
                    } else {
                        todo!()
                    }
                }

                if !index_filter_operands.is_empty() {
                    (
                        stmt::ExprOr {
                            operands: index_filter_operands,
                        }
                        .into(),
                        true.into(),
                    )
                } else {
                    (
                        true.into(),
                        stmt::ExprOr {
                            operands: result_filter_operands,
                        }
                        .into(),
                    )
                }
            }
            _ => todo!("partition_filter={:#?}", expr),
        }
    }
}
