use std::collections::{hash_map, HashMap};

use by_address::ByAddress;
use toasty_core::{schema::db::Index, stmt};

use crate::engine::index::PartitionCtx;

#[derive(Debug)]
pub(super) struct IndexMatch<'stmt> {
    /// The index in question
    pub(super) index: &'stmt Index,

    /// Restriction matches for each column
    pub(super) columns: Vec<IndexColumnMatch<'stmt>>,
}

#[derive(Debug)]
pub(super) struct IndexColumnMatch<'stmt> {
    pub(super) exprs: HashMap<ByAddress<&'stmt stmt::Expr>, ExprMatch>,
}

#[derive(Debug, Copy, Clone)]
pub(super) struct ExprMatch {
    pub(super) eq: bool,
}

type ExprPair = (stmt::Expr, stmt::Expr);

impl<'stmt> IndexMatch<'stmt> {
    pub(super) fn match_restriction(
        &mut self,
        cx: &stmt::ExprContext<'_>,
        expr: &'stmt stmt::Expr,
    ) -> bool {
        use stmt::Expr::*;

        match expr {
            Pattern(stmt::ExprPattern::BeginsWith(e)) => {
                debug_assert!(e.pattern.is_value(), "TODO");

                // Equivalent to a binary op with a `<=` operator.
                match &*e.expr {
                    stmt::Expr::Reference(expr_column @ stmt::ExprReference::Column(_)) => {
                        let m = self.match_expr_binary_op_column(
                            cx,
                            expr_column,
                            expr,
                            stmt::BinaryOp::Le,
                        );
                        assert!(m, "TODO; expr={expr:#?}");
                        m
                    }
                    _ => todo!("expr={:#?}", expr),
                }
            }
            BinaryOp(e) => match (&*e.lhs, &*e.rhs) {
                (stmt::Expr::Reference(lhs @ stmt::ExprReference::Column(_)), rhs) => {
                    assert!(
                        !rhs.is_expr_reference(),
                        "TODO: handle ExprReference on both sides"
                    );
                    self.match_expr_binary_op_column(cx, lhs, expr, e.op)
                }
                (_, stmt::Expr::Reference(rhs @ stmt::ExprReference::Column(_))) => {
                    let mut op = e.op;
                    op.reverse();

                    self.match_expr_binary_op_column(cx, rhs, expr, op)
                }
                _ => todo!("expr={:#?}", expr),
            },
            InList(e) => self.match_expr_in_list(cx, &e.expr, expr),
            IsNull(e) => match &*e.expr {
                stmt::Expr::Reference(expr_column @ stmt::ExprReference::Column(_)) => {
                    self.match_expr_binary_op_column(cx, expr_column, expr, stmt::BinaryOp::Eq)
                }
                _ => todo!("expr={:#?}", expr),
            },
            And(and_exprs) => {
                let matched = self.match_all_restrictions(cx, and_exprs);

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
                let matched_any_expr = self.match_all_restrictions(cx, or_exprs);
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
            Any(expr_any) => {
                // Any operates on a Map expression that evaluates to bools.
                // It's similar to a dynamic OR expression. If the map expression
                // matches an index, the Any expression matches as well.
                // We don't need the "balanced" check like OR because each
                // evaluation of the map will have the same structure.
                let stmt::Expr::Map(expr_map) = &*expr_any.expr else {
                    todo!("expr_any.expr={:#?}", expr_any.expr);
                };

                // Try to match the map expression (the predicate applied to each element)
                if self.match_restriction(cx, &expr_map.map) {
                    // If the map expression matched, propagate that match to this Any expression
                    for column in &mut self.columns {
                        if let Some(operand_match) =
                            column.exprs.get(&ByAddress(&*expr_map.map)).copied()
                        {
                            column.exprs.insert(
                                ByAddress(expr),
                                ExprMatch {
                                    eq: operand_match.eq,
                                },
                            );
                        }
                    }
                    true
                } else {
                    false
                }
            }
            _ => {
                // Unsupported expression type for index matching - return false
                // to indicate this expression cannot be matched against an index
                false
            }
        }
    }

    /// Returns true if **any** expression in the provided list match with
    /// **any** index column.
    fn match_all_restrictions(
        &mut self,
        cx: &stmt::ExprContext<'_>,
        exprs: &'stmt [stmt::Expr],
    ) -> bool {
        let mut matched = false;

        for expr in exprs {
            matched |= self.match_restriction(cx, expr);
        }

        matched
    }

    fn match_expr_in_list(
        &mut self,
        cx: &stmt::ExprContext<'_>,
        lhs: &'stmt stmt::Expr,
        expr: &'stmt stmt::Expr,
    ) -> bool {
        match lhs {
            stmt::Expr::Reference(expr_column @ stmt::ExprReference::Column(_)) => {
                self.match_expr_binary_op_column(cx, expr_column, expr, stmt::BinaryOp::Eq)
            }
            stmt::Expr::Record(expr_record) => {
                let mut matched = false;

                for sub_expr in expr_record {
                    let stmt::Expr::Reference(expr_column @ stmt::ExprReference::Column(_)) =
                        sub_expr
                    else {
                        todo!()
                    };
                    matched |= self.match_expr_binary_op_column(
                        cx,
                        expr_column,
                        sub_expr,
                        stmt::BinaryOp::Eq,
                    );
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
        cx: &stmt::ExprContext<'_>,
        expr_ref: &stmt::ExprReference,
        expr: &'stmt stmt::Expr,
        op: stmt::BinaryOp,
    ) -> bool {
        let mut matched = false;

        for (i, index_column) in self.index.columns.iter().enumerate() {
            // Check that the path matches an index column
            if cx.resolve_expr_reference(expr_ref).expect_column().id != index_column.column {
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
    pub(super) fn compute_cost(&self, filter: &stmt::Expr) -> usize {
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

    pub(super) fn partition_filter(
        &self,
        ctx: &mut PartitionCtx<'_>,
        expr: &stmt::Expr,
    ) -> ExprPair {
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
                    let expr = match (&*binary_op.lhs, &*binary_op.rhs) {
                        (stmt::Expr::Reference(stmt::ExprReference::Column(_)), _) => expr.clone(),
                        (
                            lhs,
                            stmt::Expr::Reference(column_ref @ stmt::ExprReference::Column(_)),
                        ) => {
                            let mut op = binary_op.op;
                            op.reverse();

                            stmt::ExprBinaryOp {
                                lhs: Box::new(stmt::Expr::Reference(*column_ref)),
                                rhs: Box::new(lhs.clone()),
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
            Any(expr_any) => {
                // For Any expressions, partition the inner map expression
                let stmt::Expr::Map(expr_map) = &*expr_any.expr else {
                    todo!("expr_any.expr={:#?}", expr_any.expr);
                };

                let (index_filter, result_filter) = self.partition_filter(ctx, &expr_map.map);

                // If the map expression can be used as an index filter, reconstruct
                // the Any with the index filter version
                if !index_filter.is_true() {
                    let index_any = stmt::ExprAny {
                        expr: Box::new(stmt::Expr::Map(stmt::ExprMap {
                            base: expr_map.base.clone(),
                            map: Box::new(index_filter),
                        })),
                    };
                    (index_any.into(), true.into())
                } else if !result_filter.is_true() {
                    let result_any = stmt::ExprAny {
                        expr: Box::new(stmt::Expr::Map(stmt::ExprMap {
                            base: expr_map.base.clone(),
                            map: Box::new(result_filter),
                        })),
                    };
                    (true.into(), result_any.into())
                } else {
                    (true.into(), true.into())
                }
            }
            _ => todo!("partition_filter={:#?}", expr),
        }
    }
}
