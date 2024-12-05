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

    pub fn eval(&self, mut input: impl Input) -> Result<stmt::Value> {
        todo!()
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
