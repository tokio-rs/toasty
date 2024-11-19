use super::*;

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
        model: &'a Model,
        filter: &'a stmt::Expr,
    ) -> IndexPlan<'a> {
        let mut index_planner = IndexPlanner {
            model,
            filter,
            index_matches: vec![],
            index_paths: vec![],
        };

        let index_path = index_planner.plan_index_path();
        let capability = match self.capability {
            capability::Capability::KeyValue(capability) => capability,
            // Just make it work for now
            _ => &capability::KeyValue {
                primary_key_ne_predicate: true,
            },
        };

        let mut cx = PartitionCtx {
            capability,
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
    pub(crate) index: &'a ModelIndex,

    /// Filter to apply to the index
    pub(crate) index_filter: stmt::Expr,

    /// How to filter results after applying the index filter
    pub(crate) result_filter: Option<stmt::Expr>,

    /// True if we have to apply the result filter our self
    pub(crate) post_filter: Option<stmt::Expr>,
}

struct IndexPlanner<'a> {
    model: &'a Model,

    /// Query filter
    filter: &'a stmt::Expr,

    /// Matches clauses in the filter with available indices
    index_matches: Vec<IndexMatch<'a>>,

    /// Possible ways to execute the query using one or more index
    index_paths: Vec<IndexPath>,
}

#[derive(Debug)]
struct IndexMatch<'a> {
    /// The index in question
    index: &'a ModelIndex,

    /// Restriction matches for each field
    fields: Vec<IndexFieldMatch<'a>>,
}

#[derive(Debug)]
struct IndexFieldMatch<'a> {
    exprs: HashMap<ByAddress<&'a stmt::Expr>, ExprMatch>,
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
    capability: &'a capability::KeyValue,
    apply_result_filter_on_results: bool,
}

impl<'a> IndexPlanner<'a> {
    fn plan_index_path(&mut self) -> IndexPath {
        // A preprocessing step that matches filter clauses to various index fields.
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
        for index in &self.model.indices {
            let mut index_match = IndexMatch {
                index,
                fields: index
                    .fields
                    .iter()
                    .map(|_| IndexFieldMatch {
                        exprs: HashMap::new(),
                    })
                    .collect(),
            };

            if !index_match.match_restriction(self.filter) {
                continue;
            }

            // Check if the *first* index field matched any sub expression. If
            // not, the index is not useful to us.
            if index_match.fields[0].exprs.is_empty() {
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

impl<'a> IndexMatch<'a> {
    fn match_restriction(&mut self, expr: &'a stmt::Expr) -> bool {
        use stmt::Expr::*;

        match expr {
            BinaryOp(e) => match (&*e.lhs, &*e.rhs) {
                (Project(lhs), Value(..)) => self.match_expr_binary_op_project(lhs, expr, e.op),
                (Value(..), Project(rhs)) => {
                    let mut op = e.op;
                    op.reverse();

                    self.match_expr_binary_op_project(rhs, expr, op)
                }
                _ => todo!("expr={:#?}", expr),
            },
            InList(e) => self.match_expr_in_list(&*e.expr, expr),
            And(and_exprs) => {
                let matched = self.match_all_restrictions(and_exprs);

                if matched {
                    // Union all matched fields for each operand
                    for field in &mut self.fields {
                        for and_expr in and_exprs {
                            if let Some(operand_match) =
                                field.exprs.get(&ByAddress(and_expr)).copied()
                            {
                                match field.exprs.entry(ByAddress(expr)) {
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
                // balanced for each field. To do this, if operand matched, then
                // we do a deeper search to see if any index field matched with
                // *all* or branches.
                if matched_any_expr {
                    'fields: for field in &mut self.fields {
                        let mut eq = true;

                        for or_expr in or_exprs {
                            if let Some(operand_match) = field.exprs.get(&ByAddress(or_expr)) {
                                eq &= operand_match.eq;
                            } else {
                                continue 'fields;
                            }
                        }

                        // At this point, we verified *each* operand matched a
                        // field, so we can consider the entire expression as
                        // having matched that field.
                        field.exprs.insert(ByAddress(expr), ExprMatch { eq });
                        matched_operand = true;
                    }
                }

                matched_operand
            }
            _ => todo!("expr={:#?}", expr),
        }
    }

    /// Returns true if **any** expression in the provided list match with
    /// **any** index field.
    fn match_all_restrictions(&mut self, exprs: &'a [stmt::Expr]) -> bool {
        let mut matched = false;

        for expr in exprs {
            matched |= self.match_restriction(expr);
        }

        matched
    }

    fn match_expr_in_list(&mut self, lhs: &'a stmt::Expr, expr: &'a stmt::Expr) -> bool {
        match lhs {
            stmt::Expr::Project(path) => {
                self.match_expr_binary_op_project(path, expr, stmt::BinaryOp::Eq)
            }
            stmt::Expr::Record(expr_record) => {
                let mut matched = false;

                for sub_expr in expr_record {
                    let stmt::Expr::Project(path) = sub_expr else {
                        todo!()
                    };
                    matched |=
                        self.match_expr_binary_op_project(path, sub_expr, stmt::BinaryOp::Eq);
                }

                if matched {
                    for field in &mut self.fields {
                        for sub_expr in expr_record {
                            if field.exprs.contains_key(&ByAddress(sub_expr)) {
                                field.exprs.insert(ByAddress(expr), ExprMatch { eq: true });

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

    fn match_expr_binary_op_project(
        &mut self,
        project: &stmt::ExprProject,
        expr: &'a stmt::Expr,
        op: stmt::BinaryOp,
    ) -> bool {
        let mut matched = false;

        for (i, index_field) in self.index.fields.iter().enumerate() {
            // Check that the path matches an index field
            if !project.projection.resolves_to(index_field.field) {
                continue;
            }

            // If the index field is scoped as a partition key, then only
            // equality predicates are supported.
            if index_field.scope.is_partition() && !op.is_eq() {
                continue;
            }

            self.fields[i]
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

            for field in &self.fields {
                let Some(expr_match) = field.exprs.get(&ByAddress(filter)) else {
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
            BinaryOp(binary_op) => {
                if self
                    .fields
                    .iter()
                    .any(|f| f.exprs.contains_key(&ByAddress(expr)))
                {
                    if binary_op.op.is_ne() && !ctx.capability.primary_key_ne_predicate {
                        ctx.apply_result_filter_on_results = true;
                        return (true.into(), true.into());
                    }

                    // Normalize the expression to include the path on the LHS
                    let expr = match (&*binary_op.lhs, &*binary_op.rhs) {
                        (Project(_), Value(_)) => expr.clone(),
                        (Value(value), Project(path)) => {
                            let mut op = binary_op.op;
                            op.reverse();

                            stmt::ExprBinaryOp {
                                lhs: Box::new(path.clone().into()),
                                rhs: Box::new(value.clone().into()),
                                op,
                            }
                            .into()
                        }
                        _ => todo!(),
                    };

                    (expr, true.into())
                } else {
                    (true.into(), expr.clone())
                }
            }
            InList(_) => {
                if self
                    .fields
                    .iter()
                    .any(|f| f.exprs.contains_key(&ByAddress(expr)))
                {
                    (expr.clone(), true.into())
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
                    _ => stmt::ExprOr {
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
