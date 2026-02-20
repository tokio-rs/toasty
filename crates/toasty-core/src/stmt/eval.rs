use crate::{
    stmt::{BinaryOp, ConstInput, Expr, ExprArg, ExprSet, Input, Projection, Statement, Value},
    Result,
};

enum ScopeStack<'a> {
    Root,
    Scope {
        args: &'a [Value],
        parent: &'a ScopeStack<'a>,
    },
}

impl Statement {
    pub fn eval(&self, mut input: impl Input) -> Result<Value> {
        self.eval_ref(&ScopeStack::Root, &mut input)
    }

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

                assert!(query.order_by.is_none(), "TODO");
                assert!(query.limit.is_none(), "TODO");
                assert!(!query.single, "TODO");

                query.body.eval_ref(scope, input)
            }
            _ => Err(crate::Error::expression_evaluation_failed(
                "can only evaluate Query statements",
            )),
        }
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
    pub fn eval(&self, mut input: impl Input) -> Result<Value> {
        self.eval_ref(&ScopeStack::Root, &mut input)
    }

    pub fn eval_bool(&self, mut input: impl Input) -> Result<bool> {
        self.eval_ref_bool(&ScopeStack::Root, &mut input)
    }

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
                    BinaryOp::Ge => Ok((lhs >= rhs).into()),
                    BinaryOp::Gt => Ok((lhs > rhs).into()),
                    BinaryOp::Le => Ok((lhs <= rhs).into()),
                    BinaryOp::Lt => Ok((lhs < rhs).into()),
                    BinaryOp::IsA => todo!("IsA binary op not yet implemented"),
                }
            }
            Expr::Cast(expr_cast) => expr_cast.ty.cast(expr_cast.expr.eval_ref(scope, input)?),
            Expr::ConcatStr(expr_concat_str) => {
                let mut ret = String::new();

                for expr in &expr_concat_str.exprs {
                    let Value::String(s) = expr.eval_ref(scope, input)? else {
                        return Err(crate::Error::expression_evaluation_failed(
                            "string concatenation requires string values",
                        ));
                    };

                    ret.push_str(&s);
                }

                Ok(ret.into())
            }
            Expr::Default => Err(crate::Error::expression_evaluation_failed(
                "DEFAULT can only be evaluated by the database",
            )),
            Expr::IsNull(expr_is_null) => {
                let value = expr_is_null.expr.eval_ref(scope, input)?;
                Ok(value.is_null().into())
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

                let Value::List(ref mut items) = &mut base else {
                    todo!("error handling; base={base:#?}")
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
            Expr::Value(value) => Ok(value.clone()),
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
                ScopeStack::Root => todo!("error handling"),
                ScopeStack::Scope { parent, .. } => parent,
            };
        }

        match scope {
            ScopeStack::Root => input.resolve_arg(expr_arg, projection),
            ScopeStack::Scope { mut args, .. } => args.resolve_arg(expr_arg, projection),
        }
    }

    fn scope<'child>(&'child self, args: &'child [Value]) -> ScopeStack<'child> {
        ScopeStack::Scope { args, parent: self }
    }
}
