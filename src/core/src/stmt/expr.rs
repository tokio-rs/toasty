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

    /// Concat multiple expressions together
    Concat(ExprConcat<'stmt>),

    /// Return an enum value
    Enum(ExprEnum<'stmt>),

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

    /// Returns the identity expression.
    ///
    /// TODO: delete?
    pub const fn identity() -> Expr<'stmt> {
        Expr::Project(ExprProject {
            base: ProjectBase::ExprSelf,
            projection: Projection::identity(),
        })
    }

    pub const fn is_identity(&self) -> bool {
        match self {
            Expr::Project(expr_project) => expr_project.is_identity(),
            _ => false,
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

    pub fn self_expr() -> Expr<'stmt> {
        ExprProject {
            base: ProjectBase::ExprSelf,
            projection: Projection::identity(),
        }
        .into()
    }

    pub fn record<T>(items: impl IntoIterator<Item = T>) -> Expr<'stmt>
    where
        T: Into<Expr<'stmt>>,
    {
        Expr::Record(ExprRecord::from_iter(items.into_iter()))
    }

    pub fn is_record(&self) -> bool {
        matches!(self, Expr::Record(_))
    }

    pub fn as_record(&self) -> &ExprRecord<'stmt> {
        match self {
            Expr::Record(expr_record) => expr_record,
            _ => panic!(),
        }
    }

    pub fn as_record_mut(&mut self) -> &mut ExprRecord<'stmt> {
        match self {
            Expr::Record(expr_record) => expr_record,
            _ => panic!(),
        }
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

    pub fn simplify(&mut self) {
        visit_mut::for_each_expr_mut(self, move |expr| {
            let maybe_expr = match expr {
                // Simplification step. If the original expression is an "in
                // list" op, but the right-hand side is a record with a single
                // entry, then simplify the expression to an equality.
                Expr::InList(e) => e.simplify(),
                Expr::Record(expr_record) => expr_record.simplify(),
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
                *expr = input.resolve_arg(expr_arg);
            }
            Expr::Project(expr_project) => match &expr_project.base {
                ProjectBase::ExprSelf => {
                    *expr = input.resolve_self_projection(&expr_project.projection);
                }
                _ => {}
            },
            _ => {}
        });

        self.simplify();
    }

    /// Special case of `eval` where the expression is a constant
    ///
    /// # Panics
    ///
    /// `eval_const` panics if the expression is not constant
    pub(crate) fn eval_const(&self) -> Value<'stmt> {
        self.eval_ref(&mut eval::const_input()).unwrap()
    }

    pub(crate) fn eval_bool_ref(&self, input: &mut impl eval::Input<'stmt>) -> Result<bool> {
        match self.eval_ref(input)? {
            Value::Bool(ret) => Ok(ret),
            _ => todo!(),
        }
    }

    pub(crate) fn eval_ref(&self, input: &mut impl eval::Input<'stmt>) -> Result<Value<'stmt>> {
        match self {
            Expr::And(expr_and) => {
                debug_assert!(!expr_and.operands.is_empty());

                for operand in &expr_and.operands {
                    if !operand.eval_bool_ref(input)? {
                        return Ok(false.into());
                    }
                }

                Ok(true.into())
            }
            Expr::Arg(expr_arg) => Ok(input.resolve_arg(expr_arg)),
            Expr::Value(value) => Ok(value.clone()),
            Expr::BinaryOp(expr_binary_op) => {
                let lhs = expr_binary_op.lhs.eval_ref(input)?;
                let rhs = expr_binary_op.rhs.eval_ref(input)?;

                match expr_binary_op.op {
                    BinaryOp::Eq => Ok((lhs == rhs).into()),
                    BinaryOp::Ne => Ok((lhs != rhs).into()),
                    _ => todo!("{:#?}", self),
                }
            }
            Expr::Enum(expr_enum) => Ok(ValueEnum {
                variant: expr_enum.variant,
                fields: expr_enum.fields.eval_ref(input)?,
            }
            .into()),
            Expr::Project(expr_project) => match expr_project.base {
                ProjectBase::ExprSelf => {
                    Ok(input.resolve_self_projection(&expr_project.projection))
                }
                _ => todo!(),
            },
            Expr::Record(expr_record) => Ok(expr_record.eval_ref(input)?.into()),
            Expr::List(exprs) => {
                let mut applied = vec![];

                for expr in exprs {
                    applied.push(expr.eval_ref(input)?);
                }

                Ok(Value::List(applied))
            }
            _ => todo!("{:#?}", self),
        }
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
}

impl<'stmt> Default for Expr<'stmt> {
    fn default() -> Self {
        Expr::Value(Value::default())
    }
}

impl<'stmt> ops::Index<usize> for Expr<'stmt> {
    type Output = Expr<'stmt>;

    fn index(&self, index: usize) -> &Self::Output {
        match self {
            Expr::Record(expr_record) => expr_record.index(index),
            _ => todo!(),
        }
    }
}

impl<'stmt> ops::IndexMut<usize> for Expr<'stmt> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match self {
            Expr::Record(expr_record) => expr_record.index_mut(index),
            _ => todo!("trying to index {:#?}", self),
        }
    }
}

impl<'stmt> ops::Index<PathStep> for Expr<'stmt> {
    type Output = Expr<'stmt>;

    fn index(&self, index: PathStep) -> &Self::Output {
        self.index(index.into_usize())
    }
}

impl<'stmt> ops::IndexMut<PathStep> for Expr<'stmt> {
    fn index_mut(&mut self, index: PathStep) -> &mut Self::Output {
        self.index_mut(index.into_usize())
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

impl<'stmt> From<ColumnId> for Expr<'stmt> {
    fn from(value: ColumnId) -> Self {
        ExprProject::from(value).into()
    }
}

impl<'stmt> From<ExprRecord<'stmt>> for Expr<'stmt> {
    fn from(value: ExprRecord<'stmt>) -> Expr<'stmt> {
        Expr::Record(value)
    }
}

impl<'stmt> From<&Field> for Expr<'stmt> {
    fn from(value: &Field) -> Self {
        ExprProject::from(value).into()
    }
}

impl<'stmt> From<FieldId> for Expr<'stmt> {
    fn from(value: FieldId) -> Self {
        ExprProject::from(value).into()
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
