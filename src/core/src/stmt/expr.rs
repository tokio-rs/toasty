use super::*;

use std::ops;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr<'stmt> {
    /// AND a set of binary expressions
    And(ExprAnd<'stmt>),

    /// An argument when the expression is a function body
    Arg(ExprArg),

    /// Binary expression
    BinaryOp(ExprBinaryOp<'stmt>),

    /// Cast an expression to a different type
    Cast(ExprCast<'stmt>),

    /// References a column from a table in the statement
    Column(ExprColumn),

    /// Concat multiple expressions together
    Concat(ExprConcat<'stmt>),

    /// Return an enum value
    Enum(ExprEnum<'stmt>),

    /// References a field in the statement
    Field(ExprField),

    /// In list
    InList(ExprInList<'stmt>),

    /// The expression is contained by the given subquery
    InSubquery(ExprInSubquery<'stmt>),

    /// OR a set of binary expressi5nos
    Or(ExprOr<'stmt>),

    /// Checks if an expression matches a pattern.
    Pattern(ExprPattern<'stmt>),

    /// Project an expression
    Project(ExprProject<'stmt>),

    /// Evaluates to a tuple value
    Record(ExprRecord<'stmt>),

    /// A list of expressions of the same type
    List(Vec<Expr<'stmt>>),

    /// Evaluate a sub-statement
    Stmt(ExprStmt<'stmt>),

    /// A type reference. This is used by the "is a" expression
    Type(ExprTy),

    /// Evaluates to a constant value reference
    Value(Value<'stmt>),
}

impl<'stmt> Expr<'stmt> {
    pub fn and(lhs: impl Into<Expr<'stmt>>, rhs: impl Into<Expr<'stmt>>) -> Expr<'stmt> {
        let mut lhs = lhs.into();
        let rhs = rhs.into();

        match (&mut lhs, rhs) {
            (Expr::And(lhs_and), Expr::And(rhs_and)) => {
                lhs_and.operands.extend(rhs_and.operands);
                lhs
            }
            (Expr::And(lhs_and), rhs) => {
                lhs_and.operands.push(rhs);
                lhs
            }
            (_, Expr::And(mut rhs_and)) => {
                rhs_and.operands.push(lhs);
                rhs_and.into()
            }
            (_, rhs) => ExprAnd::new(vec![lhs, rhs]).into(),
        }
    }

    pub fn or(lhs: impl Into<Expr<'stmt>>, rhs: impl Into<Expr<'stmt>>) -> Expr<'stmt> {
        let mut lhs = lhs.into();
        let rhs = rhs.into();

        match (&mut lhs, rhs) {
            (Expr::Or(lhs_or), Expr::Or(rhs_or)) => {
                lhs_or.operands.extend(rhs_or.operands);
                lhs
            }
            (Expr::Or(lhs_or), rhs) => {
                lhs_or.operands.push(rhs);
                lhs
            }
            (_, Expr::Or(mut lhs_or)) => {
                lhs_or.operands.push(lhs);
                lhs_or.into()
            }
            (_, rhs) => ExprOr::new(vec![lhs, rhs]).into(),
        }
    }

    pub fn null() -> Expr<'stmt> {
        Expr::Value(Value::Null)
    }

    /// Is a value that evaluates to null
    pub fn is_null(&self) -> bool {
        matches!(self, Expr::Value(Value::Null))
    }

    pub fn in_subquery(lhs: impl Into<Expr<'stmt>>, rhs: impl Into<Query<'stmt>>) -> Expr<'stmt> {
        ExprInSubquery {
            expr: Box::new(lhs.into()),
            query: Box::new(rhs.into()),
        }
        .into()
    }

    pub fn list<T>(items: impl IntoIterator<Item = T>) -> Expr<'stmt>
    where
        T: Into<Expr<'stmt>>,
    {
        Expr::List(items.into_iter().map(Into::into).collect())
    }

    /// Returns true if the expression is the `true` boolean expression
    pub fn is_true(&self) -> bool {
        matches!(self, Expr::Value(Value::Bool(true)))
    }

    /// Returns true if the expression is a constant value.
    pub fn is_value(&self) -> bool {
        matches!(self, Expr::Value(..))
    }

    pub fn is_stmt(&self) -> bool {
        matches!(self, Expr::Stmt(..))
    }

    /// Returns true if the expression is a binary operation
    pub fn is_binary_op(&self) -> bool {
        matches!(self, Expr::BinaryOp(..))
    }

    pub fn is_arg(&self) -> bool {
        matches!(self, Expr::Arg(_))
    }

    pub fn into_value(self) -> Value<'stmt> {
        match self {
            Expr::Value(value) => value,
            _ => todo!(),
        }
    }

    pub fn into_stmt(self) -> ExprStmt<'stmt> {
        match self {
            Expr::Stmt(stmt) => stmt,
            _ => todo!(),
        }
    }

    /// Returns `true` if the expression is a constant expression.
    pub fn is_const(&self) -> bool {
        match self {
            Expr::Value(_) => true,
            Expr::Record(expr_record) => expr_record.iter().all(|expr| expr.is_const()),
            _ => false,
        }
    }

    pub fn simplify(&mut self) {
        visit_mut::for_each_expr_mut(self, move |expr| {
            let maybe_expr = match expr {
                Expr::BinaryOp(expr) => expr.simplify(),
                Expr::Cast(expr) => expr.simplify(),
                Expr::InList(expr) => expr.simplify(),
                Expr::Record(expr) => expr.simplify(),
                _ => None,
            };

            if let Some(simplified) = maybe_expr {
                *expr = simplified;
            }
        });
    }

    pub fn substitute(&mut self, mut input: impl substitute::Input<'stmt>) {
        self.substitute_ref(&mut input);
    }

    pub(crate) fn substitute_ref(&mut self, input: &mut impl substitute::Input<'stmt>) {
        visit_mut::for_each_expr_mut(self, move |expr| match expr {
            Expr::Arg(expr_arg) => {
                if let Some(sub) = input.resolve_arg(expr_arg) {
                    *expr = sub;
                }
            }
            Expr::Field(expr_field) => {
                if let Some(sub) = input.resolve_field(expr_field) {
                    *expr = sub;
                }
            }
            Expr::Column(expr_column) => {
                if let Some(sub) = input.resolve_column(expr_column) {
                    *expr = sub;
                }
            }
            Expr::Project(expr) => todo!("project = {:#?}", expr),
            /*
            Expr::Project(expr_project) => match &expr_project.base {
                ProjectBase::ExprSelf => {
                    *expr = input.resolve_self_projection(&expr_project.projection);
                }
                _ => {}
            },
            */
            _ => {}
        });

        self.simplify();
    }

    pub fn map_projections(&self, f: impl FnMut(&Projection) -> Projection) -> Expr<'stmt> {
        struct MapProjections<T>(T);

        impl<'stmt, T: FnMut(&Projection) -> Projection> VisitMut<'stmt> for MapProjections<T> {
            fn visit_projection_mut(&mut self, i: &mut Projection) {
                *i = self.0(i);
            }
        }

        let mut mapped = self.clone();
        MapProjections(f).visit_expr_mut(&mut mapped);
        mapped
    }

    /// Assume the expression evaluates to a set of records and extend the
    /// evaluation to include the given expression.
    ///
    /// TODO: maybe split up the ops and instead have `expr.as_or_cast_to_concat().push()`
    pub fn push(&mut self, expr: impl Into<Expr<'stmt>>) {
        use std::mem;

        match self {
            Expr::Concat(exprs) => exprs.push(expr.into()),
            Expr::Value(Value::Null) => {
                *self = ExprConcat::new(vec![expr.into()]).into();
            }
            _ => {
                let prev = mem::replace(self, Expr::Value(Value::Null));
                *self = ExprConcat::new(vec![prev, expr.into()]).into();
            }
        }
    }

    pub fn take(&mut self) -> Expr<'stmt> {
        std::mem::replace(self, Expr::Value(Value::Null))
    }

    /*
    /// Updates the expression to assume a projected `expr_self`
    ///
    /// TODO: rename this... not a good name
    pub fn project_self(&mut self, steps: usize) {
        visit_mut::for_each_expr_mut(self, |expr| {
            match expr {
                Expr::Project(expr_project) => {
                    assert!(expr_project.base.is_expr_self());

                    // Trim the start of the projection
                    expr_project.projection = Projection::from(&expr_project.projection[steps..]);
                }
                _ => {}
            }
        });
    }
    */
}

impl<'stmt> Default for Expr<'stmt> {
    fn default() -> Self {
        Expr::Value(Value::default())
    }
}

impl<'stmt, I: Into<PathStep>> ops::Index<I> for Expr<'stmt> {
    type Output = Expr<'stmt>;

    fn index(&self, index: I) -> &Self::Output {
        match self {
            Expr::Record(expr_record) => expr_record.index(index.into().into_usize()),
            _ => todo!(),
        }
    }
}

impl<'stmt, I: Into<PathStep>> ops::IndexMut<I> for Expr<'stmt> {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        match self {
            Expr::Record(expr_record) => expr_record.index_mut(index.into().into_usize()),
            _ => todo!("trying to index {:#?}", self),
        }
    }
}

impl<'stmt> Node<'stmt> for Expr<'stmt> {
    fn map<V: Map<'stmt>>(&self, visit: &mut V) -> Self {
        visit.map_expr(self)
    }

    fn visit<V: Visit<'stmt>>(&self, mut visit: V) {
        visit.visit_expr(self);
    }

    fn visit_mut<V: VisitMut<'stmt>>(&mut self, mut visit: V) {
        visit.visit_expr_mut(self);
    }
}

// === Conversions ===

impl<'stmt> From<bool> for Expr<'stmt> {
    fn from(value: bool) -> Expr<'stmt> {
        Expr::Value(Value::from(value))
    }
}

impl<'stmt> From<i64> for Expr<'stmt> {
    fn from(value: i64) -> Self {
        Expr::Value(value.into())
    }
}

impl<'stmt> From<&i64> for Expr<'stmt> {
    fn from(value: &i64) -> Self {
        Expr::Value(value.into())
    }
}

impl<'stmt> From<String> for Expr<'stmt> {
    fn from(value: String) -> Self {
        Expr::Value(value.into())
    }
}

impl<'stmt> From<&'stmt String> for Expr<'stmt> {
    fn from(value: &'stmt String) -> Self {
        Expr::Value(value.into())
    }
}

impl<'stmt> From<Value<'stmt>> for Expr<'stmt> {
    fn from(value: Value<'stmt>) -> Expr<'stmt> {
        Expr::Value(value)
    }
}

impl<'stmt, E1, E2> From<(E1, E2)> for Expr<'stmt>
where
    E1: Into<Expr<'stmt>>,
    E2: Into<Expr<'stmt>>,
{
    fn from(value: (E1, E2)) -> Expr<'stmt> {
        Expr::Record(value.into())
    }
}
