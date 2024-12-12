use super::*;

/// A entry point for an evaluation
#[derive(Debug, Clone)]
pub struct Func {
    /// Function arguments
    pub args: Vec<stmt::Type>,

    /// Function return type
    pub ret: stmt::Type,

    /// Function body
    pub expr: Expr,
}

struct TypedInput<'a, I> {
    input: &'a mut I,
    tys: &'a [stmt::Type],
}

impl Func {
    pub fn new(args: Vec<stmt::Type>, expr: Expr) -> Func {
        let ret = expr.ty(&args);
        Func { args, ret, expr }
    }

    /// Returns the identity function for the given type
    pub fn identity(ty: stmt::Type) -> Func {
        Func {
            args: vec![ty.clone()],
            ret: ty,
            expr: Expr::arg(0),
        }
    }

    pub fn is_identity(&self) -> bool {
        matches!(&self.expr, Expr::Arg(expr_arg) if expr_arg.position == 0)
    }

    pub fn eval(&self, mut input: impl Input) -> Result<stmt::Value> {
        let mut input = TypedInput {
            input: &mut input,
            tys: &self.args,
        };

        self.expr.eval_ref(&mut input)
    }

    /// Special case of `eval` where the expression is a constant
    ///
    /// # Panics
    ///
    /// `eval_const` panics if the expression is not constant
    pub fn eval_const(&self) -> stmt::Value {
        assert!(self.args.is_empty());
        self.expr.eval_ref(&mut const_input()).unwrap()
    }

    pub fn eval_bool(&self, mut input: impl Input) -> Result<bool> {
        self.expr.eval_bool_ref(&mut input)
    }
}

impl<'a, I: Input> Input for TypedInput<'a, I> {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &stmt::Projection) -> stmt::Value {
        let value = self.input.resolve_arg(expr_arg, projection);

        if !value.is_null() {
            let mut ty = &self.tys[expr_arg.position];

            for step in projection {
                ty = match ty {
                    stmt::Type::Record(tys) => &tys[step],
                    _ => todo!("ty={ty:#?}"),
                };
            }

            assert!(value.is_a(ty), "value={value:#?}; ty={ty:#?};");
        }

        value
    }
}
