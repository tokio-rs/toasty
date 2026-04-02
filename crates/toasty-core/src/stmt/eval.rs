//! Client-side evaluation of constant or input-bound expressions and
//! statements.
//!
//! The evaluator walks the expression tree recursively, resolving arguments
//! via an [`Input`] implementation and producing [`Value`]s. It supports
//! boolean logic, comparison, casting, records, lists, let-bindings, match
//! expressions, and subqueries (VALUES only).
//!
//! # Examples
//!
//! ```
//! use toasty_core::stmt::{Expr, Value, ConstInput};
//!
//! let expr = Expr::from(Value::from(42_i64));
//! let result = expr.eval(ConstInput::new()).unwrap();
//! assert_eq!(result, Value::from(42_i64));
//! ```

use crate::{
    Result,
    stmt::{
        BinaryOp, ConstInput, Expr, ExprArg, ExprSet, Input, Limit, Projection, Statement, Value,
    },
};
use std::cmp::Ordering;

enum ScopeStack<'a> {
    Root,
    Scope {
        args: &'a [Value],
        parent: &'a ScopeStack<'a>,
    },
}

impl Statement {
    /// Evaluates this statement using the provided [`Input`] for argument
    /// resolution. Only `Query` statements are supported.
    ///
    /// # Errors
    ///
    /// Returns an error for non-Query statements, or if evaluation of any
    /// sub-expression fails.
    pub fn eval(&self, mut input: impl Input) -> Result<Value> {
        self.eval_ref(&ScopeStack::Root, &mut input)
    }

    /// Evaluates this statement as a constant expression (no external input).
    pub fn eval_const(&self) -> Result<Value> {
        self.eval(ConstInput::new())
    }

    fn eval_ref(&self, scope: &ScopeStack<'_>, input: &mut impl Input) -> Result<Value> {
        match self {
            Statement::Query(query) => {
                if query.with.is_some() {
                    return Err(crate::Error::expression_evaluation_failed(
                        "cannot evaluate statement with WITH clause",
                    ));
                }

                if query.order_by.is_some() {
                    return Err(crate::Error::expression_evaluation_failed(
                        "cannot evaluate statement with ORDER BY clause",
                    ));
                }

                let mut result = query.body.eval_ref(scope, input)?;

                if let Some(limit) = &query.limit {
                    limit.eval_ref(&mut result, scope, input)?;
                }

                if query.single {
                    let Value::List(mut items) = result else {
                        return Err(crate::Error::expression_evaluation_failed(
                            "single-row query requires body to evaluate to a list",
                        ));
                    };
                    if items.len() != 1 {
                        return Err(crate::Error::expression_evaluation_failed(
                            "single-row query did not return exactly one row",
                        ));
                    }
                    return Ok(items.remove(0));
                }

                Ok(result)
            }
            _ => Err(crate::Error::expression_evaluation_failed(
                "can only evaluate Query statements",
            )),
        }
    }
}

impl Limit {
    fn eval_ref(
        &self,
        value: &mut Value,
        scope: &ScopeStack<'_>,
        input: &mut impl Input,
    ) -> Result<()> {
        let Value::List(items) = value else {
            return Err(crate::Error::expression_evaluation_failed(
                "LIMIT requires body to evaluate to a list",
            ));
        };

        match self {
            Limit::Cursor(_) => {
                return Err(crate::Error::expression_evaluation_failed(
                    "cursor-based pagination cannot be evaluated client-side",
                ));
            }
            Limit::Offset(limit_offset) => {
                if let Some(offset_expr) = &limit_offset.offset {
                    let skip = offset_expr.eval_ref_usize(scope, input)?;
                    if skip >= items.len() {
                        items.clear();
                    } else {
                        items.drain(..skip);
                    }
                }

                let n = limit_offset.limit.eval_ref_usize(scope, input)?;
                items.truncate(n);
            }
        }
        Ok(())
    }
}

impl ExprSet {
    fn eval_ref(&self, scope: &ScopeStack<'_>, input: &mut impl Input) -> Result<Value> {
        let ExprSet::Values(values) = self else {
            return Err(crate::Error::expression_evaluation_failed(
                "can only evaluate Values expressions",
            ));
        };

        let mut ret = vec![];

        for row in &values.rows {
            ret.push(row.eval_ref(scope, input)?);
        }

        Ok(Value::List(ret))
    }
}

impl Expr {
    /// Evaluates this expression using the provided [`Input`] for argument
    /// and reference resolution.
    pub fn eval(&self, mut input: impl Input) -> Result<Value> {
        self.eval_ref(&ScopeStack::Root, &mut input)
    }

    /// Evaluates this expression and returns the result as a `bool`.
    ///
    /// # Errors
    ///
    /// Returns an error if the expression does not evaluate to a boolean.
    pub fn eval_bool(&self, mut input: impl Input) -> Result<bool> {
        self.eval_ref_bool(&ScopeStack::Root, &mut input)
    }

    /// Evaluates this expression as a constant (no external input).
    pub fn eval_const(&self) -> Result<Value> {
        self.eval(ConstInput::new())
    }

    fn eval_ref(&self, scope: &ScopeStack<'_>, input: &mut impl Input) -> Result<Value> {
        match self {
            Expr::And(expr_and) => {
                debug_assert!(!expr_and.operands.is_empty());

                for operand in &expr_and.operands {
                    if !operand.eval_ref_bool(scope, input)? {
                        return Ok(false.into());
                    }
                }

                Ok(true.into())
            }
            Expr::Arg(expr_arg) => {
                let Some(expr) = scope.resolve_arg(expr_arg, &Projection::identity(), input) else {
                    return Err(crate::Error::expression_evaluation_failed(
                        "failed to resolve argument",
                    ));
                };
                expr.eval_ref(scope, input)
            }
            Expr::BinaryOp(expr_binary_op) => {
                let lhs = expr_binary_op.lhs.eval_ref(scope, input)?;
                let rhs = expr_binary_op.rhs.eval_ref(scope, input)?;

                match expr_binary_op.op {
                    BinaryOp::Eq => Ok((lhs == rhs).into()),
                    BinaryOp::Ne => Ok((lhs != rhs).into()),
                    BinaryOp::Ge => Ok((cmp_ordered(&lhs, &rhs)? != Ordering::Less).into()),
                    BinaryOp::Gt => Ok((cmp_ordered(&lhs, &rhs)? == Ordering::Greater).into()),
                    BinaryOp::Le => Ok((cmp_ordered(&lhs, &rhs)? != Ordering::Greater).into()),
                    BinaryOp::Lt => Ok((cmp_ordered(&lhs, &rhs)? == Ordering::Less).into()),
                }
            }
            Expr::Cast(expr_cast) => expr_cast.ty.cast(expr_cast.expr.eval_ref(scope, input)?),
            Expr::Default => Err(crate::Error::expression_evaluation_failed(
                "DEFAULT can only be evaluated by the database",
            )),
            Expr::Error(expr_error) => Err(crate::Error::expression_evaluation_failed(
                &expr_error.message,
            )),
            Expr::IsNull(expr_is_null) => {
                let value = expr_is_null.expr.eval_ref(scope, input)?;
                Ok(value.is_null().into())
            }
            Expr::IsVariant(_) => Err(crate::Error::expression_evaluation_failed(
                "IsVariant must be lowered before evaluation",
            )),
            Expr::Let(expr_let) => {
                let args: Vec<_> = expr_let
                    .bindings
                    .iter()
                    .map(|b| b.eval_ref(scope, input))
                    .collect::<Result<_, _>>()?;
                let scope = scope.scope(&args);
                expr_let.body.eval_ref(&scope, input)
            }
            Expr::Not(expr_not) => {
                let value = expr_not.expr.eval_ref_bool(scope, input)?;
                Ok((!value).into())
            }
            Expr::List(exprs) => {
                let mut ret = vec![];

                for expr in &exprs.items {
                    ret.push(expr.eval_ref(scope, input)?);
                }

                Ok(Value::List(ret))
            }
            Expr::Map(expr_map) => {
                let mut base = expr_map.base.eval_ref(scope, input)?;

                let Value::List(items) = &mut base else {
                    return Err(crate::Error::expression_evaluation_failed(
                        "Map base must evaluate to a list",
                    ));
                };

                for item in items.iter_mut() {
                    let args = [item.take()];
                    let scope = scope.scope(&args);
                    *item = expr_map.map.eval_ref(&scope, input)?;
                }

                Ok(base)
            }
            Expr::Project(expr_project) => match &*expr_project.base {
                Expr::Arg(expr_arg) => {
                    let Some(expr) = scope.resolve_arg(expr_arg, &expr_project.projection, input)
                    else {
                        return Err(crate::Error::expression_evaluation_failed(
                            "failed to resolve argument",
                        ));
                    };

                    expr.eval_ref(scope, input)
                }
                Expr::Reference(expr_reference) => {
                    let Some(expr) = input.resolve_ref(expr_reference, &expr_project.projection)
                    else {
                        return Err(crate::Error::expression_evaluation_failed(
                            "failed to resolve reference",
                        ));
                    };

                    expr.eval_ref(scope, input)
                }
                _ => {
                    let base = expr_project.base.eval_ref(scope, input)?;
                    Ok(base.entry(&expr_project.projection).to_value())
                }
            },
            Expr::Record(expr_record) => {
                let mut ret = Vec::with_capacity(expr_record.len());

                for expr in &expr_record.fields {
                    ret.push(expr.eval_ref(scope, input)?);
                }

                Ok(Value::record_from_vec(ret))
            }
            Expr::Reference(expr_reference) => {
                let Some(expr) = input.resolve_ref(expr_reference, &Projection::identity()) else {
                    return Err(crate::Error::expression_evaluation_failed(
                        "failed to resolve reference",
                    ));
                };

                expr.eval_ref(scope, input)
            }
            Expr::Or(expr_or) => {
                debug_assert!(!expr_or.operands.is_empty());

                for operand in &expr_or.operands {
                    if operand.eval_ref_bool(scope, input)? {
                        return Ok(true.into());
                    }
                }

                Ok(false.into())
            }
            Expr::Any(expr_any) => {
                let list = expr_any.expr.eval_ref(scope, input)?;

                let Value::List(items) = list else {
                    return Err(crate::Error::expression_evaluation_failed(
                        "Any expression must evaluate to a list",
                    ));
                };

                for item in &items {
                    match item {
                        Value::Bool(true) => return Ok(true.into()),
                        Value::Bool(false) => {}
                        _ => {
                            return Err(crate::Error::expression_evaluation_failed(
                                "Any expression items must evaluate to bool",
                            ));
                        }
                    }
                }

                Ok(false.into())
            }
            Expr::InList(expr_in_list) => {
                let needle = expr_in_list.expr.eval_ref(scope, input)?;
                let list = expr_in_list.list.eval_ref(scope, input)?;

                let Value::List(items) = list else {
                    return Err(crate::Error::expression_evaluation_failed(
                        "InList right-hand side must evaluate to a list",
                    ));
                };

                Ok(items.iter().any(|item| item == &needle).into())
            }
            Expr::Match(expr_match) => {
                let subject = expr_match.subject.eval_ref(scope, input)?;
                for arm in &expr_match.arms {
                    if subject == arm.pattern {
                        return arm.expr.eval_ref(scope, input);
                    }
                }
                expr_match.else_expr.eval_ref(scope, input)
            }
            Expr::Exists(expr_exists) => {
                // Evaluate the subquery body. For Values bodies the rows are
                // evaluated and flattened; for other bodies we evaluate the
                // query as an expression.
                match &expr_exists.subquery.body {
                    ExprSet::Values(values) => {
                        for row in &values.rows {
                            let val = row.eval_ref(scope, input)?;
                            match val {
                                // An empty list means no rows — keep checking
                                Value::List(items) if items.is_empty() => {}
                                // Null means the row doesn't exist
                                Value::Null => {}
                                // Any other value means at least one row exists
                                _ => return Ok(true.into()),
                            }
                        }
                        Ok(false.into())
                    }
                    _ => todo!("ExprExists with non-Values body"),
                }
            }
            Expr::Value(value) => Ok(value.clone()),
            Expr::Func(_) => Err(crate::Error::expression_evaluation_failed(
                "database functions cannot be evaluated client-side",
            )),
            _ => todo!("expr={self:#?}"),
        }
    }

    fn eval_ref_bool(&self, scope: &ScopeStack<'_>, input: &mut impl Input) -> Result<bool> {
        match self.eval_ref(scope, input)? {
            Value::Bool(ret) => Ok(ret),
            _ => Err(crate::Error::expression_evaluation_failed(
                "expected boolean value",
            )),
        }
    }

    fn eval_ref_usize(&self, scope: &ScopeStack<'_>, input: &mut impl Input) -> Result<usize> {
        match self.eval_ref(scope, input)? {
            Value::I64(n) if n >= 0 => Ok(n as usize),
            _ => Err(crate::Error::expression_evaluation_failed(
                "expected non-negative integer",
            )),
        }
    }
}

impl ScopeStack<'_> {
    fn resolve_arg(
        &self,
        expr_arg: &ExprArg,
        projection: &Projection,
        input: &mut impl Input,
    ) -> Option<Expr> {
        let mut nesting = expr_arg.nesting;
        let mut scope = self;

        while nesting > 0 {
            nesting -= 1;

            scope = match scope {
                ScopeStack::Root => return None,
                ScopeStack::Scope { parent, .. } => parent,
            };
        }

        match scope {
            ScopeStack::Root => input.resolve_arg(expr_arg, projection),
            &ScopeStack::Scope { mut args, .. } => args.resolve_arg(expr_arg, projection),
        }
    }

    fn scope<'child>(&'child self, args: &'child [Value]) -> ScopeStack<'child> {
        ScopeStack::Scope { args, parent: self }
    }
}

fn cmp_ordered(lhs: &Value, rhs: &Value) -> Result<Ordering> {
    if lhs.is_null() || rhs.is_null() {
        return Err(crate::Error::expression_evaluation_failed(
            "ordered comparison with NULL is undefined",
        ));
    }
    lhs.partial_cmp(rhs).ok_or_else(|| {
        crate::Error::expression_evaluation_failed("ordered comparison between incompatible types")
    })
}
